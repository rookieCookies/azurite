use std::{env, io::Read, vec::IntoIter};

use azurite_common::Bytecode;
use colored::Colorize;

fn main() {
    let mut args = env::args();
    args.next();
    let argument = match args.next() {
        Some(v) => v,
        None => invalid_usage(),
    };

    match argument.as_str() {
        "build" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            
            let _ = compile(file);
        }
        "run" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            if file.ends_with(".az") {
                let _ = compile(file.clone());
                let file = format!("{file}urite");

                println!("{} {file}", "Running..".bright_green().bold());
                let _ = azurite_runtime::run_file(file);
            } else {
                println!("{} {file}", "Running..".bright_green().bold());
                let _ = azurite_runtime::run_file(file);
            }
        }
        "disassemble" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            
            let _ = compile(file.clone());

            println!("{} {file}", "Disassembling..".bright_green().bold());
            let file = format!("{file}urite");
            let zipfile = std::fs::File::open(&file).unwrap();

            let mut archive = zip::ZipArchive::new(zipfile).unwrap();
        
            let mut bytecode_file = match archive.by_name("bytecode.azc") {
                Ok(file) => file,
                Err(..) => {
                    println!("bytecode.azc not found");
                    return
                }
            };
        
            let mut bytecode = vec![];
            match bytecode_file.read_to_end(&mut bytecode) {
                Ok(_) => {}
                Err(_) => return,
            };
            
            drop(bytecode_file);
            println!("{}", disassemble(bytecode.into_iter()))
        }
        _ => invalid_usage(),
    }

    // Some(())
}

fn invalid_usage() -> ! {
    println!("{}: please provide a sub-command (build, run, disassemble) followed by a file name", "invalid usage".red().bold());
    std::process::exit(1)
}

fn compile(file: String) -> Result<(), ()> {
    println!("{} {file}", "Compiling..".bright_green().bold());
    azurite_compiler::run_file(file)?;
    println!("{}", "Finished!".bright_green().bold());
    Ok(())
}

fn disassemble(mut v: IntoIter<u8>) -> String {
    let mut depth = 0;
    let mut disassemble = String::new();
    loop {
        disassemble.push_str("    ".repeat(depth).as_str());
        match Bytecode::from_u8(v.next().expect(&disassemble)).unwrap() {
            Bytecode::Return => {
                disassemble.push_str("return");
                if depth == 0 {
                    if v.next().is_none() {
                        return disassemble;
                    }
                    panic!("{disassemble}")
                }
                depth -= 1
            }
            Bytecode::ReturnFromFunction => {
                disassemble.push_str("return from function");
            }
            Bytecode::LoadConst => {
                disassemble.push_str(format!("load const {}", v.next().unwrap()).as_str())
            }
            Bytecode::Add => disassemble.push_str("add"),
            Bytecode::Subtract => disassemble.push_str("subtract"),
            Bytecode::Multiply => disassemble.push_str("multiply"),
            Bytecode::Divide => disassemble.push_str("divide"),
            Bytecode::EqualsTo => disassemble.push_str("equals to"),
            Bytecode::NotEqualsTo => disassemble.push_str("not equals to"),
            Bytecode::GreaterThan => disassemble.push_str("greater than"),
            Bytecode::LesserThan => disassemble.push_str("lesser than"),
            Bytecode::GreaterEquals => disassemble.push_str("greater equals"),
            Bytecode::LesserEquals => disassemble.push_str("lesser equals"),
            Bytecode::GetVar => disassemble.push_str(&format!(
                "get var {}",
                u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()])
            )),
            Bytecode::GetVarFast => {
                disassemble.push_str(&format!("get var fast {}", v.next().unwrap()))
            }
            Bytecode::ReplaceVar => disassemble.push_str(&format!(
                "replace var {}",
                u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()])
            )),
            Bytecode::ReplaceVarFast => {
                disassemble.push_str(&format!("replace var fast {}", v.next().unwrap()))
            }
            Bytecode::ReplaceVarInObject => {
                let size = v.next().unwrap();

                disassemble.push_str(&format!(
                    "replace var in object {size} - {}",
                    &(0..size)
                        .map(|_| format!("{} ", v.next().unwrap()))
                        .collect::<String>()
                ))
            }
            Bytecode::Not => disassemble.push_str("not"),
            Bytecode::Negative => disassemble.push_str("negate"),
            Bytecode::Assert => todo!(),
            Bytecode::Pop => disassemble.push_str("pop"),
            Bytecode::PopMulti => disassemble.push_str(&format!("pop multi {}", v.next().unwrap())),
            Bytecode::JumpIfFalse => {
                disassemble.push_str(&format!("jump if false {}", v.next().unwrap()));
                // depth += 1;
            }
            Bytecode::Jump => disassemble.push_str(&format!("jump {}", v.next().unwrap())),
            Bytecode::JumpBack => disassemble.push_str(&format!("jump back {}", v.next().unwrap())),
            Bytecode::LoadFunction => {
                disassemble.push_str(&format!(
                    "load function (arg count: {}) (has return {}) {}",
                    v.next().unwrap(),
                    v.next().unwrap(),
                    v.next().unwrap()
                ));
                depth += 1;
            }
            Bytecode::CallFunction => {
                disassemble.push_str(&format!("call function {}", v.next().unwrap()));
            }
            Bytecode::CreateStruct => {
                let size = v.next().unwrap();

                disassemble.push_str(&format!("create struct {size}"))
            }
            Bytecode::AccessData => {
                disassemble.push_str(&format!("access data {}", v.next().unwrap()))
            }
            Bytecode::RawCall => disassemble.push_str(&format!("raw call {}", v.next().unwrap())),
        };
        disassemble.push('\n');
    }
}
