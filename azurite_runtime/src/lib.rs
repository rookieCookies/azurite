#![feature(iter_next_chunk)]
#![feature(mutex_unpoison)]
#![feature(try_trait_v2)]

mod object_map;
mod runtime;
mod garbage_collection;

use azurite_archiver::{Packed, Data};
use azurite_common::CompilationMetadata;
use colored::Colorize;
use libloading::Library;
use libloading::Symbol;
use object_map::ObjectData;
use object_map::ObjectMap;
use std::env;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Write;
use std::panic::catch_unwind;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use std::{time::Instant, ops::FromResidual, convert::Infallible, ffi::CString, mem::size_of};

pub use object_map::Object;
pub use object_map::ObjectIndex;
pub use object_map::Structure;


const _: () = assert!(size_of::<VMData>() <= 16);


#[allow(clippy::type_complexity)]
static PANIC_INFO : Mutex<Option<((String, u32, u32), String)>> = Mutex::new(None);


type ExternFunction<'a> = Symbol<'a, ExternFunctionRaw>;
type ExternFunctionRaw = unsafe extern "C" fn(&mut VM) -> Status;


/// Runs a 'Packed' file assuming it is
/// correctly structured
///
/// # Panics
/// - If the 'Packed' value is not correct
pub fn run_packed(packed: Packed) -> Result<(), &'static str> {
    let mut files : Vec<Data> = packed.into();

    let Some(constants) = files.pop() else { return Err("the file isn't a valid azurite file") };
    let Some(bytecode)  = files.pop() else { return Err("the file isn't a valid azurite file") };
    let Some(metadata)  = files.pop() else { return Err("the file isn't a valid azurite file") };
    let Ok(metadata)    = metadata.0.try_into() else { return Err("the file isn't a valid azurite file")};
    let metadata        = CompilationMetadata::from_bytes(metadata);

    assert!(files.is_empty());

    run(metadata, &bytecode.0, constants.0);
    Ok(())
}


/// The main VM object
pub struct VM<'a> {
    pub(crate) constants: Vec<VMData>,
    pub stack: Stack,
    pub objects: ObjectMap,

    callstack: Vec<Code<'a>>,
    current: Code<'a>,
    libraries: Vec<Library>,
    externs: Vec<ExternFunctionRaw>,
    metadata: CompilationMetadata,

    debug: VMDebugInfo,
}


impl VM<'_> {
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


const STACK_SIZE : usize = 16 * 1024 / size_of::<VMData>();


#[derive(Debug)]
#[repr(C)]
pub struct Stack {
    values: [VMData; STACK_SIZE],
    stack_offset: usize,
    top: usize,
}


#[allow(clippy::inline_always)]
impl Stack {
    fn new() -> Self {
        Self {
            values: [VMData::new_unit(); STACK_SIZE],
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
        let result = unsafe { *self.code.get_unchecked(self.pointer) };
        self.pointer += 1;
        
        result
    }


    #[inline(always)]
    #[must_use]
    fn next_n<const N: usize>(&mut self) -> [u8; N] {        
        let slice = &self.code[self.pointer..][..N];
        let arr : &[u8; N] = slice.try_into().expect("invalid length");

        self.pointer += N;

        *arr
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

        String::from_utf8(bytes).expect("string in the bytecode which is tried as a string isn't valid utf-8")
    }
    

    #[inline(always)]
    #[must_use]
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.next_n::<4>())
    }
    

    #[inline(always)]
    #[must_use]
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.next_n::<8>())
    }


    #[inline(always)]
    fn goto(&mut self, at: usize) {
        self.pointer = at;
    }
}


#[repr(C)]
#[derive(Clone, Copy)]
pub struct VMData {
    tag: u64,
    data: RawVMData,
}


macro_rules! def_new_vmdata_func {
    ($ident: ident, $field: ident, $ty: ty, $const: ident) => {
        #[inline(always)]
        pub fn $ident(val: $ty) -> Self {
            Self::new(Self::$const, RawVMData { $field: val })
        }
    }
}
impl VMData {
    pub const TAG_UNIT : u64 = 0;
    pub const TAG_U8: u64 = 1;
    pub const TAG_U16: u64 = 2;
    pub const TAG_U32: u64 = 3;
    pub const TAG_U64: u64 = 4;
    pub const TAG_I8: u64 = 5;
    pub const TAG_I16: u64 = 6;
    pub const TAG_I32: u64 = 7;
    pub const TAG_I64: u64 = 8;
    pub const TAG_FLOAT: u64 = 9;
    pub const TAG_BOOL: u64 = 10;
    pub const TAG_STR: u64 = 11;


