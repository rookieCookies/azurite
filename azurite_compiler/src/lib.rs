#![warn(clippy::pedantic)]
mod ast;
mod compiler;
mod error;
mod lexer;
mod lexer_tests;
mod parser;
mod static_analysis;
mod utils;

use std::{
    fs::{self, File},
    io::Write,
    process::ExitCode,
};

use azurite_archiver::{Packed, Data as ArchiverData};
use azurite_common::{Data, FileData};
use colored::Colorize;
use compiler::{compile, Compilation};

/// # Errors
pub fn run_file(file: &str) -> Result<(), ExitCode> {
    let data = if let Ok(v) = fs::read_to_string(&file) {
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

    let file = if let Ok(v) = File::create(format!("{name}.azurite")) {
        v
    } else {
        eprintln!("unable to create {name}.azurite");
        return Err(ExitCode::FAILURE);
    };

    if create_file(compilation, file).is_err() {
        return Err(ExitCode::FAILURE);
    };
    Ok(())
}

fn create_file(compilation: Compilation, mut file: File) -> Result<(), ()> {
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

    let mut line_data: Vec<u8> = Vec::with_capacity(compilation.line_table.len());
    compilation.line_table.into_iter().for_each(|x| {
        line_data.append(&mut x.to_le_bytes().into());
    });

    let packed = Packed::new()
        .with(ArchiverData(compilation.bytecode))
        .with(ArchiverData(data))
        .with(ArchiverData(line_data));

    if file.write_all(&packed.as_bytes()).is_err(){
        eprintln!("unable to write to the file");
        return Err(())
    };

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub identifiers: Vec<String>,
}
