#![allow(clippy::inline_always)]
#![allow(clippy::unnecessary_wraps)]

use std::mem::size_of;

use azurite_common::Bytecode;
#[cfg(feature = "hotspot")]
use fxhash::FxBuildHasher;
#[cfg(feature = "hotspot")]
use std::collections::HashMap;
#[cfg(feature = "hotspot")]
use std::time::Instant;

// TODO: Eventually make it so the code can't panic even with corrupted bytecode

use crate::{
    get_vm_memory, native_library, object_map::ObjectMap, runtime_error::RuntimeError, Object,
    ObjectData, VMData,
};

pub struct VM {
    pub objects: ObjectMap,
    pub constants: Vec<VMData>,
    pub stack: Stack,
    pub functions: Vec<Function>,

    #[cfg(feature = "hotspot")]
    pub hotspots: HashMap<Bytecode, (usize, f64), FxBuildHasher>,
}

#[must_use]
pub fn corrupt_bytecode() -> RuntimeError {
    RuntimeError::new(0, "corrupt bytecode")
}

impl VM {
    /// # Errors
    /// This function will return an error if `get_vm_memory`
    /// returns an error
    pub fn new() -> Result<Self, RuntimeError> {
        Ok(Self {
            constants: vec![],
            stack: Stack::new(),
            functions: Vec::with_capacity(16),
            objects: ObjectMap::with_capacity(get_vm_memory()?),

            #[cfg(feature = "hotspot")]
            hotspots: HashMap::with_capacity_and_hasher(32, FxBuildHasher::default()),
        })
    }

    /// # Errors
    /// # Panics
    pub fn run(&mut self, code: &[u8]) -> Result<(), RuntimeError> {
        let mut callstack: Vec<Code> = Vec::with_capacity(128);
        let mut current = Code {
            bytecode: code,
            index: 0,
            stack_offset: 0,
            has_return: false,
        };

        loop {
            #[cfg(feature = "bytecode")]
            {
                let value = Bytecode::from_u8(current.bytecode[current.index]);
                println!("{value:?}");
            }
            let value = match Bytecode::from_u8(*current.next()) {
                Some(v) => v,
                None => return Err(RuntimeError::new(current.index as u64, "the file ended before a return")),
            };
            match value {
                Bytecode::EqualsTo => self.op_equals_to(&mut current),
                Bytecode::NotEqualsTo => self.op_not_equals_to(&mut current),
                Bytecode::GreaterThan => self.op_greater_than(&mut current),
                Bytecode::LesserThan => self.op_lesser_than(&mut current),
                Bytecode::GreaterEquals => self.op_greater_equals(&mut current),
                Bytecode::LesserEquals => self.op_lesser_equals(&mut current),
                Bytecode::JumpIfFalse => self.op_jump_if_false(&mut current),
                Bytecode::Jump => self.op_jump(&mut current),
                Bytecode::JumpBack => self.op_jump_back(&mut current),
                Bytecode::LoadFunction => self.op_load_function(&mut current),
                Bytecode::LoadConst => self.op_load_const(&mut current),
                Bytecode::Add => self.op_add(&mut current),
                Bytecode::Subtract => self.op_subtract(&mut current),
                Bytecode::Multiply => self.op_multiply(&mut current),
                Bytecode::Divide => self.op_divide(&mut current),
                Bytecode::GetVar => self.op_get_variable(&mut current),
                Bytecode::GetVarFast => self.op_get_variable_fast(&mut current),
                Bytecode::ReplaceVarFast => self.op_replace_variable_fast(&mut current),
                Bytecode::ReplaceVar => self.op_replace_variable(&mut current),
                Bytecode::ReplaceVarInObject => self.op_replace_variable_in_object(&mut current),
                Bytecode::Not => self.op_not(&mut current),
                Bytecode::Negative => self.op_negative(&mut current),
                Bytecode::Pop => self.op_pop(&mut current),
                Bytecode::PopMulti => self.op_pop_multi(&mut current),
                Bytecode::CreateStruct => self.op_create_struct(&mut current),
                Bytecode::AccessData => self.op_access_data(&mut current),
                Bytecode::CallFunction => {
                    let index = *current.next() as usize;
                    let function = match self.functions.get(index) {
                        Some(v) => v,
                        None => return Err(RuntimeError::new(current.index as u64, "tried to call a none-existant function")),
                    };
                    let (argument_count, has_return) =
                        (function.argument_count as usize, function.has_return);

                    let function_code = Code {
                        bytecode: code
                            .get(function.start..function.start + function.size)
                            .unwrap(),
                        index: 0,
                        stack_offset: self.stack.top - argument_count,
                        has_return,
                    };

                    callstack.push(current);
                    current = function_code;
                    Ok(())
                }
                Bytecode::ReturnFromFunction | Bytecode::Return => {
                    if current.has_return {
                        let return_value = self.stack.top-1;

                        self.stack
                            .pop_multi_ignore(self.stack.top - current.stack_offset - 1);
                        self.stack.swap_top_with(return_value);
                        self.stack.step();
                    } else {
                        self.stack
                            .pop_multi_ignore(self.stack.top - current.stack_offset);
                    }

                    if callstack.is_empty() {
                        return Ok(());
                    }
                    current = callstack.pop().unwrap();
                    Ok(())
                }
                Bytecode::RawCall => {
                    let index = current.next();
                    // let stack_current = self.stack.top;
                    // println!("{index}");
                    native_library::RAW_FUNCTIONS[*index as usize]((self, &mut current))?;
                    // debug_assert!(self.stack.top - stack_current == 0);
                    Ok(())
                }
            }?;

            #[cfg(feature = "stack")]
            {
                // let value = Bytecode::from_u8(code.code[code.index]);
                print!("        ");
                (0..self.stack.top).for_each(|x| print!("[{:?}]", self.stack.data[x]));
                println!()
            }

            #[cfg(feature = "objects")]
            {
                // let value = Bytecode::from_u8(code.code[code.index]);
                print!("        ");
                self.objects.data.iter().filter(|x| !matches!(x.data, ObjectData::Free { .. })).for_each(|x| print!("{{{:?}}}", x));
                println!()
            }
        }
    }