    pub fn new(tag: u64, data: RawVMData) -> Self {
        Self {
            tag,
            data,
        }
    }
    

    #[inline(always)]
    pub fn tag(self) -> u64 {
        self.tag
    }
    

    pub fn new_unit() -> Self {
        Self::new(Self::TAG_UNIT, RawVMData { as_unit: () })
    }


    pub fn new_object(tag: u64, val: ObjectIndex) -> Self {
        assert!(tag > 256, "object typeid is within the reserved area");
        Self::new(tag, RawVMData { as_object: val })
    }


    pub fn new_string(val: ObjectIndex) -> Self {
        Self::new(Self::TAG_STR, RawVMData { as_object: val })
    }


    def_new_vmdata_func!(new_i8, as_i8, i8, TAG_I8);
    def_new_vmdata_func!(new_i16, as_i16, i16, TAG_I16);
    def_new_vmdata_func!(new_i32, as_i32, i32, TAG_I32);
    def_new_vmdata_func!(new_i64, as_i64, i64, TAG_I64);
    def_new_vmdata_func!(new_u8, as_u8, u8, TAG_U8);
    def_new_vmdata_func!(new_u16, as_u16, u16, TAG_U16);
    def_new_vmdata_func!(new_u32, as_u32, u32, TAG_U32);
    def_new_vmdata_func!(new_u64, as_u64, u64, TAG_U64);
    def_new_vmdata_func!(new_float, as_float, f64, TAG_FLOAT);
    def_new_vmdata_func!(new_bool, as_bool, bool, TAG_BOOL);
}


impl PartialEq for VMData {
    fn eq(&self, other: &Self) -> bool {
        if self.tag != other.tag {
            return false
        }

        match self.tag {
            Self::TAG_I8 => self.as_i8() == other.as_i8(),
            Self::TAG_I16 => self.as_i16() == other.as_i16(),
            Self::TAG_I32 => self.as_i32() == other.as_i32(),
            Self::TAG_I64 => self.as_i64() == other.as_i64(),
            Self::TAG_U8  => self.as_u8 () == other.as_u8 (),
            Self::TAG_U16 => self.as_u16() == other.as_u16(),
            Self::TAG_U32 => self.as_u32() == other.as_u32(),
            Self::TAG_U64 => self.as_u64() == other.as_u64(),
            Self::TAG_FLOAT => self.as_float() == other.as_float(),
            Self::TAG_UNIT => true,
            Self::TAG_BOOL => self.as_bool() == other.as_bool(),
            _ if self.tag > 256 => self.as_object() == other.as_object(),
            _ => panic!("reserved"),
        }
    }
}


impl Debug for VMData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VMData {{ tag: {}({})), data: {} }}",
            self.tag,
            match self.tag {
                Self::TAG_UNIT => "unit",
                Self::TAG_I8  => "i8",
                Self::TAG_I16 => "i16",
                Self::TAG_I32 => "i32",
                Self::TAG_I64 => "i64",
                Self::TAG_U8  => "u8",
                Self::TAG_U16 => "u16",
                Self::TAG_U32 => "u32",
                Self::TAG_U64 => "u64",
                Self::TAG_FLOAT => "float",
                Self::TAG_BOOL => "bool",
                
                _ if self.is_object() => "obj",
                _ => "res"
            },
            match self.tag {
                Self::TAG_UNIT => "()".to_string(),
                Self::TAG_I8 => self.as_i8().to_string(),
                Self::TAG_I16 => self.as_i16().to_string(),
                Self::TAG_I32 => self.as_i32().to_string(),
                Self::TAG_I64 => self.as_i64().to_string(),
                Self::TAG_U8 => self.as_u8().to_string(),
                Self::TAG_U16 => self.as_u16().to_string(),
                Self::TAG_U32 => self.as_u32().to_string(),
                Self::TAG_U64 => self.as_u64().to_string(),
                Self::TAG_FLOAT => self.as_float().to_string(),
                Self::TAG_BOOL => self.as_bool().to_string(),

                _ if self.is_object() => self.as_object().to_string(),
                _ => "reserved".to_string(),
            }
        )
    }
}


