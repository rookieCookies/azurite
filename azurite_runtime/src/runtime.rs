use azurite_common::consts;
use libloading::{Library, Symbol};

use crate::{VMData, object_map::Object, VM, Code};
use std::ops::{Add, Sub, Mul, Div};

impl VM {
    pub(crate) fn run(&mut self, mut current: Code) {
        let mut callstack = Vec::with_capacity(64);

        // SAFETY: `external_funcs` must be dropped before `libraries`
        let mut libraries = vec![];
        let mut external_funcs = vec![];

        loop {
            let value = current.next();

            match value {
                consts::ExternFile => {
                    let path = current.string();
                    let func_amount = current.next();
                    
                    unsafe {
                        let lib = Library::new(path).unwrap();

                        for _ in 0..func_amount {
                            let name = current.string();
                            // println!("{name}");
                            let func : Symbol<'_, unsafe extern fn(&mut VM)> = lib.get(name.as_bytes()).unwrap();
                            external_funcs.push(func.into_raw());
                        }
                        
                        libraries.push(lib);
                    }
                    
                },
                
                consts::Copy => {
                    let src = current.next();
                    let dst = current.next();

                    let data = self.stack.reg(src);
                    self.stack.set_reg(dst, data);
                },

                
                consts::Swap => {
                    let v1 = current.next();
                    let v2 = current.next();

                    self.stack.values.swap(v1 as usize, v2 as usize);
                }

                
                consts::Add => self.binary_operation(&mut current, VM::arithmetic_operation, i64::checked_add, f64::add),
                consts::Subtract => self.binary_operation(&mut current, VM::arithmetic_operation, i64::checked_sub, f64::sub),
                consts::Multiply => self.binary_operation(&mut current, VM::arithmetic_operation, i64::checked_mul, f64::mul),
                consts::Divide => self.binary_operation(&mut current, VM::arithmetic_operation, i64::checked_div, f64::div),
                
                consts::Equals => self.binary_operation(&mut current, VM::comparisson_operation, i64::eq, f64::eq),
                consts::NotEquals => self.binary_operation(&mut current, VM::comparisson_operation, i64::ne, f64::ne),
                consts::GreaterThan => self.binary_operation(&mut current, VM::comparisson_operation, i64::gt, f64::gt),
                consts::LesserThan => self.binary_operation(&mut current, VM::comparisson_operation, i64::lt, f64::lt),
                consts::GreaterEquals => self.binary_operation(&mut current, VM::comparisson_operation, i64::ge, f64::ge),
                consts::LesserEquals => self.binary_operation(&mut current, VM::comparisson_operation, i64::le, f64::le),

                
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
                    current.check(1 + 4 + 4);
                    
                    let condition = current.next();
                    let if_true = current.u32();
                    let if_false = current.u32();

                    let val = match self.stack.reg(condition) {
                        VMData::Bool(v) => v,
                        _ => unreachable!()
                    };

                    if val {
                        current.goto(if_true as usize)
                    } else {
                        current.goto(if_false as usize)
                    }
                }


                consts::Return => {
                    if callstack.is_empty() {
                        break
                    }

                    let ret_val = self.stack.reg(0);
                    let ret_reg = current.return_to;
                    
                    current = callstack.pop().unwrap();
                    self.stack.set_stack_offset(current.offset);

                    self.stack.set_reg(ret_reg, ret_val);
                    self.stack.pop(1);
                },
                

                consts::Call => {
                    let goto = current.u32();
                    let dst = current.next();
                    let arg_count = current.next() as usize;

                    self.stack.push(arg_count + 1);
                    
                    let temp = self.stack.top - arg_count - self.stack.stack_offset;
                    for v in 0..arg_count {
                        let reg = self.stack.reg(current.next());
                        self.stack.set_reg((temp + v) as u8, reg)
                    }

                    let mut code = Code::new(current.code, self.stack.top - arg_count - 1, dst );
                    code.pointer = goto as usize;
                    
                    callstack.push(current);
                    current = code;
                    
                    self.stack.set_stack_offset(current.offset);
                }

                
                consts::ExtCall => {
                    let index = current.u32();
                    let dst = current.next();
                    let arg_count = current.next() as usize;

                    self.stack.push(arg_count + 1);
                    
                    let temp = self.stack.top - arg_count - self.stack.stack_offset;
                    for v in 0..arg_count {
                        let reg = self.stack.reg(current.next());
                        self.stack.set_reg((temp + v) as u8, reg)
                    }

                    self.stack.set_stack_offset(self.stack.top - arg_count - 1);
                    
                    unsafe { external_funcs[index as usize](self); };

                    let ret_val = self.stack.reg(0);
                    self.stack.set_stack_offset(current.offset);
                    
                    self.stack.set_reg(dst, ret_val);
                }

                consts::Push => {
                    let amount = current.next();
                    self.stack.push(amount as usize)
                }


                consts::Pop => {
                    let amount = current.next();
                    self.stack.pop(amount as usize)
                }


                consts::Unit => {
                    let reg = current.next();

                    #[cfg(debug_assertions)]
                    self.stack.set_reg(reg, VMData::Empty)
                }


                consts::Struct => {
                    let dst = current.next();
                    let r1 = current.next();
                    let r2 = current.next();

                    let vec = self.stack.values[(self.stack.stack_offset + r1 as usize)..=(self.stack.stack_offset + r2 as usize)].to_vec();
                    
                    let index = self.objects.put(Object::Struct(vec)).unwrap();
                    self.stack.set_reg(dst, VMData::Object(index as u64))
                }


                consts::AccStruct => {
                    let dst = current.next();
                    let struct_at = current.next();
                    let index = current.next();

                    let val = self.stack.reg(struct_at);
                    let obj = match val {
                        VMData::Object(v) => self.objects.get(v as usize),

                        _ => unreachable!()
                    };

                    let accval = match obj {
                        Object::Struct(v) => v[index as usize],
                        
                        _ => unreachable!()
                    };

                    self.stack.set_reg(dst, accval);
                }


                consts::SetField => {
                    let struct_at = current.next();
                    let data = current.next();
                    let index = current.next();

                    let data = self.stack.reg(data);

                    let val = self.stack.reg(struct_at);
                    let obj = match val {
                        VMData::Object(v) => self.objects.get_mut(v as usize),

                        _ => unreachable!()
                    };

                    match obj {
                        Object::Struct(v) => v[index as usize] = data,
                        
                        _ => unreachable!()
                    };
                }
                
                _ => panic!("unreachable {value}"),
            };

        };
    }
}


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
        
        operation_func(self, (dst, v1, v2), int_func, float_func)
        
    }


    #[inline(always)]
    fn arithmetic_operation(
        &mut self,
        (dst, v1, v2): (u8, u8, u8),
        int_func: fn(i64, i64) -> Option<i64>,
        float_func: fn(f64, f64) -> f64
    ) {
        let val = match (self.stack.reg(v1), self.stack.reg(v2)) {
            (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Integer(int_func(v1, v2).unwrap()),
            (VMData::Float(v1), VMData::Float(v2))     => VMData::Float(float_func(v1, v2)),

            _ => panic!("{} {} {}", self.stack.stack_offset, v1, v2)
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
            (VMData::Float(v1), VMData::Float(v2))     => VMData::Bool(float_func(&v1, &v2)),

            _ => unreachable!()
            
        };

        self.stack.set_reg(dst, val)
    }
}