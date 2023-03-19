mod ast;
pub mod compiler;
mod error;
mod lexer;
mod lexer_tests;
mod parser;
mod static_analysis;
mod utils;

use std::{
    fs::{self, File},
    io::Write,
    process::ExitCode, env,
};

use azurite_archiver::{Packed, Data as ArchiverData};
use azurite_common::{Data, FileData, environment};
use colored::Colorize;
use compiler::{compile, Compilation};

/// # Errors
pub fn run_file(file: &str) -> Result<(), ExitCode> {
    let data = if let Ok(v) = fs::read_to_string(file) {
        v
    } else {
        eprintln!("{}", "unable to locate provided file".red().bold());
        return Err(ExitCode::FAILURE);
    };

    let file_data = FileData {
        path: file.to_owned(),
        data,
    };

    let compilation = match compile(file_data) {
        Ok(v) => v,
        Err(_) => return Err(ExitCode::FAILURE),
    };

    let name = if let Some(v) = file.split_once(".az") {
        v.0
    } else {
        eprintln!("file doesn't have a .az extension");
        return Err(ExitCode::FAILURE);
    };
    let name = name.to_string();

    let mut file = if let Ok(v) = File::create(format!("{name}.azurite")) {
        v
    } else {
        eprintln!("unable to create {name}.azurite");
        return Err(ExitCode::FAILURE);
    };

    if let Ok(v) = create_file(compilation) {
        file.write_all(&v.as_bytes()).unwrap()
    }
    Ok(())
}

#[allow(clippy::result_unit_err)]
pub fn create_file(compilation: Compilation) -> Result<Packed, ()> {
    // Convert the constants from an enum
    // representation to a byte representation
    let mut data: Vec<u8> = vec![];
    for item in compilation.constants {
        data.push(item.type_representation().into_byte_representation());
        match item {
            Data::Integer(v) => data.extend(v.to_le_bytes()),
            Data::Float(v) => data.extend(v.to_le_bytes()),
            Data::String(v) => {
                let value : u32 = v.len().try_into().expect("constant string too big");
                let values = value.to_le_bytes();
                data.push(values[0]);
                data.push(values[1]);
                data.push(values[2]);
                data.push(values[3]);
                data.extend(v.as_bytes());
            }
            Data::Bool(v) => data.push(u8::from(v)),
        }
    }


    let mut packed = Packed::new()
        .with(ArchiverData(compilation.bytecode))
        .with(ArchiverData(data));

    #[cfg(not(afl))]
    let is_release_mode = env::var(environment::RELEASE_MODE).map(|v| v == "1").unwrap_or(false);
    #[cfg(afl)]
    let is_release_mode = false;

    if !is_release_mode {

        // Line table mapping each instruction to a line number
        {
            let mut line_data: Vec<u8> = Vec::with_capacity(compilation.instruction_debug_table.len());
            line_data.push(0);
            line_data.push(0);
            line_data.push(0);
            line_data.push(0);

            for i in compilation.instruction_debug_table {
                line_data.append(&mut i.line.to_le_bytes().into());
            }

            packed = packed.with(ArchiverData(line_data));
        }


        // Function table
        {
            let mut function_data: Vec<u8> = vec![];
            for i in compilation.function_debug_table {
                let size = i.len() as u8;
                let data = i.as_bytes();

                function_data.push(size);
                function_data.append(&mut data.into());
            }

            packed = packed.with(ArchiverData(function_data));
        }
    }

    Ok(packed)
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub identifiers: Vec<String>,
}
