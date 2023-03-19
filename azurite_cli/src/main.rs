use std::env::Args;
use std::fs;
use std::time::Instant;
use std::{
    env,
    path::Path,
    process::ExitCode,
    vec::IntoIter,
};

use azurite_archiver::Packed;
use azurite_common::{consts, prepare, environment};
use colored::Colorize;

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), ExitCode> {
    let mut args = env::args();
    args.next();
    let argument = match args.next() {
        Some(v) => v,
        None => invalid_usage(),
    };

    env::set_var("RUST_BACKTRACE", "1");
    prepare();

    match argument.as_str() {
        "build" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            parse_environments(args);
            
            compile(&file)?;
        }
        "run" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            parse_environments(args);

            let file_path = Path::new(&file);
            if file_path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("az"))
            {
                compile(&file)?;
                let file = format!("{file}urite");

                println!("{} {file}", "Running..".bright_green().bold());
                let _ = azurite_runtime::run_file(&file);
            } else {
                println!("{} {file}", "Running..".bright_green().bold());
                let _ = azurite_runtime::run_file(&file);
            }
        }
        "run-dir" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };

            parse_environments(args);

            let file_path = Path::new(&file);
            let directory = fs::read_dir(file_path).unwrap();
            for buffer in directory {
                let path = buffer.unwrap().path();
                let file = path.to_str().unwrap();

                if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("az"))
                {
                    compile(file)?;
                    let file = format!("{file}urite");

                    println!("{} {file}", "Running..".bright_green().bold());
                    let _ = azurite_runtime::run_file(&file);
                }
            }
        }
        "disassemble" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };

            parse_environments(args);

            compile(&file)?;
            

            let file = format!("{file}urite");
            println!("{} {file}", "Disassembling..".bright_green().bold());

            let file_data = std::fs::read(&file).unwrap();

            let packed = Packed::from_bytes(&file_data).unwrap();
            let mut data : Vec<_> = packed.into();
            
            println!("{}", disassemble(std::mem::take(&mut data[0].0).into_iter()));
        },
        _ => invalid_usage(),
    }
    Ok(())

    // Some(())
}

fn parse_environments(arguments: Args) {
    for i in arguments {
        match i.as_str() {
            "--release" => env::set_var(environment::RELEASE_MODE, "1"),
            "--" => (),
            _ => {
                println!("unexpected argument {i}");
                std::process::exit(0)
            }
        }
    }
}

fn invalid_usage() -> ! {
    println!("{}: please provide a sub-command (build, run, disassemble, constants, repl) followed by a file name", "invalid usage".red().bold());
    std::process::exit(1)
}

fn compile(file: &str) -> Result<(), ExitCode> {
    println!("{} {file}", "Compiling..".bright_green().bold());
    let instant = Instant::now();
    azurite_compiler::run_file(file)?;
    println!("{}", format!("Finished in {} seconds!", instant.elapsed().as_secs_f64()).bright_green().bold());
    Ok(())
}

