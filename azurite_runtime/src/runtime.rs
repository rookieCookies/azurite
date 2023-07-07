use azurite_common::consts;
use colored::Colorize;
use libloading::Library;

use crate::{object_map::{Object, Structure}, Code, FatalError, Status, VMData, VM, ExternFunction};
use std::ops::{Add, Mul, Sub};

impl VM<'_> {
    #[allow(clippy::too_many_lines)]
    #[inline(never)]
    pub(crate) fn run(&mut self) -> Status {
        macro_rules! all_integer_types {
            ($f: ident) => {
                (
                    i8 ::$f,
                    i16::$f,
                    i32::$f,
                    i64::$f,
                    u8 ::$f,
                    u16::$f,
                    u32::$f,
                    u64::$f
                )
            }
        }

        
        macro_rules! cast_to {
            ($t: ty, $variant: ident) => { {
                let dst = self.current.next();
                let val = self.current.next();

                let v = match self.stack.reg(val) {
                    VMData::I8 (v)   => v as $t,
                    VMData::I16(v)   => v as $t,
                    VMData::I32(v)   => v as $t,
                    VMData::I64(v)   => v as $t,
                    VMData::U8 (v)   => v as $t,
                    VMData::U16(v)   => v as $t,
                    VMData::U32(v)   => v as $t,
                    VMData::U64(v)   => v as $t,
                    VMData::Float(v) => v as $t,

                    _ => unreachable!(),
                };

                self.stack.set_reg(dst, VMData::$variant(v));
            } }
        }


        let result: Status = 'global: loop {
            let value = self.current.next();

            match value {
                consts::ExternFile => {
                    let path = self.current.string();

                    #[cfg(target_os = "windows")]
                    let path = format!("{path}.dll");

                    #[cfg(target_os = "linux")]
                    let path = format!("{path}.so");

                    #[cfg(target_os = "macos")]
                    let path = format!("{path}.dylib");

                    #[cfg(not(any(
                        target_os = "windows",
                        target_os = "linux",
                        target_os = "macos",
                    )))]
                    compile_error!("this platform is not supported");

                    let func_amount = self.current.next();

                    // let Ok(lib) = Library::new(&path) else { break Err(format!("can't find a runtime library file named {path}")); };
                    let lib = match unsafe { Library::new(&path) } {
                        Ok(v) => v,
                        Err(_) => {
                            let new_path = {
                                let Ok(p) = std::env::current_exe() else { break Status::err("can't get the path for the runtime executable") };
                                let Some(p) = p.parent() else { break Status::err("can't get the parent of the path of the current executable") };

                                p
                                    .join("runtime")
                                    .join(&path)
                            };
                            
                            match unsafe { Library::new(&new_path) } {
                                Ok(v) => v,
                                Err(_) => break Status::Err(FatalError::new(format!("can't find a runtime library file named {path}")))
                            }
                        }
                    };


                    for _ in 0..func_amount {
                        let index = self.current.u32();
                        let name = self.current.string();
                        let Ok(func) = (unsafe { lib.get::<ExternFunction<'_>>(name.as_bytes()) }) else { break 'global Status::err(format!("can't find a function named {name:?} in {path}")); };

                        if index as usize > self.externs.len() {
                            self.externs.push(**unsafe { func.into_raw() });
                        } else {
                            self.externs.insert(index as usize, **unsafe { func.into_raw() });
                        }
                    }

                    if let Ok(x) = unsafe { lib.get::<ExternFunction<'_>>(b"_init") } {
                        unsafe { x(self) };
                    }

                    // std::mem::forget(lib);
                    self.libraries.push(lib);
                }


                consts::Copy => {
                    let dst = self.current.next();
                    let src = self.current.next();

                    let data = self.stack.reg(src);
                    self.stack.set_reg(dst, data);
                }

                
                consts::Swap => {
                    let v1 = self.current.next();
                    let v2 = self.current.next();

                    self.stack.values.swap(v1 as usize, v2 as usize);
                }


                consts::Add => self.binary_operation(
                    VM::arithmetic_operation,
                    all_integer_types!(wrapping_add),
                    f64::add,
                ),

                
                consts::Subtract => self.binary_operation(
                    VM::arithmetic_operation,
                    all_integer_types!(wrapping_sub),
                    f64::sub,
                ),

                
                consts::Multiply => self.binary_operation(
                    VM::arithmetic_operation,
                    all_integer_types!(wrapping_mul),
                    f64::mul,
                ),

                
                consts::Modulo => self.binary_operation(
                    VM::arithmetic_operation,
                    all_integer_types!(wrapping_rem),
                    f64::rem_euclid,
                ),

                
                consts::Divide => {
                    macro_rules! integer_division {
                        ($v: ident, $v1: expr, $v2: expr) => {
                            if $v2 == 0 {
                                break Status::Err(FatalError::new(String::from(
                                    "division by zero",
                                )));
                            } else {
                                VMData::$v($v1.wrapping_div($v2))
                            }
                        } 
                    }
                    
                    let dst = self.current.next();
                    let v1 = self.current.next();
                    let v2 = self.current.next();

                    let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
                        (VMData::I8(v1),  VMData::I8(v2))  => integer_division!(I8,  v1, v2),
                        (VMData::I16(v1), VMData::I16(v2)) => integer_division!(I16, v1, v2),
                        (VMData::I32(v1), VMData::I32(v2)) => integer_division!(I32, v1, v2),
                        (VMData::I64(v1), VMData::I64(v2)) => integer_division!(I64, v1, v2),

                        (VMData::U8(v1),  VMData::U8(v2))  => integer_division!(U8,  v1, v2),
                        (VMData::U16(v1), VMData::U16(v2)) => integer_division!(U16, v1, v2),
                        (VMData::U32(v1), VMData::U32(v2)) => integer_division!(U32, v1, v2),
                        (VMData::U64(v1), VMData::U64(v2)) => integer_division!(U64, v1, v2),

                        (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(v1 / v2),

                        _ => unreachable!(),
                    };

                    self.stack.set_reg(dst, val);
                }


                consts::GreaterThan   => self.binary_operation(VM::comparisson_operation, all_integer_types!(gt), f64::gt),
                consts::LesserThan    => self.binary_operation(VM::comparisson_operation, all_integer_types!(lt), f64::lt),
                consts::GreaterEquals => self.binary_operation(VM::comparisson_operation, all_integer_types!(ge), f64::ge),
                consts::LesserEquals  => self.binary_operation(VM::comparisson_operation, all_integer_types!(le), f64::le),


                consts::Equals => {
                    let dst = self.current.next();
                    let v1 = self.current.next();
                    let v2 = self.current.next();

                    let value = self.stack.reg(v1) == self.stack.reg(v2);
                    self.stack.set_reg(dst, VMData::Bool(value));
                }


                consts::NotEquals => {
                    let dst = self.current.next();
                    let v1 = self.current.next();
                    let v2 = self.current.next();

                    let value = self.stack.reg(v1) != self.stack.reg(v2);
                    self.stack.set_reg(dst, VMData::Bool(value));
                }


                consts::LoadConst => {
                    let dst = self.current.next();
                    let data_index = self.current.next();

                    let data = self.constants[data_index as usize];
                    self.stack.set_reg(dst, data);
                }


                consts::Jump => {
                    let jump_at = self.current.u32();

                    self.current.goto(jump_at as usize);
                }


                consts::JumpCond => {
                    let condition = self.current.next();
                    let if_true = self.current.u32();
                    let if_false = self.current.u32();

                    let val = self.stack.reg(condition).as_bool();

                    if val {
                        self.current.goto(if_true as usize);
                    } else {
                        self.current.goto(if_false as usize);
                    }
                }


                consts::Return => {
                    let Some(current) = self.callstack.pop() else { break Status::Ok };

                    let ret_val = self.stack.reg(0);
                    let ret_reg = self.current.return_to;

                    self.current = current;
                    self.stack.set_stack_offset(self.current.offset);

                    self.stack.set_reg(ret_reg, ret_val);
                    self.stack.pop(1);
                }


                consts::Call => {
                    let goto = self.current.u32();
                    let dst = self.current.next();
                    let arg_count = self.current.next() as usize;

                    if let Status::Err(e) = self.stack.push(arg_count + 1) {
                        break Status::Err(e);
                    }

                    let temp = self.stack.top - arg_count - self.stack.stack_offset;
                    for v in 0..arg_count {
                        let reg = self.stack.reg(self.current.next());
                        self.stack.set_reg(convert_usize_to_u8(temp + v), reg);
                    }

                    let mut code = Code::new(self.current.code, self.stack.top - arg_count - 1, dst);
                    code.goto(goto as usize);

                    self.callstack.push(std::mem::replace(&mut self.current, code));

                    self.stack.set_stack_offset(self.current.offset);
                }


                consts::ExtCall => {
                    let index = self.current.u32();
                    let dst = self.current.next();
                    let arg_count = self.current.next() as usize;

                    if let Status::Err(e) = self.stack.push(arg_count + 1) {
                        break Status::Err(e);
                    }

                    let temp = self.stack.top - arg_count - self.stack.stack_offset;
                    for v in 0..arg_count {
                        let reg = self.stack.reg(self.current.next());
                        self.stack.set_reg(convert_usize_to_u8(temp + v), reg);
                    }

                    self.stack.set_stack_offset(self.stack.top - arg_count - 1);

                    let function = self.externs[index as usize];
                    let result = unsafe { function(self) };

                    
                    if result.is_exit() || result.is_err() {
                        break result;
                    }

                    let ret_val = self.stack.reg(0);
                    self.stack.set_stack_offset(self.current.offset);

                    self.stack.set_reg(dst, ret_val);
                    self.stack.pop(arg_count + 1);
                }


                consts::Push => {
                    let amount = self.current.next();
                    if let Status::Err(e) = self.stack.push(amount as usize) {
                        break Status::Err(e);
                    }
                }


                consts::Pop => {
                    let amount = self.current.next();
                    self.stack.pop(amount as usize);
                }


                consts::Unit => {
                    #[cfg(debug_assertions)]
                    {
                        let reg = self.current.next();
                        self.stack.set_reg(reg, VMData::Empty);
                    }
                }


                consts::Struct => {
                    let dst = self.current.next();
                    let amount = self.current.next();

                    let vec = (0..amount)
                        .map(|_| self.stack.reg(self.current.next()))
                        .collect();

                    let index = match self.create_object(Object::new(Structure::new(vec))) {
                        Ok(v) => v,
                        Err(e) => break Status::Err(e),
                    };
                    
                    self.stack.set_reg(dst, VMData::Object(index));
                }


                consts::AccStruct => {
                    let dst = self.current.next();
                    let struct_at = self.current.next();
                    let index = self.current.next();

                    let val = self.stack.reg(struct_at);
                    let obj = match val {
                        VMData::Object(v) => self.objects.get(v),

                        _ => unreachable!(),
                    };

                    let accval = obj.structure().fields()[index as usize];

                    // dbg!(dst, struct_at, val);
                    self.stack.set_reg(dst, accval);
                }


                consts::SetField => {
                    let struct_at = self.current.next();
                    let data = self.current.next();
                    let index = self.current.next();

                    let data = self.stack.reg(data);

                    let val = self.stack.reg(struct_at);
                    let obj = match val {
                        VMData::Object(v) => self.objects.get_mut(v),

                        _ => unreachable!(),
                    };

                    obj.structure_mut().fields_mut()[index as usize] = data;
                }


                consts::UnaryNeg => {
                    let dst = self.current.next();
                    let val = self.current.next();

                    match self.stack.reg(val) {
                        VMData::I8 (v)  => self.stack.set_reg(dst, VMData::I8(-v)),
                        VMData::I16(v)  => self.stack.set_reg(dst, VMData::I16(-v)),
                        VMData::I32(v)  => self.stack.set_reg(dst, VMData::I32(-v)),
                        VMData::I64(v)  => self.stack.set_reg(dst, VMData::I64(-v)),
                        VMData::Float(v) => self.stack.set_reg(dst, VMData::Float(-v)),

                        _ => unreachable!(),
                    }
                }


                consts::UnaryNot => {
                    let dst = self.current.next();
                    let val = self.current.next();

                    let data = self.stack.reg(val).as_bool();
                    self.stack.set_reg(dst, VMData::Bool(!data))
                }


                consts::CastToI8  => cast_to!(i8 , I8),
                consts::CastToI16 => cast_to!(i16, I16),
                consts::CastToI32 => cast_to!(i32, I32),
                consts::CastToI64 => cast_to!(i64, I64),
                consts::CastToU8  => cast_to!(u8 , U8),
                consts::CastToU16 => cast_to!(u16, U16),
                consts::CastToU32 => cast_to!(u32, U32),
                consts::CastToU64 => cast_to!(u64, U64),
                consts::CastToFloat => cast_to!(f64, Float),

                _ => panic!("unreachable {value}"),
            };
        };


        self.externs.clear();
        let libraries = std::mem::take(&mut self.libraries);
        for library in libraries {
            unsafe {
                let shutdown: ExternFunction = match library.get(b"_shutdown") {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                shutdown(self);
            }
        }

        
        if let Status::Err(e) = &result {
            println!(
                "{}",
                format!("panicked at '{}'", e.read_message().to_string_lossy()).bright_red()
            );
        }


        result
    }
}