impl Display for VMData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self.tag {
            Self::TAG_UNIT => "()".to_string(),
            Self::TAG_I8 => self.as_i8().to_string(),
            Self::TAG_I16 => self.as_i16().to_string(),
            Self::TAG_I32 => self.as_i32().to_string(),
            Self::TAG_I64 => self.as_i64().to_string(),
            Self::TAG_U8 => self.as_u8().to_string(),
            Self::TAG_U16 => self.as_u16().to_string(),
            Self::TAG_U32 => self.as_u32().to_string(),
            Self::TAG_U64 => self.as_u64().to_string(),
            Self::TAG_FLOAT => self.as_float().to_string(),
            Self::TAG_BOOL => self.as_bool().to_string(),
            
            _ if self.is_object() => self.as_object().to_string(),
            _ => "reserved".to_string(),
        })
    }
}


/// The runtime union of stack values
#[repr(C)]
#[derive(Clone, Copy)]
pub union RawVMData {
    as_unit: (),
    as_i8: i8,
    as_i16: i16,
    as_i32: i32,
    as_i64: i64,
    as_u8: u8,
    as_u16: u16,
    as_u32: u32,
    as_u64: u64,
    as_float: f64,
    as_bool: bool,
    as_object: ObjectIndex,
}


macro_rules! enum_variant_function {
    ($getter: ident, $is: ident, $variant: ident, $ty: ty) => {
        #[inline(always)]
        #[must_use]
        pub fn $getter(self) -> $ty {
            unsafe { self.data.$getter }
            // match self.tag {
            //     Self::$variant => unsafe { self.data.$getter },
            //     _ => unreachable!()
            // }
        }


        #[inline(always)]
        #[must_use]
        pub fn $is(self) -> bool {
            self.tag == Self::$variant
        }
    }
}


#[allow(clippy::inline_always)]
impl VMData {
    enum_variant_function!(as_i8 , is_i8 , TAG_I8 , i8);
    enum_variant_function!(as_i16, is_i16, TAG_I16, i16);
    enum_variant_function!(as_i32, is_i32, TAG_I32, i32);
    enum_variant_function!(as_i64, is_i64, TAG_I64, i64);
    enum_variant_function!(as_u8 , is_u8 , TAG_U8 , u8);
    enum_variant_function!(as_u16, is_u16, TAG_U16, u16);
    enum_variant_function!(as_u32, is_u32, TAG_U32, u32);
    enum_variant_function!(as_u64, is_u64, TAG_U64, u64);

    enum_variant_function!(as_float, is_float, TAG_FLOAT, f64);
    enum_variant_function!(as_bool, is_bool, TAG_BOOL, bool);


    #[inline(always)]
    #[must_use]
    pub fn is_object(self) -> bool {
        self.tag > 256 || self.tag == Self::TAG_STR
    }

    pub fn as_object(self) -> ObjectIndex {
        if !self.is_object() {
            unreachable!()
        }

        unsafe { self.data.as_object }
    }
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
        objects: ObjectMap::new((8 * 1000 * 1000) / size_of::<Object>()),
        
        callstack: Vec::with_capacity(128),
        current: Code::new(bytecode, 0, 0),
        libraries: Vec::with_capacity(metadata.library_count as usize),
        externs: Vec::with_capacity(metadata.extern_count as usize),
        
