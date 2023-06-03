// #![deny(clippy::pedantic)]
#![feature(iter_next_chunk)]
#![feature(try_trait_v2)]
mod object_map;
mod runtime;

use azurite_archiver::{Packed, Data};
use object_map::ObjectMap;
use std::{time::Instant, ops::FromResidual, convert::Infallible, ffi::CString};

pub use object_map::Object;

/// Runs a 'Packed' file assuming it is
/// correctly structured
///
/// # Panics
/// - If the 'Packed' value is not correct
pub fn run_packed(packed: Packed) {
    let mut files : Vec<Data> = packed.into();

    let constants = files.pop().unwrap();
    let bytecode = files.pop().unwrap();

    assert!(files.is_empty());

    run(&bytecode.0, constants.0);
}


/// The main VM object
#[derive(Debug)]
pub struct VM {
    pub(crate) constants: Vec<VMData>,
    pub stack: Stack,
    pub objects: ObjectMap,
}


#[derive(Debug)]
#[repr(C)]
pub struct Stack {
    values: [VMData; 1024],
    stack_offset: usize,
    top: usize,
}


#[allow(clippy::inline_always)]
impl Stack {
    fn new() -> Self {
        Self {
            values: [VMData::Empty; 1024],
            stack_offset: 0,
            top: 1,
        }
    }

    /// Returns the value at `stack_offset + reg`
    ///
    /// This method panics in debug mode if the resulting value is
    /// beyond the "top" of the stack. 
    ///
    /// In release mode accessing a register above the "top" of the
    /// stack is unspecified behaviour and could lead to crashes
    #[inline(always)]
    #[must_use]
    pub fn reg(&self, reg: u8) -> VMData {
        debug_assert!((reg as usize + self.stack_offset) < self.top, "{reg} {} {}", self.stack_offset, self.top);
        self.values[reg as usize + self.stack_offset]
    }

    /// Sets the value at `stack_offset + reg` to the given data
    ///
    /// This method panics in debug mode if the resulting value is
    /// beyond the "top" of the stack. 
    ///
    /// In release mode accessing a register above the "top" of the
    /// stack is unspecified behaviour and could lead to crashes
    #[inline(always)]
    pub fn set_reg(&mut self, reg: u8, data: VMData) {
        debug_assert!((reg as usize + self.stack_offset) < self.top, "reg: {reg} offset: {} top: {} {data:?}", self.stack_offset, self.top);
        self.values[reg as usize + self.stack_offset] = data;
    }

    #[inline(always)]
    fn set_stack_offset(&mut self, amount: usize) {
        debug_assert!(amount < self.top);
        self.stack_offset = amount;
    }

    #[inline(always)]
    fn push(&mut self, amount: usize) -> Result {
        self.top += amount;
        if self.top >= self.values.len() {
            return Result::Err(FatalError::new(String::from("stack overflow")))
        }

        Result::Ok
    }

    #[inline(always)]
    fn pop(&mut self, amount: usize) {
        self.top -= amount;
    }
}


/// The runtime union of stack values
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMData {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Object(u64),
    Empty,
}


#[allow(clippy::inline_always)]
impl VMData {
    /// Consumes the union value and returns an i64
    ///
    /// # Panics
    /// - If the union type is not an integer
    #[inline(always)]
    #[must_use]
    pub fn integer(self) -> i64 {
        match self {
            VMData::Integer(v) => v,
            _ => unreachable!()
        }
    }

    
    /// Consumes the union value and returns an f64
    ///
    /// # Panics
    /// - If the union type is not an float
    #[inline(always)]
    #[must_use]
    pub fn float(self) -> f64 {
        match self {
            VMData::Float(v) => v,
            _ => unreachable!()
        }
    }
    
    
    /// Consumes the union value and returns a bool
    ///
    /// # Panics
    /// - If the union type is not a boolean
    #[inline(always)]
    #[must_use]
    pub fn bool(self) -> bool {
        match self {
            VMData::Bool(v) => v,
            _ => unreachable!()
        }
    }

    
    /// Consumes the union value and returns the object index
    /// This object index can be used to index into the objectmap
    /// provided by the VM
    ///
    /// # Panics
    /// - If the union type is not an object
    #[inline(always)]
    #[must_use]
    pub fn object(self) -> u64 {
        match self {
            VMData::Object(v) => v,
            _ => unreachable!()
        }
    }
}


#[derive(Debug)]
#[repr(C)]
pub enum Result {
    Ok,
    Err(FatalError)
}


impl Result {
    pub fn ok() -> Result {
        Result::Ok
    }


    pub fn err(str: impl ToString) -> Result {
        Result::Err(FatalError::new(str.to_string()))
    }


    #[inline]
    pub fn is_err(&self) -> bool {
        matches!(self, Result::Err(_))
    }


    #[inline]
    pub fn is_ok(&self) -> bool {
        matches!(self, Result::Ok)
    }
}


impl FromResidual<std::result::Result<Infallible, FatalError>> for Result {
    fn from_residual(residual: std::result::Result<Infallible, FatalError>) -> Self {
        match residual {
            Ok(_) => Self::Ok,
            Err(e) => Self::Err(e),
        }
    }
}


/// An unrecoverable runtime error
#[derive(Debug)]
#[repr(C)]
pub struct FatalError {
    index: usize,
    message: *mut i8,
}


impl FatalError {
    pub fn new(message: String) -> Self {
        Self {
            index: usize::MAX,
            message: CString::new(message).unwrap().into_raw(),
        }
    }


    #[inline]
    pub fn read_message(self) -> CString {
        unsafe { CString::from_raw(self.message) } 
    }
}


fn run(bytecode: &[u8], constants: Vec<u8>) {
    let mut vm = VM {
        constants: Vec::new(),
        stack: Stack::new(),
        objects: ObjectMap::new(128),
    };

    bytes_to_constants(&mut vm, constants);
    
    let start = Instant::now();

    vm.run(Code::new(bytecode, 0, 0));
    
    let end = start.elapsed();
    println!("it took {}ms {}ns, result {:?}", end.as_millis(), end.as_nanos(), vm.stack.reg(0));

}


fn bytes_to_constants(vm: &mut VM, data: Vec<u8>) {
    let mut constants_iter = data.into_iter();

    while let Some(datatype) = constants_iter.next() {
        let constant = match datatype {
            0 => VMData::Integer(i64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),
            
            1 => VMData::Float(f64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),

            2 => VMData::Bool(constants_iter.next().unwrap() == 1),

            3 => {
                let length = u64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap());

                let mut vec = Vec::with_capacity(length as usize);
                for _ in 0..length {
                    vec.push(constants_iter.next().unwrap());
                }

                let object = String::from_utf8(vec).unwrap();
                
                let index = vm.objects.put(object_map::Object::String(object)).unwrap();

                VMData::Object(index as u64)
            }

            _ => unreachable!()
        };

        vm.constants.push(constant);
    };
}



#[derive(Debug)]
pub(crate) struct Code<'a> {
    pointer: usize,
    code: &'a [u8],

    offset: usize,
    return_to: u8,
}


#[allow(clippy::inline_always)]
impl<'a> Code<'a> {
    fn new(code: &[u8], offset: usize, return_to: u8) -> Code { Code { pointer: 0, code, offset, return_to } }

    #[inline(always)]
    #[must_use]
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

            bytes.push(val);
        }

        String::from_utf8(bytes).unwrap()
    }
    

    #[inline(always)]
    #[must_use]
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


