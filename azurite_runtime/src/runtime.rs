use azurite_common::consts;
use colored::Colorize;
use libloading::{Library, Symbol};

use crate::{object_map::{Object, Structure}, Code, FatalError, Status, VMData, VM};
use std::ops::{Add, Mul, Sub};

type ExternFunction<'a> = Symbol<'a, ExternFunctionRaw>;
type ExternFunctionRaw = unsafe extern "C" fn(&mut VM) -> Status;


impl VM {
    #[allow(clippy::too_many_lines)]
    #[inline(never)]
    pub(crate) fn run(&mut self, mut current: Code) -> Status {
        macro_rules! cast_to {
            ($t: ty, $variant: ident) => { {
                let dst = current.next();
                let val = current.next();

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

        
        let mut callstack = Vec::with_capacity(64);

        // SAFETY: `external_funcs` must be dropped before `libraries`
        let mut libraries = vec![];
        let mut external_funcs : Vec<ExternFunctionRaw> = Vec::with_capacity(self.metadata.extern_count as usize);

        let result: Status = 'global: loop {
            let value = current.next();

            // println!("{} {:?}\n\t{:?}", current.pointer, azurite_common::Bytecode::from_u8(value).unwrap(), self.stack.values.iter().take(self.stack.top).collect::<Vec<_>>());
            match value {
                consts::ExternFile => {
                    let path = current.string();

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

                    let func_amount = current.next();

                    unsafe {
                        // let Ok(lib) = Library::new(&path) else { break Err(format!("can't find a runtime library file named {path}")); };
                        let lib = match Library::new(&path) {
                            Ok(v) => v,
                            Err(_) => {
                                let new_path = std::env::current_exe()
                                    .unwrap()
                                    .parent()
                                    .unwrap()
                                    .join("runtime")
                                    .join(&path);
                                match Library::new(&new_path) {
                                    Ok(v) => v,
                                    Err(_) => {
                                        break Status::Err(FatalError::new(format!(
                                            "can't find a runtime library file named {path}"
                                        )))
                                    }
                                }
                            }
                        };

                        for _ in 0..func_amount {
                            let index = current.u32();
                            let name = current.string();
                            let Ok(func) = lib.get::<ExternFunction<'_>>(name.as_bytes()) else { break 'global Status::Err(FatalError::new(format!("can't find a function named {name} in {path}"))); };

                            if index as usize > external_funcs.len() {
                                external_funcs.push(**func.into_raw());
                            } else {
                                external_funcs.insert(index as usize, **func.into_raw());
                            }
                        }

                        if let Ok(x) = lib.get::<ExternFunction<'_>>(b"_init") {
                            x(self);
                        }

                        libraries.push(lib);
                    }
                }


                consts::Copy => {
                    let dst = current.next();
                    let src = current.next();

                    let data = self.stack.reg(src);
                    self.stack.set_reg(dst, data);
                }

                
                consts::Swap => {
                    let v1 = current.next();
                    let v2 = current.next();

                    self.stack.values.swap(v1 as usize, v2 as usize);
                }


                consts::Add => self.binary_operation(
                    &mut current,
                    VM::arithmetic_operation,
                    i8 ::wrapping_add,
                    i16::wrapping_add,
                    i32::wrapping_add,
                    i64::wrapping_add,
                    u8 ::wrapping_add,
                    u16::wrapping_add,
                    u32::wrapping_add,
                    u64::wrapping_add,
                    f64::add,
                ),

                
                consts::Subtract => self.binary_operation(
                    &mut current,
                    VM::arithmetic_operation,
                    i8 ::wrapping_sub,
                    i16::wrapping_sub,
                    i32::wrapping_sub,
                    i64::wrapping_sub,
                    u8 ::wrapping_sub,
                    u16::wrapping_sub,
                    u32::wrapping_sub,
                    u64::wrapping_sub,
                    f64::sub,
                ),

                
                consts::Multiply => self.binary_operation(
                    &mut current,
                    VM::arithmetic_operation,
                    i8 ::wrapping_mul,
                    i16::wrapping_mul,
                    i32::wrapping_mul,
                    i64::wrapping_mul,
                    u8 ::wrapping_mul,
                    u16::wrapping_mul,
                    u32::wrapping_mul,
                    u64::wrapping_mul,
                    f64::mul,
                ),

                
                consts::Modulo => self.binary_operation(
                    &mut current,
                    VM::arithmetic_operation,
                    i8 ::wrapping_rem,
                    i16::wrapping_rem,
                    i32::wrapping_rem,
                    i64::wrapping_rem,
                    u8 ::wrapping_rem,
                    u16::wrapping_rem,
                    u32::wrapping_rem,
                    u64::wrapping_rem,
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
                    
                    let dst = current.next();
                    let v1 = current.next();
                    let v2 = current.next();

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


                consts::GreaterThan   => self.binary_operation(&mut current, VM::comparisson_operation, i8::gt, i16::gt, i32::gt, i64::gt, u8::gt, u16::gt, u32::gt, u64::gt, f64::gt),
                consts::LesserThan    => self.binary_operation(&mut current, VM::comparisson_operation, i8::lt, i16::lt, i32::lt, i64::lt, u8::lt, u16::lt, u32::lt, u64::lt, f64::lt),
                consts::GreaterEquals => self.binary_operation(&mut current, VM::comparisson_operation, i8::ge, i16::ge, i32::ge, i64::ge, u8::ge, u16::ge, u32::ge, u64::ge, f64::ge),
                consts::LesserEquals  => self.binary_operation(&mut current, VM::comparisson_operation, i8::le, i16::le, i32::le, i64::le, u8::le, u16::le, u32::le, u64::le, f64::le),


                consts::Equals => {
                    let dst = current.next();
                    let v1 = current.next();
                    let v2 = current.next();

                    let value = self.stack.reg(v1) == self.stack.reg(v2);
                    self.stack.set_reg(dst, VMData::Bool(value));
                }


                consts::NotEquals => {
                    let dst = current.next();
                    let v1 = current.next();
                    let v2 = current.next();

                    let value = self.stack.reg(v1) != self.stack.reg(v2);
                    self.stack.set_reg(dst, VMData::Bool(value));
                }


                consts::LoadConst => {
                    let dst = current.next();
                    let data_index = current.next();

                    let data = self.constants[data_index as usize];
                    self.stack.set_reg(dst, data);
                }


                consts::Jump => {
                    let jump_at = current.u32();

                    current.goto(jump_at as usize);
                }


                consts::JumpCond => {
                    let condition = current.next();
                    let if_true = current.u32();
                    let if_false = current.u32();

                    let val = self.stack.reg(condition).as_bool();

                    if val {
                        current.goto(if_true as usize);
                    } else {
                        current.goto(if_false as usize);
                    }
                }


                consts::Return => {
                    if callstack.is_empty() {
                        return Status::Ok;
                    }

                    let ret_val = self.stack.reg(0);
                    let ret_reg = current.return_to;

                    current = callstack.pop().unwrap();
                    self.stack.set_stack_offset(current.offset);

                    self.stack.set_reg(ret_reg, ret_val);
                    self.stack.pop(1);
                }


                consts::Call => {
                    let goto = current.u32();
                    let dst = current.next();
                    let arg_count = current.next() as usize;

                    if let Status::Err(e) = self.stack.push(arg_count + 1) {
                        break Status::Err(e);
                    }

                    let temp = self.stack.top - arg_count - self.stack.stack_offset;
                    for v in 0..arg_count {
                        let reg = self.stack.reg(current.next());
                        self.stack.set_reg((temp + v).try_into().unwrap(), reg);
                    }

                    let mut code = Code::new(current.code, self.stack.top - arg_count - 1, dst);
                    code.goto(goto as usize);

                    callstack.push(current);
                    current = code;

                    self.stack.set_stack_offset(current.offset);
                }


                consts::ExtCall => {
                    let index = current.u32();
                    let dst = current.next();
                    let arg_count = current.next() as usize;

                    if let Status::Err(e) = self.stack.push(arg_count + 1) {
                        break Status::Err(e);
                    }

                    let temp = self.stack.top - arg_count - self.stack.stack_offset;
                    for v in 0..arg_count {
                        let reg = self.stack.reg(current.next());
                        self.stack.set_reg((temp + v).try_into().unwrap(), reg);
                    }

                    self.stack.set_stack_offset(self.stack.top - arg_count - 1);

                    let function = external_funcs[index as usize];
                    let result = unsafe { function(self) };

                    
                    if result.is_exit() || result.is_err() {
                        break result;
                    }

                    let ret_val = self.stack.reg(0);
                    self.stack.set_stack_offset(current.offset);

                    self.stack.set_reg(dst, ret_val);
                    self.stack.pop(arg_count + 1);
                }


                consts::Push => {
                    let amount = current.next();
                    if let Status::Err(e) = self.stack.push(amount as usize) {
                        break Status::Err(e);
                    }
                }


                consts::Pop => {
                    let amount = current.next();
                    self.stack.pop(amount as usize);
                }


                consts::Unit => {
                    #[cfg(debug_assertions)]
                    {
                        let reg = current.next();
                        self.stack.set_reg(reg, VMData::Empty);
                    }
                }


                consts::Struct => {
                    let dst = current.next();
                    let amount = current.next();

                    let vec = (0..amount)
                        .map(|_| self.stack.reg(current.next()))
                        .collect();

                    let index = match self.create_object(Object::new(Structure::new(vec))) {
                        Ok(v) => v,
                        Err(e) => break Status::Err(e),
                    };
                    
                    self.stack.set_reg(dst, VMData::Object(index));
                }


                consts::AccStruct => {
                    let dst = current.next();
                    let struct_at = current.next();
                    let index = current.next();

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
                    let struct_at = current.next();
                    let data = current.next();
                    let index = current.next();

                    let data = self.stack.reg(data);

                    let val = self.stack.reg(struct_at);
                    let obj = match val {
                        VMData::Object(v) => self.objects.get_mut(v),

                        _ => unreachable!(),
                    };

                    obj.structure_mut().fields_mut()[index as usize] = data;
                }


                consts::UnaryNeg => {
                    let dst = current.next();
                    let val = current.next();

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
                    let dst = current.next();
                    let val = current.next();

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
impl VM {
    #[inline(always)]
    fn binary_operation<A, B, C, D, E, F, G, H, I>(
        &mut self,
        code: &mut Code,

        operation_func: fn(&mut VM, (u8, u8, u8), A, B, C, D, E, F, G, H, I),

        i8_func:  A,
        i16_func: B,
        i32_func: C,
        i64_func: D,
        u8_func:  E,
        u16_func: F,
        u32_func: G,
        u64_func: H,

        float_func: I,
    ) {
        let dst = code.next();
        let v1 = code.next();
        let v2 = code.next();

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
