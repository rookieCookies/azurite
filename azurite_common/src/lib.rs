use std::{env, fmt::Display, fs, process::ExitCode, vec::IntoIter};

use colored::Colorize;


pub mod environment {
    pub const RELEASE_MODE : &str = "AZURITE_COMPILER_RELEASE_MODE";
    
    pub const DUMP_IR      : &str = "AZURITE_COMPILER_DUMP_IR";
    pub const DUMP_IR_FILE : &str = "AZURITE_COMPILER_DUMP_IR_FILE";
}


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
        pub mod consts {
            $(pub const $variant: $type = super::$name::$variant as $type;)*
        }
        impl $name {
            #[inline(always)]
            #[must_use]
            pub fn as_u8(self) -> $type { self as _ }

            #[inline(always)]
            #[must_use]
            pub fn from_u8(value: $type) -> Option<Self> {
                match value {
                    $(consts::$variant => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

/// # Panics
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

/// # Errors
/// This function will error if the arguments are invalid
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
            });
        }
    }
    Ok((provided_file_name, environment_parameters))
}

/// # Errors
/// This functiuon will error if it can't find the directories
pub fn set_directory_to_current() -> Result<(), ExitCode> {
    let current_executable = if let Ok(v) = std::env::current_exe() {
        v
    } else {
        eprintln!("failed to find the current executable");
        return Err(ExitCode::FAILURE);
    };

    let mut file_path = if let Ok(v) = fs::canonicalize(current_executable) {
        v
    } else {
        eprintln!("failed to find the canonical path for the current executable");
        return Err(ExitCode::FAILURE);
    };

    file_path.pop();
    if env::set_current_dir(file_path).is_err() {
        eprintln!("failed to update the current environment directory");
        return Err(ExitCode::FAILURE);
    };

    Ok(())
}

#[derive(Debug, Default)]
pub struct FileData {
    pub path: String,
    pub data: String,
}

impl FileData {
    #[must_use]
    pub fn new(path: String, data: &str) -> Self {
        let data = data.replace('\r', "");
        Self { path, data }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DataType {
    Integer,
    Float,
    String,
    Bool,
    Struct(String),

    Empty,
}

impl DataType {
    /// # Panics
    /// This function will panic if the `DataType` is `DataType::Empty`
    #[must_use]
    pub fn into_byte_representation(&self) -> u8 {
        match self {
            DataType::Integer => 0,
            DataType::Float => 1,
            DataType::String => 2,
            DataType::Bool => 3,
            DataType::Struct(_) => 4,
            DataType::Empty => panic!("empty types should not get past static analysis"),
        }
    }

    #[must_use]
    pub fn from_byte_representation(v: u8) -> Option<DataType> {
        Some(match v {
                    0 => DataType::Integer,
                    1 => DataType::Float,
                    2 => DataType::String,
                    3 => DataType::Bool,
                    _ => return None
                })
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
    #[must_use]
    pub fn from_string(v: &str) -> DataType {
        match v {
            "()" => DataType::Empty,
            "int" => DataType::Integer,
            "bool" => DataType::Bool,
            "float" => DataType::Float,
            "str" => DataType::String,
            _ => DataType::Struct(v.to_string()),
        }
    }

    /// # Panics
    /// This function will panic if the `DataType` is `DataType::Empty`
    #[must_use]
    pub const fn size(&self) -> usize {
        match self {
            DataType::Integer | DataType::Float => 8,
            DataType::String => usize::MAX,
            DataType::Bool => 1,
            DataType::Struct(_) => 0,
            DataType::Empty => panic!("empty types should not get past static analysis"),
        }
    }
}

impl Data {
    #[must_use]
    pub fn type_representation(&self) -> DataType {
        match self {
            Data::Integer(_) => DataType::Integer,
            Data::Float(_) => DataType::Float,
            Data::String(_) => DataType::String,
            Data::Bool(_) => DataType::Bool,
        }
    }
}

opcode! {
// #[repr(u8)]
#[derive(Hash, PartialEq, Eq, Debug)]
pub enum Bytecode : u8 {
    Return,
    Copy,
    Swap,

    Call,
    ExtCall,
    Push,
    Pop,

    ExternFile,
    
    Struct,
    AccStruct,
    SetField,
    
    Add,
    Subtract,
    Multiply,
    Divide,

    Equals,
    NotEquals,
    GreaterThan,
    LesserThan,
    GreaterEquals,
    LesserEquals,
    
    LoadConst,
    Unit,

    Jump,
    JumpCond,
}

}
