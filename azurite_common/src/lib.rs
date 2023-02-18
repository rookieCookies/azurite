use std::{env, fmt::Display, fs, process::ExitCode, vec::IntoIter};

use colored::Colorize;

macro_rules! opcode {
    ( $(#[$attr:meta])* $vis:vis enum $name:ident : $type:ty {
        $($variant:ident),* $(,)?
    } ) => {
        #[repr($type)]
        $(#[$attr])*
        $vis enum $name {
            $($variant,)*
        }
        #[allow(non_upper_case_globals)]
        mod consts {
            $(pub const $variant: $type = super::$name::$variant as $type;)*
        }
        impl $name {
            #[inline(always)]
            pub fn as_u8(self) -> $type { self as _ }

            #[inline(always)]
            pub fn from_u8(value: $type) -> Option<Self> {
                match value {
                    $(consts::$variant => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

pub fn prepare() {
    #[cfg(windows)]
    {
        use colored::control::set_virtual_terminal;
        set_virtual_terminal(true).unwrap();
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct EnvironmentParameter {
    pub identifier: String,
    pub value: String,
}

pub fn parse_args(
    mut arguments: IntoIter<String>,
) -> Result<(String, Vec<EnvironmentParameter>), String> {
    // The first argument is *always* the path of the executable
    arguments.next();

    let provided_file_name = match arguments.next() {
        Some(v) => v,
        None => {
            return Err(format!(
                "{} azurite %file name%",
                "invalid usage:".red().bold()
            ))
        }
    };

    // Parsing arguments
    let mut environment_parameters = Vec::new();
    for arg in arguments {
        if arg.starts_with("--") {
            // We split to ensure the two dashes are removed from the argument's id
            let arg = arg.split_at(2).1;
            let (id, value) = match arg.split_once('=') {
                Some((id, value)) => (id, value),
                None => (arg, "1"),
            };
            environment_parameters.push(EnvironmentParameter {
                identifier: id.to_string(),
                value: value.to_string(),
            })
        }
    }
    Ok((provided_file_name, environment_parameters))
}

pub fn set_directory_to_current() -> Result<(), ExitCode> {
    let current_executable = match std::env::current_exe() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("failed to find the current executable");
            return Err(ExitCode::FAILURE);
        }
    };

    let mut file_path = match fs::canonicalize(current_executable) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("failed to find the canonical path for the current executable");
            return Err(ExitCode::FAILURE);
        }
    };

    file_path.pop();
    match env::set_current_dir(file_path) {
        Ok(_) => (),
        Err(_) => {
            eprintln!("failed to update the current environment directory");
            return Err(ExitCode::FAILURE);
        }
    };

    Ok(())
}

#[derive(Debug, Default)]
pub struct FileData {
    pub path: String,
    pub data: String,
}

impl FileData {
    pub fn new(path: String, data: String) -> Self {
        Self { path, data }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

#[derive(Clone, PartialEq, Debug)]
pub enum DataType {
    Integer,
    Float,
    String,
    Bool,
    Struct(String),

    Empty,
}

impl DataType {
    pub fn byte_representation(&self) -> u8 {
        match self {
            DataType::Integer => 0,
            DataType::Float => 1,
            DataType::String => 2,
            DataType::Bool => 3,
            DataType::Struct(_) => 4,
            DataType::Empty => panic!("empty types should not get past static analysis"),
        }
    }
}

impl Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DataType::Integer => "int",
                DataType::Float => "float",
                DataType::String => "str",
                DataType::Bool => "bool",
                DataType::Struct(type_name) => type_name.as_str(),
                DataType::Empty => "()",
            }
        )
    }
}

impl TryFrom<u8> for DataType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Integer,
            1 => Self::Float,
            2 => Self::String,
            3 => Self::Bool,
            _ => return Err(value),
        })
    }
}

impl DataType {
    pub fn from_string(v: &str) -> DataType {
        match v {
            "int" => DataType::Integer,
            "bool" => DataType::Bool,
            "float" => DataType::Float,
            "str" => DataType::String,
            _ => DataType::Struct(v.to_string()),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            DataType::Integer => 8,
            DataType::Float => 8,
            DataType::String => usize::MAX,
            DataType::Bool => 1,
            DataType::Struct(_) => 0,
            DataType::Empty => panic!("empty types should not get past static analysis"),
        }
    }
}

impl Data {
    pub fn type_representation(&self) -> DataType {
        match self {
            Data::Integer(_) => DataType::Integer,
            Data::Float(_) => DataType::Float,
            Data::String(_) => DataType::String,
            Data::Bool(_) => DataType::Bool,
        }
    }
}

pub const STRING_TERMINATOR: u8 = 255;

opcode! {
// #[repr(u8)]
#[derive(Hash, PartialEq, Eq, Debug)]
pub enum Bytecode : u8 {
    Return,
    ReturnFromFunction,
    LoadConst,
    Add,
    Subtract,
    Multiply,
    Divide,
    EqualsTo,
    NotEqualsTo,
    GreaterThan,
    LesserThan,
    GreaterEquals,
    LesserEquals,
    GetVar,
    GetVarFast,
    ReplaceVar,
    ReplaceVarFast,
    ReplaceVarInObject,
    Not,
    Negative,
    Assert,
    Pop,
    PopMulti,
    JumpIfFalse,
    Jump,
    JumpBack,
    LoadFunction,
    CallFunction,
    CreateStruct,
    AccessData,
    RawCall,
}
}