#[allow(clippy::inline_always)]
#[allow(clippy::type_complexity)]
impl<'a> VM<'a> {
    #[inline(always)]
    fn binary_operation<A, B, C, D, E, F, G, H, I>(
        &mut self,
        operation_func: fn(&mut VM<'a>, (u8, u8, u8), A, B, C, D, E, F, G, H, I),

        (
            i8_func ,
            i16_func,
            i32_func,
            i64_func,
            u8_func ,
            u16_func,
            u32_func,
            u64_func,
        ): (A, B, C, D, E, F, G, H),

        float_func: I,
    ) {
        let dst = self.current.next();
        let v1  = self.current.next();
        let v2  = self.current.next();

        operation_func(self, (dst, v1, v2),
            i8_func,
            i16_func,
            i32_func,
            i64_func,
            u8_func,
            u16_func,
            u32_func,
            u64_func,

            float_func
        );
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn arithmetic_operation(
        &mut self,
        (dst, v1, v2): (u8, u8, u8),

        i8_func:  fn(i8 , i8 ) -> i8 ,
        i16_func: fn(i16, i16) -> i16,
        i32_func: fn(i32, i32) -> i32,
        i64_func: fn(i64, i64) -> i64,
        u8_func:  fn(u8,  u8 ) -> u8 ,
        u16_func: fn(u16, u16) -> u16,
        u32_func: fn(u32, u32) -> u32,
        u64_func: fn(u64, u64) -> u64,

        float_func: fn(f64, f64) -> f64,
    ) {
        let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
            (VMData::I8(v1),  VMData::I8(v2))  => VMData::I8(i8_func(v1, v2)),
            (VMData::I16(v1), VMData::I16(v2)) => VMData::I16(i16_func(v1, v2)),
            (VMData::I32(v1), VMData::I32(v2)) => VMData::I32(i32_func(v1, v2)),
            (VMData::I64(v1), VMData::I64(v2)) => VMData::I64(i64_func(v1, v2)),
            (VMData::U8(v1),  VMData::U8(v2))  => VMData::U8(u8_func(v1, v2)),
            (VMData::U16(v1), VMData::U16(v2)) => VMData::U16(u16_func(v1, v2)),
            (VMData::U32(v1), VMData::U32(v2)) => VMData::U32(u32_func(v1, v2)),
            (VMData::U64(v1), VMData::U64(v2)) => VMData::U64(u64_func(v1, v2)),

            (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(float_func(v1, v2)),

            _ => unreachable!(),
        };

        self.stack.set_reg(dst, val);
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn comparisson_operation(
        &mut self,
        (dst, v1, v2): (u8, u8, u8),

        i8_func:  fn(&i8 , &i8 ) -> bool,
        i16_func: fn(&i16, &i16) -> bool,
        i32_func: fn(&i32, &i32) -> bool,
        i64_func: fn(&i64, &i64) -> bool,
        u8_func:  fn(&u8,  &u8 ) -> bool,
        u16_func: fn(&u16, &u16) -> bool,
        u32_func: fn(&u32, &u32) -> bool,
        u64_func: fn(&u64, &u64) -> bool,

        float_func: fn(&f64, &f64) -> bool,
    ) {
        let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
            (VMData::I8(v1),  VMData::I8(v2))  => VMData::Bool(i8_func(&v1, &v2)),
            (VMData::I16(v1), VMData::I16(v2)) => VMData::Bool(i16_func(&v1, &v2)),
            (VMData::I32(v1), VMData::I32(v2)) => VMData::Bool(i32_func(&v1, &v2)),
            (VMData::I64(v1), VMData::I64(v2)) => VMData::Bool(i64_func(&v1, &v2)),
            (VMData::U8(v1),  VMData::U8(v2))  => VMData::Bool(u8_func(&v1, &v2)),
            (VMData::U16(v1), VMData::U16(v2)) => VMData::Bool(u16_func(&v1, &v2)),
            (VMData::U32(v1), VMData::U32(v2)) => VMData::Bool(u32_func(&v1, &v2)),
            (VMData::U64(v1), VMData::U64(v2)) => VMData::Bool(u64_func(&v1, &v2)),

            (VMData::Float(v1), VMData::Float(v2)) => VMData::Bool(float_func(&v1, &v2)),

            _ => unreachable!(),
        };

        self.stack.set_reg(dst, val);
    }
}


#[inline(always)]
fn convert_usize_to_u8(v: usize) -> u8 {
    v.try_into().expect("usize overflows a u8")
}
