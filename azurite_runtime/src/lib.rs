#![feature(get_many_mut)]
#![feature(vec_push_within_capacity)]

use std::{env, mem::size_of, slice::Iter, process::ExitCode, io::Read};

use azurite_common::{DataType, STRING_TERMINATOR};
use object_map::ObjectMap;
use runtime_error::RuntimeError;
use vm::VM;

pub mod garbage_collector;
pub mod native_library;
pub mod object_map;
pub mod runtime_error;
pub mod vm;


pub fn run_file(path: String) -> Result<(), ExitCode> {
    let zipfile = std::fs::File::open(&path).unwrap();

    let mut archive = zip::ZipArchive::new(zipfile).unwrap();

    let mut bytecode_file = match archive.by_name("bytecode.azc") {
        Ok(file) => file,
        Err(..) => {
            println!("bytecode.azc not found");
            return Err(ExitCode::FAILURE);
        }
    };

    let mut bytecode = vec![];
    match bytecode_file.read_to_end(&mut bytecode) {
        Ok(_) => {}
        Err(_) => return Err(ExitCode::FAILURE),
    };

    drop(bytecode_file);

    let mut constants_file = match archive.by_name("constants.azc") {
        Ok(file) => file,
        Err(..) => {
            println!("constants.azc not found");
            return Err(ExitCode::FAILURE);
        }
    };

    let mut constants = vec![];
    match constants_file.read_to_end(&mut constants) {
        Ok(_) => {}
        Err(_) => return Err(ExitCode::FAILURE),
    };

    drop(constants_file);

    let mut vm = match VM::new() {
        Ok(v) => v,
        Err(err) => return err.trigger(path),
    };

    vm.constants = match load_constants(constants, &mut vm) {
        Ok(v) => v,
        Err(err) => {
            err.trigger(path)?;
            return Err(ExitCode::FAILURE);
        }
    };
    // let instant = Instant::now();

    let runtime = vm.run(&bytecode);

    // let end = instant.elapsed().as_millis();
    // println!("\n\nit took {}ms", end);

    #[cfg(feature = "hotspot")]
    {
        use azurite_common::Bytecode;
        use std::cmp::Ordering;
        let mut x = vm
            .hotspots
            .into_iter()
            .map(|x| (x.1 .0, x.0, x.1 .1))
            .collect::<Vec<(usize, Bytecode, f64)>>();
        x.sort_by(|x, y| {
            if x.0 < y.0 {
                Ordering::Greater
            } else if x.0 > y.0 {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        println!("---------------------------------------------");
        x.into_iter().for_each(|(x, y, time)| {
            println!("| {:>15} -> {x:>10} - {time:>9.3} |", format!("{:?}", y))
        });
        println!("---------------------------------------------");
    }
    if let Err(runtime) = runtime {
        println!("runtime err");
        runtime.trigger(path)?;
        return Err(ExitCode::FAILURE);
    }

    Ok(())
}


pub fn load_constants(
    mut constant_bytes: Vec<u8>,
    vm: &mut VM,
) -> Result<Vec<VMData>, RuntimeError> {
    // Buffer required or else the last
    // constant won't be parsed
    //
    // The value of this doesn't matter
    constant_bytes.push(0);

    let mut constants = Vec::with_capacity(32);
    let mut constant_byte_iterator = constant_bytes.into_iter();

    let mut size_lookout = None;
    let mut current_type = DataType::Integer;
    let mut values: Vec<u8> = Vec::with_capacity(32);
    while let Some(current_byte) = constant_byte_iterator.next() {
        if size_lookout.is_some() && values.len() < size_lookout.unwrap() &&
            // If it is looking for a string and the
            // the current byte is the termination byte
            (current_type != DataType::String || current_byte != STRING_TERMINATOR)
        {
            values.push(current_byte);
            continue;
        }

        let mut old_type = current_type.clone();

        if size_lookout.is_some() {
            let data = parse_data(&mut old_type, &values, &mut constant_byte_iterator, vm)?;
            constants.push(data);
            values.clear();
        }
        current_type = if old_type != current_type {
            old_type
        } else {
            match current_byte.try_into() {
                Ok(v) => v,
                Err(_) => current_type,
            }
        };
        size_lookout = Some(current_type.size());
    }

    Ok(constants)
}

pub fn parse_data(
    current_type: &mut DataType,
    values: &[u8],
    iterator: &mut impl Iterator<Item = u8>,
    vm: &mut VM,
) -> Result<VMData, RuntimeError> {
    Ok(match current_type {
        DataType::Integer => VMData::Integer(i64::from_le_bytes(
            values[0..DataType::Integer.size()].try_into().unwrap(),
        )),
        DataType::Float => VMData::Float(f64::from_le_bytes(
            values[0..DataType::Float.size()].try_into().unwrap(),
        )),
        DataType::String => {
            // We can be sure that it is UTF-8 since the compiler won't
            // output anything that is not valid UTF-8
            let string = String::from_utf8(values.to_owned()).unwrap();

            *current_type = iterator.next().unwrap().try_into().unwrap();
            let object = Object::new(ObjectData::String(string));
            let index = match vm.create_object(object) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };
            VMData::Object(index as u64)
        }
        DataType::Bool => VMData::Bool(values[0] > 0),
        DataType::Struct(_) => todo!(),
        DataType::Empty => todo!(),
    })
}

pub fn get_vm_memory() -> Result<usize, RuntimeError> {
    let binding = env::var("AZURITE_MEMORY").unwrap_or("MB10".to_string());
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
    base /= size_of::<Object>() * 8;
    Ok(base)
}

#[derive(Debug, Clone, PartialEq)]
pub enum VMData {
    Integer(i64),
    Float(f64),
    Object(u64), // stores index // TODO: change to a pointer
    Bool(bool),
}

impl VMData {
    fn to_string(&self, vm: &VM) -> String {
        let text = match self {
            VMData::Integer(v) => v.to_string(),
            VMData::Float(v) => v.to_string(),
            VMData::Bool(v) => v.to_string(),
            VMData::Object(object) => {
                let object = *object;
                let obj = vm.get_object(object as usize);
                obj.data.to_string(vm)
            }
        };
        text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    live: bool,
    data: ObjectData,
}

impl ObjectData {
    fn to_string(&self, vm: &VM) -> String {
        match self {
            crate::ObjectData::String(v) => v.clone(),
            crate::ObjectData::List(list) => {
                let datas = list.iter().enumerate();
                let mut stringified = String::new();
                for (index, data) in datas {
                    stringified.push_str(data.to_string(vm).as_str());
                    if index < list.len() - 1 {
                        stringified.push_str(", ")
                    }
                }
                stringified
            }
            crate::ObjectData::Struct(list) => {
                let datas = list.iter().enumerate();
                let mut stringified = String::new();
                for (index, data) in datas {
                    stringified.push_str(data.to_string(vm).as_str());
                    if index < list.len() - 1 {
                        stringified.push_str(", ")
                    }
                }
                stringified
            }
            crate::ObjectData::Free { .. } => panic!("can't display free"),
        }
    }
}

impl Object {
    fn new(data: ObjectData) -> Self {
        Self { live: false, data }
    }

    fn mark_inner(&self, objects: &mut ObjectMap) {
        match &self.data {
            ObjectData::String(_) => (),
            ObjectData::List(v) => v.iter().mark(objects),
            ObjectData::Struct(v) => v.iter().mark(objects),
            ObjectData::Free { .. } => (),
        }
    }
}

trait Mark {
    fn mark(&mut self, objects: &mut ObjectMap);
}

impl Mark for Iter<'_, VMData> {
    fn mark(&mut self, objects: &mut ObjectMap) {
        self.for_each(|x| {
            if let VMData::Object(value) = x {
                unsafe { objects.data.get_unchecked_mut(*value as usize) }.live = true;
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData {
    String(String),
    List(Vec<VMData>),
    Struct(Vec<VMData>),
    Free { next: usize },
}