        debug: Default::default(),
        metadata,
    };

    if let Err(e) = bytes_to_constants(&mut vm, constants) {
        println!(
            "{}",
            format!("panicked at '{}'", e.read_message().to_string_lossy()).bright_red()
        );
    }

    let start = Instant::now();

    let vm = Mutex::new(vm);

    std::panic::set_hook(Box::new(|a| {
        let loc = a.location().unwrap();
        let message = if let Some(v) = a.payload().downcast_ref::<&str>() {
            v.to_string()
        } else if let Some(v) = a.payload().downcast_ref::<String>() {
            v.clone()
        } else {
            String::from("no message provided")
        };

        *PANIC_INFO.lock().unwrap() = Some(((loc.file().to_owned(), loc.line(), loc.column()), message))
    }));

    
    let v = catch_unwind(|| {
        vm.lock().unwrap().run()
    });


    if v.is_err() {
        println!("a panic occurred in the runtime while running this program");
        vm.clear_poison();
        let vm = vm.into_inner().unwrap();
        let log = generate_panic_log(&vm, false);
        let mut write_to_stdout = true;
        if let Ok(current_dir) = env::current_dir() {
            let path = current_dir.join("panic_log.txt");
            write_to_stdout = std::fs::write(&path, log.as_bytes()).is_err(); 
            if !write_to_stdout {
                println!("the log file is located at {} please send this file to the azurite developer as soon as possible. the contact information is in the azurite github repo. https://github.com/rookieCookies/azurite/", path.to_string_lossy());
            }
            
        }

        if write_to_stdout {
            println!("failed to write to a log file, printing to stdout");
            let mut lock = std::io::stdout().lock();
            std::io::Write::write_all(&mut lock, log.as_bytes()).unwrap();
            std::io::Write::flush(&mut lock).unwrap();
        }

        return
    }
    
    let vm = vm.into_inner().unwrap();

    let end = start.elapsed();
    println!("it took {}ms {}ns, result {}", end.as_millis(), end.as_nanos(), vm.stack.reg(0));


    if env::var(azurite_common::environment::PANIC_LOG).unwrap_or("0".to_string()) == "1" {
        let log = generate_panic_log(&vm, true);
        let mut write_to_stdout = true;
        if let Ok(current_dir) = env::current_dir() {
            let path = current_dir.join("panic_log.txt");
            write_to_stdout = std::fs::write(&path, log.as_bytes()).is_err(); 
            if !write_to_stdout {
                println!("the log file is located at {}", path.to_string_lossy());
            }
            
        }

        if write_to_stdout {
            println!("failed to write to a log file, printing to stdout");
            let mut lock = std::io::stdout().lock();
            std::io::Write::write_all(&mut lock, log.as_bytes()).unwrap();
            std::io::Write::flush(&mut lock).unwrap();
        }
        
    }
    

}


fn bytes_to_constants(vm: &mut VM, data: Vec<u8>) -> Result<(), FatalError> {
    let mut constants_iter = data.into_iter();

    while let Some(datatype) = constants_iter.next() {
        let constant = match datatype {
            0 => VMData::new_float(f64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),

            1 => VMData::new_bool(constants_iter.next().unwrap() == 1),

            2 => {
                let length = u64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap());

                let mut vec = Vec::with_capacity(length as usize);
                for _ in 0..length {
                    vec.push(constants_iter.next().unwrap());
                }

                let object = String::from_utf8(vec).unwrap();
                
                let index = vm.create_object(Object::new(object))?;

                VMData::new_object(11, index)
            }

            3  => VMData::new_i8 (i8 ::from_le_bytes(constants_iter.next_chunk::<1>().unwrap())),
            4  => VMData::new_i16(i16::from_le_bytes(constants_iter.next_chunk::<2>().unwrap())),
            5  => VMData::new_i32(i32::from_le_bytes(constants_iter.next_chunk::<4>().unwrap())),
            6  => VMData::new_i64(i64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),
            7  => VMData::new_u8 (u8 ::from_le_bytes(constants_iter.next_chunk::<1>().unwrap())),
            8  => VMData::new_u16(u16::from_le_bytes(constants_iter.next_chunk::<2>().unwrap())),
            9  => VMData::new_u32(u32::from_le_bytes(constants_iter.next_chunk::<4>().unwrap())),
            10 => VMData::new_u64(u64::from_le_bytes(constants_iter.next_chunk::<8>().unwrap())),

            _ => unreachable!()
        };

        vm.constants.push(constant);
    };
    Ok(())
}


struct VMDebugInfo {
    last_gc_time: SystemTime,
    last_gc_duration: Duration,
    total_gc_count: u64,
}


impl Default for VMDebugInfo {
    fn default() -> Self {
        Self {
            last_gc_time: SystemTime::now(),
            last_gc_duration: Duration::ZERO,
            total_gc_count: 0,
        }
    }
}