    /// # Panics
    /// Panics if the object index is out of bounds
    #[inline(always)]
    #[must_use]
    pub fn get_object(&self, index: usize) -> &Object {
        self.objects.get(index).unwrap()
    }

    /// # Errors
    /// If the VM is out of memory this function will try running
    /// the garbage collector and try pushing again, if that fails
    /// it will return a `out of memory` error
    #[inline(always)]
    pub fn create_object(&mut self, object: Object) -> Result<usize, RuntimeError> {
        match self.objects.push(object) {
            Ok(v) => Ok(v),
            Err(obj) => {
                self.collect_garbage();
                match self.objects.push(obj) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(RuntimeError::new(0, "out of memory")),
                }
            }
        }
    }
}

#[allow(clippy::unused_self)]
impl VM {
    #[inline(always)]
    fn op_load_const(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let index = *code.next();
        self.stack.push(self.constants[index as usize].clone())?;
        Ok(())
    }

    #[inline(always)]
    fn op_add(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let values = self.stack.pop_two();
        let result = static_add(values.1, values.0)?;
        self.stack.push(result)?;
        Ok(())
    }

    #[inline(always)]
    fn op_subtract(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let values = self.stack.pop_two();
        let result = static_sub(values.1, values.0)?;
        self.stack.push(result)?;
        Ok(())
    }

    #[inline(always)]
    fn op_multiply(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let values = self.stack.pop_two();
        let result = static_mul(values.1, values.0)?;
        self.stack.push(result)?;
        Ok(())
    }

    #[inline(always)]
    fn op_divide(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let values = self.stack.pop_two();
        let result = static_div(values.1, values.0)?;
        self.stack.push(result)?;
        Ok(())
    }

