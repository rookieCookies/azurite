#![allow(clippy::cast_possible_truncation)]
use std::{cell::{Cell, RefCell}, env};

use azurite_archiver::Packed;
use azurite_common::DataType;
use runtime_error::RuntimeError;
use vm::VM;

pub mod garbage_collector;
pub mod native_library;
pub mod object_map;
pub mod runtime_error;
pub mod vm;
mod unit_tests;

/// # Panics
/// # Errors
pub fn run_file(path: &str) -> Result<(), String> {
    let file = std::fs::read(path).unwrap();

    let packed = match Packed::from_bytes(&file) {
        Some(v) => v,
        None => {
            panic!("not a valid azurite file")
        },
    };

    run_packed(packed)
}

pub fn run_packed(packed: Packed) -> Result<(), String> {
    let mut data : Vec<_> = packed.into();

    let bytecode = data.remove(0).0;
    let constants = data.remove(0).0;
    let linetable = data.remove(0).0;
    let function_table = data.remove(0).0;


    let mut vm = match VM::new() {
        Ok(v) => v,
        Err(err) => {
            let t = err.message.clone();
            err.trigger(linetable, function_table, vec![(0, 0)]);
            return Err(t);
        },
    };

    match load_constants(constants, &mut vm) {
        Ok(_) => (),
        Err(err) => {
            let t = err.message.clone();
            err.trigger(linetable, function_table, vec![(0, 0)]);
            return Err(t);
        }
    };

    let callstack_tracker = RefCell::new(vec![]);
    let runtime = vm.run(&callstack_tracker, &bytecode);

    let mut callstack_debug = callstack_tracker.into_inner();
    
    {
        callstack_debug.reverse();

        let last = callstack_debug.pop().unwrap();
        callstack_debug.insert(0, last);
    }

    if let Err(runtime) = runtime {
        let temp = runtime.message.clone();
        runtime.trigger(linetable, function_table, callstack_debug);
        return Err(temp);
    }

    Ok(())
}

/// # Panics
/// # Errors
/// - Not enough memory in the VM to be able to allocate strings
pub fn load_constants(
    mut constant_bytes: Vec<u8>,
    vm: &mut VM,
) -> Result<(), RuntimeError> {
    // Buffer required or else the last
    // constant won't be parsed
    //
    // The value of this doesn't matter
    constant_bytes.push(0);

    let mut constant_byte_iterator = constant_bytes.into_iter();

    let mut size_lookout = None;
    let mut current_type = None;
    let mut values: Vec<u8> = Vec::with_capacity(32);
    while let Some(current_byte) = constant_byte_iterator.next() {
        if let Some(size) = size_lookout {
            if values.len() < size {
                values.push(current_byte);
                continue
            }
            let data = parse_data(current_type.as_ref().unwrap(), &values, vm)?;
            vm.constants.push(data);
            current_type = None;
        }

        if current_type.is_none() {
            values.clear();

            current_type = Some(DataType::from_byte_representation(current_byte).unwrap());
            size_lookout = Some(match current_type.as_ref().unwrap() {
                DataType::String => u32::from_le_bytes([
                    constant_byte_iterator.next().unwrap(),
                    constant_byte_iterator.next().unwrap(),
                    constant_byte_iterator.next().unwrap(),
                    constant_byte_iterator.next().unwrap(),
                ]) as usize,
                _ => current_type.as_ref().unwrap().size(),
            });
        }
    }
    Ok(())
}

/// # Errors
/// - Not enough memory in the VM to be able to allocate strings
pub fn parse_data(
    current_type: &DataType,
    values: &[u8],
    vm: &mut VM,
) -> Result<VMData, RuntimeError> {
    Ok(match current_type {
        DataType::Integer => VMData::Integer(i64::from_le_bytes(
            match values[0..DataType::Integer.size()].try_into() {
                Ok(v) => v,
                Err(_) => {
                    return Err(RuntimeError::new(
                        0,
                        "constants file is corrupt, failed to parse integer",
                    ))
                }
            },
        )),
        DataType::Float => VMData::Float(f64::from_le_bytes(
            match values[0..DataType::Float.size()].try_into() {
                Ok(v) => v,
                Err(_) => {
                    return Err(RuntimeError::new(
                        0,
                        "constants file is corrupt, failed to parse float",
                    ))
                }
            },
        )),
        DataType::String => {
            // We can be sure that it is UTF-8 since the compiler won't
            // output anything that is not valid UTF-8
            let string = match String::from_utf8(values.to_owned()) {
                Ok(v) => v,
                Err(_) => {
                    return Err(RuntimeError::new(
                        0,
                        "constants file is corrupt, string is not valid utf-8",
                    ))
                }
            };

            let object = Object::new(ObjectData::String(string));
            let index = match vm.create_object(object) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };
            VMData::Object(index as u64)
        }
        DataType::Bool => VMData::Bool(values[0] > 0),
        _ => {
            return Err(RuntimeError::new(
                0,
                "constants file is corrupt, invalid type",
            ))
        }
    })
}

/// # Errors
/// This function will error if the environment value is
/// not a valid parseable value
pub fn get_vm_memory_in_bytes() -> Result<usize, RuntimeError> {
    #[cfg(afl)]
    return Ok(1280);
    
    let binding = env::var("AZURITE_MEMORY").unwrap_or_else(|_| "KB16".to_string());
    let v = binding.split_at(2);
    let mut base = match v.1.parse::<usize>() {
        Ok(v) => v,
        Err(_) => return Err(RuntimeError::new(0, "failed to parse AZURITE_MEMORY")),
    };
    base *= match v.0 {
        "BT" => 1,
        "BY" => 8,
        "KB" => 8 * 1000,
        "MB" => 8 * 1000 * 1000,
        "GB" => 8 * 1000 * 1000 * 1000,
        _ => return Err(RuntimeError::new(0, "failed to parse AZURITE_MEMORY")),
    };
    base /= 8;

    Ok(base)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VMData {
    Integer(i64),
    Float(f64),
    Object(u64), // stores index // TODO: change to a pointer
    Bool(bool),
    Empty,
}

impl VMData {
    fn to_string(self, vm: &VM) -> String {
        let text = match self {
            VMData::Integer(v) => v.to_string(),
            VMData::Float(v) => v.to_string(),
            VMData::Bool(v) => v.to_string(),
            VMData::Object(object) => {
                let obj = vm.get_object(object as usize);
                obj.data.to_string(vm)
            }
            VMData::Empty => todo!(),
        };
        text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    live: Cell<bool>,
    data: ObjectData,
}

impl ObjectData {
    fn to_string(&self, vm: &VM) -> String {
        match self {
            ObjectData::List(list) | ObjectData::Struct(list) => {
                let datas = list.iter().enumerate();
                let mut stringified = String::new();
                for (index, data) in datas {
                    stringified.push_str(data.to_string(vm).as_str());
                    if index < list.len() - 1 {
                        stringified.push_str(", ");
                    }
                }
                stringified
            }
            ObjectData::String(v) => v.clone(),
            ObjectData::Free { .. } => panic!("can't display free"),
        }
    }
}

impl Object {
    fn new(data: ObjectData) -> Self {
        Self { live: Cell::new(false), data }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData {
    String(String),
    List(Vec<VMData>),
    Struct(Vec<VMData>),
    Free { next: usize },
}