fn generate_panic_log(vm: &VM, forced: bool) -> String {
    let mut string = String::new();

    if !forced {
        let lock = PANIC_INFO.lock().unwrap();
        let panic_info = lock.as_ref().unwrap();

        let _ = writeln!(string, " - - - - - - - - - - - - - PANIC INFO - - - - - - - - - - - - - ");
        let _ = writeln!(string, "time: {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        let _ = writeln!(string);
        let _ = writeln!(string, "file: {}", panic_info.0.0);
        let _ = writeln!(string, "line: {}", panic_info.0.1);
        let _ = writeln!(string, "column: {}", panic_info.0.2);
        let _ = writeln!(string, "message: {}", &panic_info.1);
        let _ = writeln!(string);
    }
    
    let _ = writeln!(string, " - - - - - - - - - - - - -  VM STATE  - - - - - - - - - - - - - ");
    let _ = writeln!(string);
    let _ = writeln!(string, "constants:");
    let _ = writeln!(string, "\tsize: {}", vm.constants.len());
    let _ = writeln!(string, "\tcapacity: {}", vm.constants.capacity());
    
    let _ = writeln!(string, "\tvalues:");

    {
        let alignment = vm.constants.len().to_string().len();
        for constant in vm.constants.iter().enumerate() {
            let _ = writeln!(string, "\t\t{:>alignment$} - {:?}", constant.0, constant.1);
        }
    }

    
    let _ = writeln!(string);
    let _ = writeln!(string, "heap:");
    {
        let bytes = std::mem::size_of_val(vm.objects.raw());
        let _ = writeln!(string, "\ttotal memory capacity: {}b/{}kb/{}mb/{}gb", bytes, bytes / 1000, bytes / 1000 / 1000, bytes / 1000 / 1000 / 1000 );
    }
    {
        let bytes = vm.memory_usage();
        let _ = writeln!(string, "\ttotal memory usage: {}b/{}kb/{}mb/{}gb", bytes, bytes / 1000, bytes / 1000 / 1000, bytes / 1000 / 1000 / 1000 );
    }

    let _ = write!(string, "\tlast gc time: ");
    if vm.debug.total_gc_count == 0 {
        let _ = writeln!(string, "nan");
    } else {
        let _ = writeln!(string, "{}", vm.debug.last_gc_time.duration_since(UNIX_EPOCH).unwrap().as_millis());
    }

    let _ = writeln!(string, "\tlast gc duration: {}", vm.debug.last_gc_duration.as_millis());
    let _ = writeln!(string, "\ttotal gc count: {}", vm.debug.total_gc_count);

    let _ = writeln!(string, "\tobjects:");
    let _ = writeln!(string, "\t\t-- default objects are excluded --");
    for object in vm.objects.raw().iter().enumerate() {
        if let ObjectData::Free { next } = object.1.data {
            if next == ObjectIndex::new((object.0 as u64 + 1) % vm.objects.raw().len() as u64) {
                continue
            }
        }

        let _ = writeln!(string, "\t\t{} - live: {} data: {:?}", object.0, object.1.liveliness_status.take(), object.1.data);
    }

    let _ = writeln!(string);

    
    let _ = writeln!(string, "stack:");
    let _ = writeln!(string, "\tsize: {}", vm.stack.values.len());
    let _ = writeln!(string, "\tstack offset: {}", vm.stack.stack_offset);
    let _ = writeln!(string, "\ttop: {}", vm.stack.top);

    let _ = writeln!(string, "\tvalues:");
    for stack_val in vm.stack.values.iter().take(vm.stack.top).enumerate().rev() {
        let _ = writeln!(string, "\t\t{:>w$} - {:?}", stack_val.0, stack_val.1, w = vm.stack.top);
    }

    let _ = writeln!(string);

    let _ = writeln!(string, "callstack:");
    let _ = writeln!(string, "\tcurrent - ip: {} ret: {} saved stack offset: {}", vm.current.pointer, vm.current.return_to, vm.current.offset);

    {
        let w = vm.callstack.len().to_string().len();
        for (index, c) in vm.callstack.iter().enumerate().rev() {
            let _ = writeln!(string, "\t{index:>w$} - ip: {} ret: {} saved stack offset: {}", c.pointer, c.return_to, c.offset);
        }
    }

    let _ = writeln!(string);


    let _ = writeln!(string, "dyn libraries");
    let _ = writeln!(string, "\tloaded libs: {}", vm.libraries.len());
    let _ = writeln!(string, "\tloaded ext funcs: {}", vm.externs.len());
    let _ = writeln!(string, "\tloaded libs matches expected: {}", vm.libraries.len() == vm.metadata.library_count as usize);
    let _ = writeln!(string, "\tloaded ext funcs matches expected: {}", vm.externs.len() == vm.metadata.extern_count as usize);
    let _ = writeln!(string);

    let _ = writeln!(string, "compilation metadata:");
    let _ = writeln!(string, "\tlibrary count: {}", vm.metadata.library_count);
    let _ = writeln!(string, "\textern count: {}", vm.metadata.extern_count);
    let _ = writeln!(string);

    let _ = writeln!(string, "bytecode:");
    let _ = writeln!(string, "{:?}", vm.current.code.to_vec());
   

    string
}