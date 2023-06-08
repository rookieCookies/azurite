// #![recursion_limit = "1000000000000000000"]
#![deny(clippy::pedantic)]
use std::env::Args;
use std::fs;
use std::io::Write;
use std::time::Instant;
use std::{env, path::Path, process::ExitCode};

use azurite_archiver::Packed;
use azurite_common::{environment, prepare, Bytecode};
use colored::Colorize;

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), ExitCode> {
    let mut args = env::args();
    args.next();
    let Some(argument) = args.next() else { invalid_usage() };

    env::set_var("RUST_BACKTRACE", "1");
    prepare();

    match argument.as_str() {
        "build" => {
            let Some(file) = args.next() else { invalid_usage() };
            parse_environments(args);

            let data = compile(&file)?;

            fs::write(format!("{file}urite"), data.as_bytes()).unwrap();
        }

        
        "run" => {
            let Some(file) = args.next() else { invalid_usage() };
            parse_environments(args);

            let Some(compiled) = (if file.ends_with(".azurite") {
                let Ok(file_data) = fs::read(&file) else { eprintln!("can't read file {file}"); return Err(ExitCode::FAILURE) };
                Packed::from_bytes(&file_data)
            } else { Some(compile(&file)?) }) else { eprintln!("not a valid azurite file"); return Err(ExitCode::FAILURE)};

            println!("{} {file}", "Running..".bright_green().bold());
            azurite_runtime::run_packed(compiled);
        }

        
        "run-dir" => {
            let Some(file) = args.next() else { invalid_usage() };
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
                }
            }
        }

        
        "disassemble" => {
            let Some(file) = args.next() else { invalid_usage() };
            parse_environments(args);

            let packed = compile(&file)?;

            println!("{} {file}", "Disassembling..".bright_green().bold());

            let mut data: Vec<_> = packed.into();

            disassemble(std::mem::take(&mut data[0].0));
        }
        _ => invalid_usage(),
    }

    Ok(())
}


fn parse_environments(mut arguments: Args) {
    while let Some(i) = arguments.next() {
        match i.as_str() {
            "--release"    => env::set_var(environment::RELEASE_MODE, "1"),
            "--dump-ir"    => env::set_var(environment::DUMP_IR, "1"),
            "--dump-ir-to" => env::set_var(environment::DUMP_IR_FILE, match arguments.next() {
                Some(v) => v.to_string(),
                None => break,
            }),
            "--no-std"     => env::set_var(environment::NO_STD, "1"),
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

fn compile(file: &str) -> Result<Packed, ExitCode> {
    println!("{} {file}", "Compiling..".bright_green().bold());
    let instant = Instant::now();

    let Ok(raw_data) = fs::read(file) else { eprintln!("'{file}' doesn't exist"); return Err(ExitCode::FAILURE)};
    let file_data = String::from_utf8_lossy(&raw_data).replace('\t', "    ").replace('\r', "");


    let (result, debug_info) = azurite_compiler::compile(file.to_string(), file_data);
    
    let (bytecode, constants, symbol_table) = match result {
        Ok(v) => v,
        Err(e) => {
            print!("{}", e.build(&debug_info));
            return Err(ExitCode::FAILURE)
        }
    };

    let constants_bytes = azurite_compiler::convert_constants_to_bytes(constants, &symbol_table);

    
    println!(
        "{}",
        format!("Finished in {} seconds!", instant.elapsed().as_secs_f64())
            .bright_green()
            .bold()
    );

    Ok(Packed::new()
        .with(azurite_archiver::Data(bytecode))
        .with(azurite_archiver::Data(constants_bytes))
    )
}

#[allow(clippy::format_push_string)]
#[allow(clippy::too_many_lines)]
fn disassemble(v: Vec<u8>) {
    let mut d = Disassembler {
        code: v,
        top: 0,
    };
    
    let mut lock = std::io::stdout().lock();

    while d.code.len() > d.top {
        let _ = write!(lock, "{} | ", d.top);
        let _ = match d.bytecode() {
            Bytecode::Return => writeln!(lock, "ret"),
            Bytecode::Copy => writeln!(lock, "copy {} {}", d.next(), d.next()),
            Bytecode::Swap => writeln!(lock, "swap {} {}", d.next(), d.next()),
            Bytecode::Call => {
                let _ = write!(lock, "call {} {} ", d.u32(), d.next());
                let arg_count = d.next();
                let _ = write!(lock, "{arg_count} (");
                (0..arg_count).for_each(|_| { let _ = write!(lock, " {}", d.next()); });
                writeln!(lock, " )")
            },
            Bytecode::ExtCall => {
                let _ = write!(lock, "ecall {} {} ", d.u32(), d.next());
                let arg_count = d.next();
                let _ = write!(lock, "{arg_count} (");
                (0..arg_count).for_each(|_| { let _ = write!(lock, " {}", d.next()); });
                writeln!(lock, " )")
            },
            Bytecode::Struct => {
                let _ = write!(lock, "struct {}", d.next());
                let arg_count = d.next();
                let _ = write!(lock, "{arg_count} (");
                (0..arg_count).for_each(|_| { let _ = write!(lock, " {}", d.next()); });
                writeln!(lock, " )")
            },
            Bytecode::Push => writeln!(lock, "push {}", d.next()),
            Bytecode::Pop => writeln!(lock, "pop {}", d.next()),
            Bytecode::Add => writeln!(lock, "add {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::Subtract => writeln!(lock, "sub {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::Multiply => writeln!(lock, "mul {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::Divide => writeln!(lock, "div {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::Equals => writeln!(lock, "eq {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::NotEquals => writeln!(lock, "neq {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::GreaterThan => writeln!(lock, "gt {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::LesserThan => writeln!(lock, "lt {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::GreaterEquals => writeln!(lock, "ge {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::LesserEquals => writeln!(lock, "le {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::LoadConst => writeln!(lock, "load {} {}", d.next(), d.next()),
            Bytecode::Jump => writeln!(lock, "jmp {}", d.u32()),
            Bytecode::JumpCond => writeln!(lock, "cond-jump {} {} {}", d.next(), d.u32(), d.u32()),
            Bytecode::Unit => writeln!(lock, "unit {}", d.next()),
            Bytecode::AccStruct => writeln!(lock, "accstruct {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::SetField => writeln!(lock, "setfield {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::ExternFile => {
                let _ = write!(lock, "extern \"{}\" ( ", d.string());

                let amount = d.next();
                for _ in 0..amount {
                    let _ = write!(lock, "\"{}\" ", d.string());
                }

                writeln!(lock, ")")
            },
            Bytecode::UnaryNot => writeln!(lock, "not {} {}", d.next(), d.next()),
            Bytecode::UnaryNeg => writeln!(lock, "neg {} {}", d.next(), d.next()),
        
        };
    }
}

struct Disassembler {
    code: Vec<u8>,
    top: usize,
}

impl Disassembler {
    fn bytecode(&mut self) -> Bytecode {
        Bytecode::from_u8(self.next()).unwrap()
    }

    fn u32(&mut self) -> u32 {
        let a0 = self.next();
        let a1 = self.next();
        let a2 = self.next();
        let a3 = self.next();

        u32::from_le_bytes([a0, a1, a2, a3])
    }

    fn next(&mut self) -> u8 {
        self.top += 1;

        self.code[self.top-1]
    }

    fn string(&mut self) -> String {
        let mut bytes = vec![];

        loop {
            let val = self.next();
            if val == 0 {
                break
            }

            bytes.push(val);
        }

        String::from_utf8(bytes).unwrap()
    }
}