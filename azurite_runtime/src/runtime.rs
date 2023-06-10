use azurite_common::consts;
use colored::Colorize;
use libloading::{Library, Symbol};

use crate::{object_map::{Object, Structure}, Code, FatalError, Status, VMData, VM};
use std::ops::{Add, Mul, Sub};

type ExternFunction<'a> = Symbol<'a, unsafe extern "C" fn(&mut VM) -> Status>;

impl VM {
    #[allow(clippy::too_many_lines)]
    #[inline(never)]
    pub(crate) fn run(&mut self, mut current: Code) -> Status {
        let mut callstack = Vec::with_capacity(64);

        // SAFETY: `external_funcs` must be dropped before `libraries`
        let mut libraries = vec![];
        let mut external_funcs = vec![];

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
                            let name = current.string();
                            let Ok(func) = lib.get::<ExternFunction<'_>>(name.as_bytes()) else { break 'global Status::Err(FatalError::new(format!("can't find a function named {name} in {path}"))); };

                            external_funcs.push(func.into_raw());
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
                    i64::wrapping_add,
                    f64::add,
                ),

                
                consts::Subtract => self.binary_operation(
                    &mut current,
                    VM::arithmetic_operation,
                    i64::wrapping_sub,
                    f64::sub,
                ),

                
                consts::Multiply => self.binary_operation(
                    &mut current,
                    VM::arithmetic_operation,
                    i64::wrapping_mul,
                    f64::mul,
                ),

                
                consts::Divide => {
                    let dst = current.next();
                    let v1 = current.next();
                    let v2 = current.next();

                    let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => {
                            if v2 == 0 {
                                break Status::Err(FatalError::new(String::from(
                                    "division by zero",
                                )));
                            }

                            VMData::Integer(v1.wrapping_div(v2))
                        }

                        (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(v1 / v2),

                        _ => unreachable!(),
                    };

                    self.stack.set_reg(dst, val);
                }


                consts::GreaterThan   => self.binary_operation(&mut current, VM::comparisson_operation, i64::gt, f64::gt),
                consts::LesserThan    => self.binary_operation(&mut current, VM::comparisson_operation, i64::lt, f64::lt),
                consts::GreaterEquals => self.binary_operation(&mut current, VM::comparisson_operation, i64::ge, f64::ge),
                consts::LesserEquals  => self.binary_operation(&mut current, VM::comparisson_operation, i64::le, f64::le),


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

                    let val = self.stack.reg(condition).bool();

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

                    let result = unsafe { external_funcs[index as usize](self) };

                    if result.is_err() {
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
                        VMData::Integer(v) => self.stack.set_reg(dst, VMData::Integer(-v)),
                        VMData::Float(v) => self.stack.set_reg(dst, VMData::Float(-v)),

                        _ => unreachable!(),
                    }
                }


                consts::UnaryNot => {
                    let dst = current.next();
                    let val = current.next();

                    let data = self.stack.reg(val).bool();
                    self.stack.set_reg(dst, VMData::Bool(!data))
                }

                _ => panic!("unreachable {value}"),
            };
        };

        if let Status::Err(e) = result {
            println!(
                "{}",
                format!("panicked at '{}'", e.read_message().to_string_lossy()).bright_red()
            );
        }

        for library in libraries {
            unsafe {
                let shutdown: ExternFunction = match library.get(b"_shutdown") {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                shutdown(self);
            }
        }

        Status::Ok
    }
}

#[allow(clippy::inline_always)]
impl VM {
    #[inline(always)]
    fn binary_operation<T, V>(
        &mut self,
        code: &mut Code,

        operation_func: fn(&mut VM, (u8, u8, u8), T, V),
        int_func: T,
        float_func: V,
    ) {
        let dst = code.next();
        let v1 = code.next();
        let v2 = code.next();

        operation_func(self, (dst, v1, v2), int_func, float_func);
    }

    #[inline(always)]
    fn arithmetic_operation(
        &mut self,
        (dst, v1, v2): (u8, u8, u8),
        int_func: fn(i64, i64) -> i64,
        float_func: fn(f64, f64) -> f64,
    ) {
        let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
            (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Integer(int_func(v1, v2)),
            (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(float_func(v1, v2)),

            _ => unreachable!(),
        };

        self.stack.set_reg(dst, val);
    }

    #[inline(always)]
    fn comparisson_operation(
        &mut self,
        (dst, v1, v2): (u8, u8, u8),
        int_func: fn(&i64, &i64) -> bool,
        float_func: fn(&f64, &f64) -> bool,
    ) {
        let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
            (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Bool(int_func(&v1, &v2)),
            (VMData::Float(v1), VMData::Float(v2)) => VMData::Bool(float_func(&v1, &v2)),

            _ => unreachable!(),
        };

        self.stack.set_reg(dst, val);
    }
}
