#![feature(iter_next_chunk)]
mod object_map;
mod runtime;

use azurite_archiver::{Packed, Data};
use libloading::Symbol;
use object_map::ObjectMap;
use std::{ops::{Add, Sub, Mul, Div}, time::Instant};

pub use object_map::Object;

pub fn run_file(packed: Packed) {
    let mut files : Vec<Data> = packed.into();
    
    let bytecode = files.remove(0);
    let constants = files.remove(0);

    let mut vm = VM {
        constants: Vec::new(),
        stack: Stack::new(),
        objects: ObjectMap::new(),
    };

    bytes_to_constants(constants.0, &mut vm);
    
    let start = Instant::now();

    vm.run(Code::new(&bytecode.0, 0, 0));
    
    let end = start.elapsed();
    println!("it took {}ms {}ns, result {:?}", end.as_millis(), end.as_nanos(), vm.stack.reg(0));

}

pub fn bytes_to_constants(data: Vec<u8>, vm: &mut VM) {
    let mut constants_iter = data.into_iter();

    while let Some(datatype) = constants_iter.next() {
        let constant = match datatype {
            0 => VMData::Integer(i64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),
            
            1 => VMData::Float(f64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),

            2 => VMData::Bool(constants_iter.next().unwrap() == 1),

            3 => {
                let mut string = Vec::new();
                loop {
                    let data = constants_iter.next().unwrap();
                    if data == 0 {
                        break
                    }

                    string.push(data);
                }

                let object = Object::String(String::from_utf8_lossy(&string).into_owned());
                
                let index = vm.objects.put(object).unwrap();

                VMData::Object(index as u64)
            }

            _ => unreachable!()
        };

        vm.constants.push(constant);
    };
}


#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMData {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Object(u64),
    Empty,
}

impl VMData {
    pub fn integer(self) -> i64 {
        match self {
            VMData::Integer(v) => v,
            _ => unreachable!()
        }
    }

    
    pub fn float(self) -> f64 {
        match self {
            VMData::Float(v) => v,
            _ => unreachable!()
        }
    }
    
    
    pub fn bool(self) -> bool {
        match self {
            VMData::Bool(v) => v,
            _ => unreachable!()
        }
    }

    
    pub fn object(self) -> u64 {
        match self {
            VMData::Object(v) => v,
            _ => unreachable!()
        }
    }
}


#[derive(Debug)]
pub struct VM {
    pub(crate) constants: Vec<VMData>,
    pub stack: Stack,
    pub objects: ObjectMap,
}


#[derive(Debug)]
pub(crate) struct Code<'a> {
    pointer: usize,
    code: &'a [u8],

    offset: usize,
    return_to: u8,
}


impl<'a> Code<'a> {
    pub fn new(code: &[u8], offset: usize, return_to: u8) -> Code { Code { pointer: 0, code, offset, return_to } }

    #[inline(always)]
    fn next(&mut self) -> u8 {
        let result = self.code[self.pointer];
        self.pointer += 1;
        
        result
    }


    fn string(&mut self) -> String {
        let mut bytes = vec![];

        loop {
            let val = self.next();
            if val == 0 {
                break
            }

            bytes.push(val)
        }

        String::from_utf8(bytes).unwrap()
    }
    

    #[inline(always)]
    fn u32(&mut self) -> u32 {
        let slice = &self.code[self.pointer..][..4];
        let arr : &[u8; 4] = slice.try_into().expect("invalid length");

        self.pointer += 4;
        
        u32::from_le_bytes(*arr)
    }


    #[inline(always)]
    fn goto(&mut self, at: usize) {
        self.pointer = at;
    }
}


#[derive(Debug)]
#[repr(C)]
pub struct Stack {
    values: [VMData; 512],
    stack_offset: usize,
    top: usize,
}

impl Stack {
    fn new() -> Self {
        Self {
            values: [VMData::Empty; 512],
            stack_offset: 0,
            top: 1,
        }
    }

    #[inline(always)]
    pub fn reg(&self, reg: u8) -> VMData {
        debug_assert!((reg as usize + self.stack_offset) < self.top, "{reg} {} {}", self.stack_offset, self.top);
        self.values[reg as usize + self.stack_offset]
    }

    #[inline(always)]
    pub fn set_reg(&mut self, reg: u8, data: VMData) {
        debug_assert!((reg as usize + self.stack_offset) < self.top, "reg: {reg} offset: {} top: {} {data:?}", self.stack_offset, self.top);
        self.values[reg as usize + self.stack_offset] = data
    }

    #[inline(always)]
    fn set_stack_offset(&mut self, amount: usize) {
        debug_assert!(amount < self.top);
        self.stack_offset = amount;
    }

    #[inline(always)]
    fn push(&mut self, amount: usize) {
        self.top += amount;
    }

    #[inline(always)]
    fn pop(&mut self, amount: usize) {
        self.top -= amount;
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}