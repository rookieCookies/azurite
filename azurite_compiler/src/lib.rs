#![warn(clippy::pedantic)]
mod ast;
mod compiler;
mod error;
mod lexer;
mod lexer_tests;
mod parser;
mod static_analysis;

use std::{fs::{self, File}, io::Write, process::ExitCode};

use azurite_common::{Data, FileData, STRING_TERMINATOR};
use colored::Colorize;
use compiler::{compile, Compilation};
use zip::write::FileOptions;

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
            return Err(ExitCode::FAILURE)
    };
    let name = name.to_string();

    let file = if let Ok(v) = File::create(format!("{name}.azurite")) {
        v
    } else {
        eprintln!("unable to create {name}.azurite");
        return Err(ExitCode::FAILURE)
    };

    if create_file(compilation, file).is_err() {
        return Err(ExitCode::FAILURE)
    };
    Ok(())
}

fn create_file(compilation: Compilation, file: File) -> Result<(), ()> {
    // Convert the constants from an enum
    // representation to a byte representation
    let mut data: Vec<u8> = vec![];
    for item in compilation.constants {
        data.push(item.type_representation().byte_representation());
        match item {
            Data::Integer(v) => data.extend(v.to_le_bytes()),
            Data::Float(v) => data.extend(v.to_le_bytes()),
            Data::String(v) => {
                data.extend(v.as_bytes());
                data.push(STRING_TERMINATOR); // terminate character
            }
            Data::Bool(v) => data.push(u8::from(v)),
        }
    }


    let mut zip = zip::ZipWriter::new(file);

    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    create_and_write(&mut zip, options, "bytecode.azc", &compilation.bytecode)?;
    create_and_write(&mut zip, options, "constants.azc", &data)?;

    let mut line_data: Vec<u8> = Vec::with_capacity(compilation.line_table.len());
    compilation.line_table.into_iter().for_each(|x| {
        line_data.append(&mut x.0.to_le_bytes().into());
        line_data.append(&mut x.1.to_le_bytes().into());
    });

    create_and_write(&mut zip, options, "linetable.azc", &line_data)?;

    match zip.finish() {
        Ok(v) => v,
        Err(_) => return Err(()),
    };
    Ok(())
}

fn create_and_write(zip: &mut zip::ZipWriter<File>, options: FileOptions, path: &str, data: &[u8]) -> Result<(), ()> {
    if zip.start_file(path, options).is_err() {
        eprintln!("unable to create {path}");
        return Err(())
    }

    if zip.write_all(data).is_err() {
        eprintln!("unable to write to {path}");
        return Err(())
    }
    Ok(())
}