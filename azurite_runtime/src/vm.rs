#![allow(clippy::inline_always)]
#![allow(clippy::unnecessary_wraps)]



use azurite_common::consts;
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
#[inline(always)]
pub fn corrupt_bytecode() -> RuntimeError {
    #[cfg(debug_assertions)]
    return RuntimeError::new(0, "corrupt bytecode");
    #[cfg(not(debug_assertions))]
    unsafe { std::hint::unreachable_unchecked() }
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
    #[allow(clippy::too_many_lines)]
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
                use azurite_common::Bytecode;
                let value = &Bytecode::from_u8(current.bytecode[current.index]).unwrap();
                println!("{:<max$}: {}{value:?}", current.index, "    | ".repeat(callstack.len()), max=code.len().to_string().len());
            }

            let value = current.next();
            match value {
                &consts::EqualsTo => {
                    let popped = self.stack.pop_two();
                    let value = VMData::Bool(match (&popped.1, &popped.0) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => v1 == v2,
                        (VMData::Float(v1), VMData::Float(v2)) => (v1 - v2).abs() < f64::EPSILON,
                        (VMData::Bool(v1), VMData::Bool(v2)) => v1 == v2,
                        (VMData::Object(v1), VMData::Object(v2)) => {
                            let (v1, v2) = (*v1, *v2);
                            self.get_object(v1 as usize).data == self.get_object(v2 as usize).data
                        }
                        _ => return Err(corrupt_bytecode()),
                    });
                    self.stack.push(value)?;
                }
                &consts::NotEqualsTo => {
                    let popped = self.stack.pop_two();
                    let value = VMData::Bool(match (&popped.1, &popped.0) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => v1 != v2,
                        (VMData::Float(v1), VMData::Float(v2)) => (v1 - v2).abs() > f64::EPSILON,
                        (VMData::Bool(v1), VMData::Bool(v2)) => v1 != v2,
                        (VMData::Object(v1), VMData::Object(v2)) => {
                            let (v1, v2) = (*v1, *v2);
                            self.get_object(v1 as usize).data != self.get_object(v2 as usize).data
                        }
                        _ => return Err(corrupt_bytecode()),
                    });
                    self.stack.push(value)?;
                }
                &consts::GreaterThan => {
                    let popped = self.stack.pop_two();
                    let value = VMData::Bool(match (&popped.1, &popped.0) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => v1 > v2,
                        (VMData::Float(v1), VMData::Float(v2)) => v1 > v2,
                        _ => return Err(corrupt_bytecode()),
                    });
                    self.stack.push(value)?;
                }
                &consts::LesserThan => {
                    let popped = self.stack.pop_two();
                    let value = VMData::Bool(match (&popped.1, &popped.0) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => v1 < v2,
                        (VMData::Float(v1), VMData::Float(v2)) => v1 < v2,
                        _ => return Err(corrupt_bytecode()),
                    });
                    self.stack.push(value)?;
                }
                &consts::GreaterEquals => {
                    let popped = self.stack.pop_two();
                    let value = VMData::Bool(match (&popped.1, &popped.0) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => v1 >= v2,
                        (VMData::Float(v1), VMData::Float(v2)) => v1 >= v2,
                        _ => return Err(corrupt_bytecode()),
                    });
                    self.stack.push(value)?;
                }
                &consts::LesserEquals => {
                    let popped = self.stack.pop_two();
                    let value = VMData::Bool(match (&popped.1, &popped.0) {
                        (VMData::Integer(v1), VMData::Integer(v2)) => v1 <= v2,
                        (VMData::Float(v1), VMData::Float(v2)) => v1 <= v2,
                        _ => return Err(corrupt_bytecode()),
                    });
                    self.stack.push(value)?;
                }
                &consts::Jump => {
                    let i = *current.next() as usize;
                    current.skip(i);
                }
                &consts::JumpIfFalse => {
                    let condition = match self.stack.pop() {
                        VMData::Bool(v) => v,
                        _ => return Err(corrupt_bytecode()),
                    };
                    let amount = *current.next() as usize;
                    if !condition {
                        current.skip(amount);
                    }
                }
                &consts::JumpBack => {
                    let i = *current.next() as usize;
                    current.back_skip(i);
                }
                &consts::JumpLarge => {
                    let amount = u16::from_le_bytes([*current.next(), *current.next()]) as usize;
                    current.skip(amount);
                }
                &consts::JumpIfFalseLarge => {
                    let condition = match self.stack.pop() {
                        VMData::Bool(v) => v,
                        _ => return Err(corrupt_bytecode()),
                    };
                    let amount = u16::from_le_bytes([*current.next(), *current.next()]) as usize;
                    if !condition {
                        current.skip(amount);
                    }
                }
                &consts::JumpBackLarge => {
                    let amount = u16::from_le_bytes([*current.next(), *current.next()]) as usize;
                    current.back_skip(amount);
                }
                &consts::LoadFunction => {
                    let arg_count = *current.next();
                    let has_return = *current.next() == 1;
                    let amount = *current.next() as usize;
                    self.functions.push(Function {
                        start: current.index,
                        argument_count: arg_count,
                        has_return,
                        size: amount,
                    });
                    current.skip(amount);
                }
                &consts::LoadConst => {
                    let index = *current.next();
                    self.stack.push(self.constants[index as usize])?;
                }
                &consts::LoadConstStr => {
                    let index = *current.next();
                    let string_index = match &self.constants[index as usize] {
                        VMData::Object(v) => *v,
                        _ => return Err(corrupt_bytecode())
                    };

                    let string_dup = match &self.objects.data.get(string_index as usize).unwrap().data {
                        ObjectData::String(v) => v,
                        _ => return Err(corrupt_bytecode())
                    };

                    let object_index = self.create_object(Object::new(ObjectData::String(string_dup.clone())))?;
                    self.stack.push(VMData::Object(object_index as u64))?;
                }
                &consts::Add => {
                    let values = self.stack.pop_two();
                    let result = static_add(values.1, values.0)?;
                    self.stack.push(result)?;
                }
                &consts::Subtract => {
                    let values = self.stack.pop_two();
                    let result = static_sub(values.1, values.0)?;
                    self.stack.push(result)?;
                }
                &consts::Multiply => {
                    let values = self.stack.pop_two();
                    let result = static_mul(values.1, values.0)?;
                    self.stack.push(result)?;
                }
                &consts::Divide => {
                    let values = self.stack.pop_two();
                    let result = static_div(values.1, values.0)?;
                    self.stack.push(result)?;
                }
                &consts::GetVar => {
                    let index = u16::from_le_bytes([*current.next(), *current.next()]);
                    debug_assert!(self.stack.top > current.stack_offset + index as usize);
                    self.stack
                        .push(self.stack.data[current.stack_offset + index as usize])?;
                }
                &consts::GetVarFast => {
                    let index = *current.next();
                    debug_assert!(self.stack.top > current.stack_offset + index as usize, "get var out of bounds {} {index}", current.stack_offset);
                    self.stack
                        .push(self.stack.data[current.stack_offset + index as usize])?;
                }
                &consts::ReplaceVarFast => {
                    let index = *current.next();
                    self.stack
                        .swap_top_with_while_stepping_back(index as usize + current.stack_offset);
                }
                &consts::ReplaceVar => {
                    let index = u16::from_le_bytes([*current.next(), *current.next()]);
                    self.stack
                        .swap_top_with_while_stepping_back(index as usize + current.stack_offset);
                }
                &consts::ReplaceVarInObject => {
                    let size = *current.next();
                    let data = self.stack.pop();
                    let mut object = self.stack.data.get_mut(*current.next() as usize).unwrap();
                    for _ in 0..(size - 1) {
                        object = match object {
                            VMData::Object(v) => match &mut unsafe {
                                &mut *(self.objects.get_mut(*v as usize).unwrap() as *mut Object)
                            }
                            .data
                            {
                                ObjectData::Struct(v) => {
                                    v.get_mut(*current.next() as usize).unwrap()
                                }
                                _ => return Err(corrupt_bytecode()),
                            },
                            _ => return Err(corrupt_bytecode()),
                        };
                    }
                    *object = data;
                }
                &consts::Not => {
                    let value = match self.stack.pop() {
                        VMData::Bool(v) => VMData::Bool(!v),
                        _ => return Err(corrupt_bytecode()),
                    };
                    self.stack.push(value)?;
                }
                &consts::Negative => {
                    let value = match self.stack.pop() {
                        VMData::Integer(v) => VMData::Integer(-v),
                        VMData::Float(v) => VMData::Float(-v),
                        _ => return Err(corrupt_bytecode()),
                    };
                    self.stack.push(value)?;
                }
                &consts::Pop => {
                    self.stack.step_back();
                }
                &consts::PopMulti => self.stack.pop_multi_ignore(*current.next() as usize),
                &consts::CreateStruct => {
                    let amount_of_variables = *current.next() as usize;
                    let mut data = Vec::with_capacity(amount_of_variables);
                    for _ in 0..amount_of_variables {
                        data.push(self.stack.pop());
                    }
                    let object_index = self.create_object(Object::new(ObjectData::Struct(data)));
                    self.stack.push(VMData::Object(match object_index {
                        Ok(v) => v,
                        Err(mut err) => {
                            err.bytecode_index = current.index as u64;
                            return Err(err);
                        }
                    } as u64))?;
                }
                &consts::AccessData => {
                    let data = self.stack.pop();
                    let index = current.next();
                    let object = match data {
                        VMData::Object(v) => v,
                        _ => return Err(corrupt_bytecode()),
                    };
                    match &self.get_object(object as usize).data {
                        ObjectData::Struct(v) => {
                            self.stack.push(v[*index as usize])?;
                        }
                        _ => return Err(corrupt_bytecode()),
                    }
                }
                &consts::CallFunction => {
                    let index = *current.next() as usize;
                    let function = match self.functions.get(index) {
                        Some(v) => v,
                        None => {
                            return Err(RuntimeError::new(
                                current.index as u64,
                                "tried to call a none-existant function",
                            ))
                        }
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
                }
                &consts::ReturnFromFunction | &consts::Return => {
                    if current.has_return {
                        let return_value = self.stack.top - 1;

                        self.stack
                            .pop_multi_ignore(self.stack.top - current.stack_offset - 1);
                        self.stack.swap_top_with_while_stepping_back(return_value);
                        self.stack.step();
                    } else {
                        self.stack
                            .pop_multi_ignore(self.stack.top - current.stack_offset);
                    }

                    if callstack.is_empty() {
                        return Ok(());
                    }
                    current = callstack.pop().unwrap();
                }
                &consts::RawCall => {
                    let index = *current.next() as usize;
                    native_library::RAW_FUNCTIONS[index]((self, &mut current))?;
                    // debug_assert!(start == self.stack.top || start + 1 == self.stack.top, "native function ({index}) doesn't have a stack affect of 0");
                }
                &consts::ReturnWithoutCallStack => {
                    let amount = *current.next();

                    let return_value = self.stack.top-1;
                    self.stack.pop_multi_ignore(amount as usize);
                    self.stack.data.swap(return_value, self.stack.top-1);
                }
                &consts::Rotate => {
                    self.stack.step_back();
                    self.stack.swap_top_with(self.stack.top-1);
                    self.stack.swap_top_with(self.stack.top-2);
                    self.stack.step();
                }
                &consts::Over => {
                    self.stack.push(self.stack.data[self.stack.top-2])?;
                }
                &consts::Swap => {
                    self.stack.swap_top_with_while_stepping_back(self.stack.top-2);
                    self.stack.step();
                }
                &consts::Duplicate => {
                    self.stack.push(self.stack.data[self.stack.top-1])?;
                }
                &consts::IndexSwap => {
                    let v1 = *current.next();
                    let v2 = *current.next();
                    self.stack.data.swap(v1 as usize, v2 as usize);
                }
                &consts::Increment => {
                    match self.stack.data.get_mut(self.stack.top-1).unwrap() {
                        VMData::Integer(v) => *v += 1,
                        VMData::Float(v) => *v += 1.0,
                        _ => return Err(corrupt_bytecode())
                    }
                }
                _ => panic!(),
            };

            #[cfg(feature = "stack")]
            {
                // let value = &consts::from_u8(code.code[code.index]);
                print!("        ");
                (0..self.stack.top).for_each(|x| print!("[{:?}]", self.stack.data[x]));
                println!()
            }

            #[cfg(feature = "objects")]
            {
                // let value = &consts::from_u8(code.code[code.index]);
                print!("        ");
                self.objects
                    .data
                    .iter()
                    .filter(|x| !matches!(x.data, ObjectData::Free { .. }))
                    .for_each(|x| print!("{{{:?}}}", x));
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
    pub data: [VMData; 512],
    pub top: usize,
}

impl Stack {
    fn new() -> Self {
        Self {
            data: [VMData::Empty; 512],
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
    pub fn pop(&mut self) -> VMData {
        self.step_back();
        self.data[self.top]
    }

    #[inline(always)]
    #[must_use]
    pub fn view_behind(&self, amount: usize) -> VMData {
        self.data[self.top - amount]
    }

    #[inline(always)]
    #[must_use]
    fn pop_two(&mut self) -> (&VMData, &VMData) {
        self.top -= 2;

        (
            self.data.get(self.top + 1).unwrap(),
            self.data.get(self.top).unwrap(),
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
    pub fn swap_top_with_while_stepping_back(&mut self, index: usize) {
        self.step_back();
        // unsafe { self.data.swap_unchecked(index, self.top) };
        self.data.swap(index, self.top);
    }

    #[inline(always)]
    pub fn swap_top_with(&mut self, index: usize) {
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
