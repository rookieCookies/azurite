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

    #[cfg(not(afl))]
    let is_release_mode = env::var(environment::RELEASE_MODE).map(|v| v == "1").unwrap_or(false);
    #[cfg(afl)]
    let is_release_mode = false;
    let mut line_data: Vec<u8> = Vec::with_capacity(compilation.line_table.len());

    if !is_release_mode {
        let mut counter : u32 = 0;
        let mut number : u32 = 0;
        for i in compilation.line_table {
            if i != number {
                line_data.append(&mut counter.to_le_bytes().into());
                line_data.append(&mut number.to_le_bytes().into());

                counter = 0;
                number = i;
            }

            counter += 1;
        }

        line_data.append(&mut counter.to_le_bytes().into());
        line_data.append(&mut number.to_le_bytes().into());

    }

    let packed = Packed::new()
        .with(ArchiverData(compilation.bytecode))
        .with(ArchiverData(data))
        .with(ArchiverData(line_data));

    Ok(packed)
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub identifiers: Vec<String>,
}
