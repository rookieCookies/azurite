#![feature(try_trait_v2)]
#![allow(unused)]
mod ast;
mod compiler;
mod error;
mod lexer;
mod lexer_tests;
mod parser;
mod static_analysis;

use std::{env, fs, io::Write, process::ExitCode};

use azurite_common::{prepare, parse_args, set_directory_to_current, Data, FileData, STRING_TERMINATOR};
use colored::Colorize;
use compiler::compile;
use zip::write::FileOptions;

fn main() -> ExitCode {
    prepare();
    let (file, environemnt_parameters) =
        match parse_args(env::args().collect::<Vec<_>>().into_iter()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };

    println!("{} {file}", "Compiling..".bright_green().bold());

    for parameter in environemnt_parameters {
        env::set_var(parameter.identifier, parameter.value)
    }

    match run_file(file) {
        Ok(_) => {}
        Err(_) => return ExitCode::FAILURE,
    };
    println!("{}", "Finished!".bright_green().bold());

    ExitCode::SUCCESS
}

fn run_file(file: String) -> Result<(), ()> {
    let data = match fs::read_to_string(&file) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("unable to locate provided file");
            return Err(());
        }
    };

    let file_data = FileData {
        path: file.clone(),
        data,
    };
    let compilation = match compile(file_data) {
        Ok(v) => v,
        Err(_) => return Err(()),
    };

    let name = file.split_once(".az").unwrap().0;
    let name = name.to_string();

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
            Data::Bool(v) => data.push(v as u8),
        }
    }

    let file = fs::File::create(format!("{name}.azurite")).unwrap();

    let mut zip = zip::ZipWriter::new(file);

    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    zip.start_file("bytecode.azc", options).unwrap();
    zip.write_all(&compilation.bytecode).unwrap();

    zip.start_file("constants.azc", options).unwrap();
    zip.write_all(&data).unwrap();

    let mut line_data: Vec<u8> = Vec::with_capacity(compilation.line_table.len());
    compilation.line_table.into_iter().for_each(|x| {
        line_data.append(&mut x.0.to_le_bytes().into());
        line_data.append(&mut x.1.to_le_bytes().into())
    });

    zip.start_file("linetable.azc", options).unwrap();
    zip.write_all(&line_data).unwrap();

    zip.finish().unwrap();

    Ok(())
}