    #[inline(always)]
    fn op_get_variable_fast(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let index = *code.next();
        debug_assert!(self.stack.top > index as usize);
        self.stack.push(
            self.stack
                .data
                .get(code.stack_offset + index as usize)
                .unwrap()
                .clone(),
        )?;
        Ok(())
    }

    #[inline(always)]
    fn op_get_variable(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let index = u16::from_le_bytes([*code.next(), *code.next()]);
        self.stack
            .push(self.stack.data[code.stack_offset + index as usize].clone())?;
        Ok(())
    }

    #[inline(always)]
    fn op_replace_variable_fast(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let index = *code.next();
        self.stack.swap_top_with(index as usize);
        Ok(())
    }

    #[inline(always)]
    fn op_replace_variable(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let index = u16::from_le_bytes([*code.next(), *code.next()]);
        self.stack.swap_top_with(index as usize);
        Ok(())
    }

    #[inline(always)]
    fn op_replace_variable_in_object(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let size = *code.next();
        let data = self.stack.pop().clone();
        let mut object = self.stack.data.get_mut(*code.next() as usize).unwrap();
        for _ in 0..(size - 1) {
            object = match object {
                VMData::Object(v) => match &mut unsafe {
                    &mut *(self.objects.get_mut(*v as usize).unwrap() as *mut Object)
                }
                .data
                {
                    ObjectData::Struct(v) => v.get_mut(*code.next() as usize).unwrap(),
                    _ => return Err(corrupt_bytecode()),
                },
                _ => return Err(corrupt_bytecode()),
            };
        }
        *object = data;
        Ok(())
    }

