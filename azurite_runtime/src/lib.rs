#![feature(iter_next_chunk)]
#![feature(try_trait_v2)]

mod object_map;
mod runtime;
mod garbage_collection;

use azurite_archiver::{Packed, Data};
use azurite_common::CompilationMetadata;
use colored::Colorize;
use object_map::ObjectMap;
use std::mem::transmute;
use std::{time::Instant, ops::FromResidual, convert::Infallible, ffi::CString, mem::size_of};

pub use object_map::Object;
pub use object_map::ObjectIndex;


const _: () = assert!(size_of::<VMData>() <= 16);


/// Runs a 'Packed' file assuming it is
/// correctly structured
///
/// # Panics
/// - If the 'Packed' value is not correct
pub fn run_packed(packed: Packed) {
    let mut files : Vec<Data> = packed.into();

    let constants = files.pop().unwrap();
    let bytecode = files.pop().unwrap();
    let metadata = unsafe { transmute::<[u8; size_of::<CompilationMetadata>()], CompilationMetadata>(files.pop().unwrap().0.try_into().unwrap()) };

    assert!(files.is_empty());

    run(metadata, &bytecode.0, constants.0);
}


/// The main VM object
pub struct VM {
    pub(crate) constants: Vec<VMData>,
    pub stack: Stack,
    pub objects: ObjectMap,
    metadata: CompilationMetadata,
}


impl VM {
    pub fn create_object(&mut self, object: Object) -> Result<ObjectIndex, FatalError> {
        match self.objects.put(object) {
            Ok(v) => Ok(v),
            Err(object) => {
                self.run_garbage_collection();
                match self.objects.put(object) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(FatalError::new(String::from("out of memory"))),
                }
            },
        }
    }
}


#[derive(Debug)]
#[repr(C)]
pub struct Stack {
    values: [VMData; 50],
    stack_offset: usize,
    top: usize,
}


#[allow(clippy::inline_always)]
impl Stack {
    fn new() -> Self {
        Self {
            values: [VMData::Empty; 50],
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
    fn push(&mut self, amount: usize) -> Status {
        self.top += amount;
        if self.top >= self.values.len() {
            return Status::Err(FatalError::new(String::from("stack overflow")))
        }

        Status::Ok
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
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Float(f64),
    Bool(bool),
    Object(ObjectIndex),
    Empty,
}


macro_rules! enum_variant_function {
    ($getter: ident, $is: ident, $variant: ident, $ty: ty) => {
        #[inline(always)]
        #[must_use]
        pub fn $getter(self) -> $ty {
            match self {
                VMData::$variant(v) => v,
                _ => unreachable!()
            }
        }


        #[inline(always)]
        #[must_use]
        pub fn $is(self) -> bool {
            matches!(self, VMData::$variant(_))
        }
    }
}


#[allow(clippy::inline_always)]
impl VMData {
    enum_variant_function!(as_i8 , is_i8 , I8 , i8);
    enum_variant_function!(as_i16, is_i16, I16, i16);
    enum_variant_function!(as_i32, is_i32, I32, i32);
    enum_variant_function!(as_i64, is_i64, I64, i64);
    enum_variant_function!(as_u8 , is_u8 , U8 , u8);
    enum_variant_function!(as_u16, is_u16, U16, u16);
    enum_variant_function!(as_u32, is_u32, U32, u32);
    enum_variant_function!(as_u64, is_u64, U64, u64);

    enum_variant_function!(as_float, is_float, Float, f64);
    enum_variant_function!(as_bool, is_bool, Bool, bool);
    enum_variant_function!(as_object, is_object, Object, ObjectIndex);
}


#[derive(Debug)]
#[repr(C)]
pub enum Status {
    Ok,
    Err(FatalError),
    Exit(i32),
}


impl Status {
    pub fn ok() -> Status {
        Status::Ok
    }


    pub fn err(str: impl ToString) -> Status {
        Status::Err(FatalError::new(str.to_string()))
    }


    #[inline]
    pub fn is_exit(&self) -> bool {
        matches!(self, Status::Exit(_))
    }


    #[inline]
    pub fn is_err(&self) -> bool {
        matches!(self, Status::Err(_))
    }


    #[inline]
    pub fn is_ok(&self) -> bool {
        matches!(self, Status::Ok)
    }
}


impl FromResidual<std::result::Result<Infallible, FatalError>> for Status {
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
    pub fn read_message(&self) -> CString {
        unsafe { CString::from_raw(self.message) } 
    }
}


fn run(metadata: CompilationMetadata, bytecode: &[u8], constants: Vec<u8>) {
    let mut vm = VM {
        constants: Vec::new(),
        stack: Stack::new(),
        objects: ObjectMap::new((1 * 1024 * 1024) / size_of::<Object>()),
        metadata,
    };

    if let Err(e) = bytes_to_constants(&mut vm, constants) {
        println!(
            "{}",
            format!("panicked at '{}'", e.read_message().to_string_lossy()).bright_red()
        );
    }
    
    let start = Instant::now();

    let v = vm.run(Code::new(bytecode, 0, 0));

    let end = start.elapsed();
    println!("it took {}ms {}ns, result {:?}", end.as_millis(), end.as_nanos(), vm.stack.reg(0));

    if let Status::Exit(v) = v {
        std::process::exit(v)
    }
    

}


fn bytes_to_constants(vm: &mut VM, data: Vec<u8>) -> Result<(), FatalError> {
    let mut constants_iter = data.into_iter();

    while let Some(datatype) = constants_iter.next() {
        let constant = match datatype {
            0 => VMData::Float(f64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),

            1 => VMData::Bool(constants_iter.next().unwrap() == 1),

            2 => {
                let length = u64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap());

                let mut vec = Vec::with_capacity(length as usize);
                for _ in 0..length {
                    vec.push(constants_iter.next().unwrap());
                }

                let object = String::from_utf8(vec).unwrap();
                
                let index = vm.create_object(Object::new(object))?;

                VMData::Object(index)
            }

            3  => VMData::I8 (i8 ::from_le_bytes(constants_iter.next_chunk::<1>().unwrap())),
            4  => VMData::I16(i16::from_le_bytes(constants_iter.next_chunk::<2>().unwrap())),
            5  => VMData::I32(i32::from_le_bytes(constants_iter.next_chunk::<4>().unwrap())),
            6  => VMData::I64(i64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),
            7  => VMData::U8 (u8 ::from_le_bytes(constants_iter.next_chunk::<1>().unwrap())),
            8  => VMData::U16(u16::from_le_bytes(constants_iter.next_chunk::<2>().unwrap())),
            9  => VMData::U32(u32::from_le_bytes(constants_iter.next_chunk::<4>().unwrap())),
            10 => VMData::U64(u64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),

            _ => unreachable!()
        };

        vm.constants.push(constant);
    };
    Ok(())
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

