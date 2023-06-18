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
pub fn run_packed(packed: Packed) {
    let mut files : Vec<Data> = packed.into();

    let constants = files.pop().unwrap();
    let bytecode = files.pop().unwrap();
    let metadata = CompilationMetadata::from_bytes(files.pop().unwrap().0.try_into().unwrap());

    assert!(files.is_empty());

    run(metadata, &bytecode.0, constants.0);
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
            values: [VMData::Empty; STACK_SIZE],
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
        let log = generate_panic_log(&vm);
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
    println!("it took {}ms {}ns, result {:?}", end.as_millis(), end.as_nanos(), vm.stack.reg(0));

    // if let Status::Exit(v) = v {
    //     std::process::exit(v)
    // }
    

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


fn generate_panic_log(vm: &VM) -> String {
    let mut string = String::new();
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
        let bytes = vm.objects.raw().len() * size_of::<Object>();
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