    #[inline(always)]
    fn op_not(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let value = match self.stack.pop() {
            VMData::Bool(v) => VMData::Bool(!v),
            _ => return Err(corrupt_bytecode()),
        };
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_negative(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let value = match self.stack.pop() {
            VMData::Integer(v) => VMData::Integer(-v),
            VMData::Float(v) => VMData::Float(-v),
            _ => return Err(corrupt_bytecode()),
        };
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_pop(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        self.stack.pop_multi_ignore(1);
        Ok(())
    }

    #[inline(always)]
    fn op_pop_multi(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        self.stack.pop_multi_ignore(*code.next() as usize);
        Ok(())
    }

    #[inline(always)]
    fn op_equals_to(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let popped = self.stack.pop_two();
        let value = VMData::Bool(match (&popped.1, &popped.0) {
            (VMData::Integer(v1), VMData::Integer(v2)) => v1 == v2,
            (VMData::Float(v1), VMData::Float(v2)) => (v1 - v2).abs() < f64::EPSILON,
            (VMData::Bool(v1), VMData::Bool(v2)) => v1 == v2,
            (VMData::Object(v1), VMData::Object(v2)) => {
                let (v1, v2) = (*v1, *v2);
                self.get_object(v1 as usize) == self.get_object(v2 as usize)
            }
            _ => return Err(corrupt_bytecode()),
        });
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_not_equals_to(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let popped = self.stack.pop_two();
        let value = VMData::Bool(match (&popped.1, &popped.0) {
            (VMData::Integer(v1), VMData::Integer(v2)) => v1 != v2,
            (VMData::Float(v1), VMData::Float(v2)) => (v1 - v2).abs() > f64::EPSILON,
            (VMData::Bool(v1), VMData::Bool(v2)) => v1 != v2,
            (VMData::Object(v1), VMData::Object(v2)) => v1 != v2,
            _ => return Err(corrupt_bytecode()),
        });
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_greater_than(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let popped = self.stack.pop_two();
        let value = VMData::Bool(match (&popped.1, &popped.0) {
            (VMData::Integer(v1), VMData::Integer(v2)) => v1 > v2,
            (VMData::Float(v1), VMData::Float(v2)) => v1 > v2,
            _ => return Err(corrupt_bytecode()),
        });
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_lesser_than(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let popped = self.stack.pop_two();
        let value = VMData::Bool(match (&popped.1, &popped.0) {
            (VMData::Integer(v1), VMData::Integer(v2)) => v1 < v2,
            (VMData::Float(v1), VMData::Float(v2)) => v1 < v2,
            _ => return Err(corrupt_bytecode()),
        });
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_greater_equals(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let popped = self.stack.pop_two();
        let value = VMData::Bool(match (&popped.1, &popped.0) {
            (VMData::Integer(v1), VMData::Integer(v2)) => v1 >= v2,
            (VMData::Float(v1), VMData::Float(v2)) => v1 >= v2,
            _ => return Err(corrupt_bytecode()),
        });
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_lesser_equals(&mut self, _code: &mut Code) -> Result<(), RuntimeError> {
        let popped = self.stack.pop_two();
        let value = VMData::Bool(match (&popped.1, &popped.0) {
            (VMData::Integer(v1), VMData::Integer(v2)) => v1 <= v2,
            (VMData::Float(v1), VMData::Float(v2)) => v1 <= v2,
            _ => return Err(corrupt_bytecode()),
        });
        self.stack.push(value)?;
        Ok(())
    }

    #[inline(always)]
    fn op_jump_if_false(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let condition = match self.stack.pop() {
            VMData::Bool(v) => v,
            _ => return Err(corrupt_bytecode()),
        };
        let amount = *code.next() as usize;
        if !condition {
            code.skip(amount);
        }
        Ok(())
    }

    #[inline(always)]
    fn op_jump(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let i = *code.next() as usize;
        code.skip(i);
        Ok(())
    }

    #[inline(always)]
    fn op_jump_back(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let amount = *code.next() as usize;
        code.back_skip(amount);
        Ok(())
    }

    #[inline(always)]
    fn op_load_function(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let arg_count = *code.next();
        let has_return = *code.next() == 1;
        let amount = *code.next() as usize;
        self.functions.push(Function {
            start: code.index,
            argument_count: arg_count,
            has_return,
            size: amount,
        });
        code.skip(amount);
        Ok(())
    }

    #[inline(always)]
    fn op_create_struct(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let amount_of_variables = *code.next() as usize;
        let mut data = Vec::with_capacity(amount_of_variables);
        for _ in 0..amount_of_variables {
            data.push(self.stack.pop().clone());
        }
        let object_index = self.create_object(Object::new(ObjectData::Struct(data)));
        self.stack.push(VMData::Object(match object_index {
            Ok(v) => v,
            Err(mut err) => {
                err.bytecode_index = code.index as u64;
                return Err(err);
            }
        } as u64))?;
        Ok(())
    }

    #[inline(always)]
    fn op_access_data(&mut self, code: &mut Code) -> Result<(), RuntimeError> {
        let data = self.stack.pop();
        let index = code.next();
        let object = match data {
            VMData::Object(v) => *v,
            _ => return Err(corrupt_bytecode()),
        };
        match &self.get_object(object as usize).data {
            ObjectData::Struct(v) => self.stack.push({
                v.get(*index as usize).unwrap().clone()
            })?,
            _ => return Err(corrupt_bytecode()),
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Code<'a> {
    pub bytecode: &'a [u8],
    pub index: usize,
    pub stack_offset: usize,
    pub has_return: bool,
}

impl Code<'_> {
    #[inline(always)]
    #[must_use]
    fn next(&mut self) -> &u8 {
        // let index = self.index;
        // unsafe {
        //     self.index = self.index.unchecked_add(1);
        // }
        self.index += 1;
        // unsafe { self.bytecode.get_unchecked(index) }

        self.bytecode.get(self.index - 1).unwrap()
    }

    #[inline(always)]
    fn skip(&mut self, amount: usize) {
        self.index += amount;
    }

    #[inline(always)]
    fn back_skip(&mut self, amount: usize) {
        self.index -= amount;
    }
}

#[derive(Debug)]
pub struct Stack {
    pub data: [VMData; 5000 / size_of::<VMData>()],
    pub top: usize,
}

impl Stack {
    fn new() -> Self {
        let mut stack = Vec::with_capacity(5000 / size_of::<VMData>());
        stack.resize(5000 / size_of::<VMData>(), VMData::Integer(0));
        Self {
            data: stack.try_into().unwrap(),
            top: 0,
        }
    }

    /// # Errors
    /// This function will error if the VM is out of memory
    #[inline(always)]
    pub fn push(&mut self, value: VMData) -> Result<(), RuntimeError> {
        *match self.data.get_mut(self.top) {
            Some(v) => v,
            None => return Err(RuntimeError::new(0, "stack overflow")),
        } = value;
        self.step();
        Ok(())
    }

    #[inline(always)]
    pub fn step(&mut self) {
        // debug_assert!(self.top.checked_add(1).is_some());
        // unsafe {
        //     self.top = self.top.unchecked_add(1);
        // }
        self.top += 1;
    }

    #[inline(always)]
    fn step_back(&mut self) {
        self.top -= 1;
    }

    /// # Panics
    /// This function will panic if it tries to pop an
    /// empty stack. The compiler should never output code
    /// like this
    #[inline(always)]
    #[must_use]
    pub fn pop(&mut self) -> &VMData {
        self.step_back();
        self.data.get(self.top).unwrap()
    }


    /// # Errors
    /// This function will error if the data is not a valid
    /// index in the vector
    #[inline(always)]
    pub fn view_behind(&self, amount: usize) -> Result<&VMData, RuntimeError> {
        match self.data.get(self.top - amount) {
            Some(v) => Ok(v),
            None => Err(RuntimeError::new(0, "tried to view data behind that's none existant")),
        }
    }

    #[inline(always)]
    #[must_use]
    fn pop_two(&mut self) -> (&VMData, &VMData) {
        self.top -= 2;

        (
            self.data.get(self.top + 1).unwrap(),
            self.data.get(self.top).unwrap()
        )
    }

    #[inline(always)]
    fn pop_multi_ignore(&mut self, amount: usize) {
        debug_assert!(self.top.checked_sub(amount).is_some());
        // unsafe {
        //     self.top = self.top.unchecked_sub(amount);
        // }
        self.top -= amount;
    }

    #[inline(always)]
    fn swap_top_with(&mut self, index: usize) {
        self.step_back();
        // unsafe { self.data.swap_unchecked(index, self.top) };
        self.data.swap(index, self.top);
    }
}

#[inline(always)]
fn static_add(data1: &VMData, data2: &VMData) -> Result<VMData, RuntimeError> {
    Ok(match (data1, data2) {
        (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Integer(v1 + v2),
        (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(v1 + v2),
        _ => return Err(corrupt_bytecode()),
    })
}

#[inline(always)]
fn static_sub(data1: &VMData, data2: &VMData) -> Result<VMData, RuntimeError> {
    Ok(match (data1, data2) {
        (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Integer(v1 - v2),
        (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(v1 - v2),
        _ => return Err(corrupt_bytecode()),
    })
}

#[inline(always)]
fn static_mul(data1: &VMData, data2: &VMData) -> Result<VMData, RuntimeError> {
    Ok(match (data1, data2) {
        (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Integer(v1 * v2),
        (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(v1 * v2),
        _ => return Err(corrupt_bytecode()),
    })
}

#[inline(always)]
fn static_div(data1: &VMData, data2: &VMData) -> Result<VMData, RuntimeError> {
    Ok(match (data1, data2) {
        (VMData::Integer(v1), VMData::Integer(v2)) => VMData::Integer(v1 / v2),
        (VMData::Float(v1), VMData::Float(v2)) => VMData::Float(v1 / v2),
        _ => return Err(corrupt_bytecode()),
    })
}

#[derive(Debug)]
pub struct Function {
    start: usize,
    argument_count: u8,
    size: usize,
    has_return: bool,
}