#[allow(clippy::format_push_string)]
#[allow(clippy::too_many_lines)]
fn disassemble(mut v: IntoIter<u8>) -> String {
    let mut depth = 0;
    let mut disassemble = String::new();
    let max = v.len();
    loop {
        let i = v.next().expect(&disassemble);
        
        disassemble.push_str(&format!("\n{:<max$}: ", max-v.len(), max=max.to_string().len()));
        disassemble.push_str("    ".repeat(depth).as_str());
        match i {
            consts::Return => {
                disassemble.push_str("return");
                if depth == 0 {
                    if v.next().is_none() {
                        return disassemble;
                    }
                    disassemble.push_str(" - ERROR!".red().bold().to_string().as_str());
                    depth += 1;
                }
                depth -= 1;
            }
            consts::ReturnFromFunction => {
                disassemble.push_str("return from function");
            }
            consts::ReturnWithoutCallStack => {
                disassemble.push_str(&format!("return without callstack {}", v.next().unwrap()));
            }
            consts::LoadConst => {
                disassemble.push_str(format!("load const {}", v.next().unwrap()).as_str());
            }
            consts::LoadConstStr => {
                disassemble.push_str(format!("load const str {}", v.next().unwrap()).as_str());
            }
            consts::Add => disassemble.push_str("add"),
            consts::Subtract => disassemble.push_str("subtract"),
            consts::Multiply => disassemble.push_str("multiply"),
            consts::Divide => disassemble.push_str("divide"),
            consts::EqualsTo => disassemble.push_str("equals to"),
            consts::NotEqualsTo => disassemble.push_str("not equals to"),
            consts::GreaterThan => disassemble.push_str("greater than"),
            consts::LesserThan => disassemble.push_str("lesser than"),
            consts::GreaterEquals => disassemble.push_str("greater equals"),
            consts::LesserEquals => disassemble.push_str("lesser equals"),
            consts::GetVar => disassemble.push_str(&format!(
                "get var {}",
                u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()])
            )),
            consts::GetVarFast => {
                disassemble.push_str(&format!("get var fast {}", v.next().unwrap()));
            }
            consts::ReplaceVar => disassemble.push_str(&format!(
                "replace var {}",
                u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()])
            )),
            consts::ReplaceVarFast => {
                disassemble.push_str(&format!("replace var fast {}", v.next().unwrap()));
            }
            consts::ReplaceVarInObject => {
                let size = v.next().unwrap();

                disassemble.push_str(&format!(
                    "replace var in object {size} - {}",
                    &(0..size)
                        .map(|_| format!("{} ", v.next().unwrap()))
                        .collect::<String>()
                ));
            }
            consts::Not => disassemble.push_str("not"),
            consts::Negative => disassemble.push_str("negate"),
            consts::Pop => disassemble.push_str("pop"),
            consts::PopMulti => disassemble.push_str(&format!("pop multi {}", v.next().unwrap())),
            consts::Jump => disassemble.push_str(&format!("jump {}", v.next().unwrap())),
            consts::JumpIfFalse => disassemble.push_str(&format!("jump if false {}", v.next().unwrap())),
            consts::JumpBack => disassemble.push_str(&format!("jump back {}", v.next().unwrap())),
            consts::JumpLarge => disassemble.push_str(&format!("jump large {}", u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()]))),
            consts::JumpIfFalseLarge => disassemble.push_str(&format!("jump if false large {}", u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()]))),
            consts::JumpBackLarge => disassemble.push_str(&format!("jump back large {}", u16::from_le_bytes([v.next().unwrap(), v.next().unwrap()]))),
            consts::LoadFunction => {
                disassemble.push_str(&format!(
                    "load function (arg count: {}) (has return {}) {}",
                    v.next().unwrap(),
                    v.next().unwrap(),
                    v.next().unwrap()
                ));
                depth += 1;
            }
            consts::CallFunction => disassemble.push_str(&format!("call function {}", v.next().unwrap())),
            consts::CreateStruct => {
                let size = v.next().unwrap();

                disassemble.push_str(&format!("create struct {size}"));
            }
            consts::AccessData => disassemble.push_str(&format!("access data {}", v.next().unwrap())),
            consts::RawCall => disassemble.push_str(&format!("raw call {}", v.next().unwrap())),
            consts::Rotate => disassemble.push_str("rotate"),
            consts::Over => disassemble.push_str("over"),
            consts::Swap => disassemble.push_str("swap"),
            consts::IndexSwap => {
                disassemble.push_str(&format!(
                    "index swap {} {}",
                    v.next().unwrap(),
                    v.next().unwrap(),
                ));
            },
            consts::Duplicate => disassemble.push_str("duplicate"),
            consts::Increment => disassemble.push_str("increment"),
            _ => disassemble.push_str(&format!("UNKNOWN INDEX{{{i}}}"))
        };
    }
}
