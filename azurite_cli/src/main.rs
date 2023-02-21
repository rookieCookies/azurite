#![warn(clippy::pedantic)]
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::{env, io::{Read, Write}, vec::IntoIter, process::ExitCode, path::Path, fs::File};

use azurite_common::Bytecode;
use azurite_runtime::{load_constants, vm::VM};
use colored::{Colorize, Color};

use rustyline::{error::ReadlineError, validate::MatchingBracketValidator, Editor};
use rustyline::{Cmd, EventHandler, KeyCode, KeyEvent, Modifiers};
use rustyline_derive::{Completer, Helper, Highlighter, Hinter, Validator};

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), ExitCode> {
    let mut args = env::args();
    args.next();
    let argument = match args.next() {
        Some(v) => v,
        None => invalid_usage(),
    };

    match argument.as_str() {
        "repl" => {
            // Using Repl
            let h = InputValidator {
                brackets: MatchingBracketValidator::new(),
            };
            let mut rl = Editor::new().unwrap();
            rl.set_helper(Some(h));
            rl.bind_sequence(
                KeyEvent(KeyCode::Enter, Modifiers::CTRL),
                EventHandler::Simple(Cmd::Newline),
            );
            if rl.load_history("repl/history.txt").is_err() {
                println!("No previous history.");
            }

            if !Path::new("repl").is_dir() {
                std::fs::create_dir("repl").unwrap();
            }

            if Path::new("repl/repl.az").is_file() {
                std::fs::remove_file("repl/repl.az").unwrap();
            }

            File::create("repl/repl.az").unwrap();
            println!("repl.az created");

            loop {
                // Repl prompt
                let readline = rl.readline(&"azurite $ ".color(Color::TrueColor { r:80, g:80, b:80 }));
                match readline {
                    Ok(line) => {
                        // Rustlyline History support
                        rl.add_history_entry(line.as_str()).unwrap();
                        rl.save_history("repl/history.txt").unwrap();

                        // Basic repl commands to check
                        if line.to_lowercase() == "exit" {
                            break;
                        };
                        if line.to_lowercase() == "reset" {
                            if Path::new("repl/repl.az").is_file() {
                                std::fs::remove_file("repl/repl.az").unwrap();
                            }
                            File::create("repl/repl.az").unwrap();
                            continue;
                        };
                        let f = OpenOptions::new()
                            .write(true)
                            .append(true)
                            .open("repl/repl.az")
                            .expect("unable to open file");
            
                        let mut f = BufWriter::new(f);
                        writeln!(f, "{line}").expect("unable to open repl.az");
                        drop(f);
                        
                        let file = "repl/repl.az";
                        azurite_compiler::run_file(file)?;
                        let file = format!("{file}urite");
                        azurite_runtime::run_file(&file)?;

                    }
                    Err(ReadlineError::Eof | ReadlineError::Interrupted) => {
                        break;
                    }
                    Err(err) => {
                        println!("Error: {:?}", err);
                    }
                }
            }
        }
        "build" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            
            compile(&file)?;
        }
        "run" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            let file_path = Path::new(&file);
            if file_path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("az")) {
                compile(&file)?;
                let file = format!("{}urite", file);

                println!("{} {file}", "Running..".bright_green().bold());
                let _ = azurite_runtime::run_file(&file);
            } else {
                println!("{} {file}", "Running..".bright_green().bold());
                let _ = azurite_runtime::run_file(&file);
            }
        }
        "disassemble" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            
            compile(&file)?;

            let file = format!("{file}urite");
            println!("{} {file}", "Disassembling..".bright_green().bold());

            let zipfile = std::fs::File::open(&file).unwrap();

            let mut archive = zip::ZipArchive::new(zipfile).unwrap();
        
            let mut bytecode_file = if let Ok(file) = archive.by_name("bytecode.azc") {
                file
            } else {
                println!("bytecode.azc not found");
                return Ok(())
            };
        
            let mut bytecode = vec![];
            match bytecode_file.read_to_end(&mut bytecode) {
                Ok(_) => {}
                Err(_) => return Ok(()),
            };
            
            drop(bytecode_file);
            println!("{}", disassemble(bytecode.into_iter()));
        }
        "constants" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            
            compile(&file)?;

            let file = format!("{file}urite");
            println!("{} {file}", "Disassembling constants..".bright_green().bold());

            let zipfile = std::fs::File::open(&file).unwrap();

            let mut archive = zip::ZipArchive::new(zipfile).unwrap();
        
            let mut constants_file = if let Ok(file) = archive.by_name("constants.azc") {
                file
            } else {
                println!("constants.azc not found");
                return Ok(())
            };
        
            let mut constants = vec![];
            match constants_file.read_to_end(&mut constants) {
                Ok(_) => {}
                Err(_) => return Ok(()),
            };
            
            drop(constants_file);
            let mut vm = match VM::new() {
                Ok(v) => v,
                Err(err) => {
                    err.trigger(&file).unwrap();
                    return Ok(())
                },
            };
            println!("{:?}", load_constants(constants, &mut vm));
        }
        _ => invalid_usage(),
    }
    Ok(())

    // Some(())
}

fn invalid_usage() -> ! {
    println!("{}: please provide a sub-command (build, run, disassemble, constants, repl) followed by a file name", "invalid usage".red().bold());
    std::process::exit(1)
}

fn compile(file: &str) -> Result<(), ExitCode> {
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
                depth -= 1;
            }
            Bytecode::ReturnFromFunction => {
                disassemble.push_str("return from function");
            }
            Bytecode::LoadConst => {
                disassemble.push_str(format!("load const {}", v.next().unwrap()).as_str());
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
                disassemble.push_str(&format!("get var fast {}", v.next().unwrap()));
            }
            Bytecode::ReplaceVar => disassemble.push_str(&format!(
                "replace var {}",
                u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()])
            )),
            Bytecode::ReplaceVarFast => {
                disassemble.push_str(&format!("replace var fast {}", v.next().unwrap()));
            }
            Bytecode::ReplaceVarInObject => {
                let size = v.next().unwrap();

                disassemble.push_str(&format!(
                    "replace var in object {size} - {}",
                    &(0..size)
                        .map(|_| format!("{} ", v.next().unwrap()))
                        .collect::<String>()
                ));
            }
            Bytecode::Not => disassemble.push_str("not"),
            Bytecode::Negative => disassemble.push_str("negate"),
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

                disassemble.push_str(&format!("create struct {size}"));
            }
            Bytecode::AccessData => {
                disassemble.push_str(&format!("access data {}", v.next().unwrap()));
            }
            Bytecode::RawCall => disassemble.push_str(&format!("raw call {}", v.next().unwrap())),
        };
        disassemble.push('\n');
    }
}

#[derive(Completer, Helper, Highlighter, Hinter, Validator)]
struct InputValidator {
    #[rustyline(Validator)]
    brackets: MatchingBracketValidator,
}