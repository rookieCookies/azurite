#![recursion_limit = "1000000000000000000" ]
use std::env::Args;
use std::fs;
use std::io::Write;
use std::time::Instant;
use std::{env, path::Path, process::ExitCode};

use azurite_archiver::Packed;
use azurite_common::{environment, prepare, Bytecode};
use azurite_compiler::Data;
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
        "repl" => {
            // let mut input = String::new();

            // let mut symbol_table = SymbolTable::new();
            // let mut conversion_state = ConversionState::new(&mut symbol_table);
            // let mut analysis_state = AnalysisState::new();
            // let mut vm = VM {
            //     constants: vec![],
            //     stack: Stack::new(),
            // };

            // if let Some(v) = args.next() {
            //     input = fs::read_to_string(v).unwrap()
            // }
            
            // loop {
            //     let tokens = match azurite_compiler::lex(&input) {
            //         Ok(tokens) => tokens,
            //         Err(errs) => {
            //             println!("{}", errs.build(("repl", &input)));
            //             input.clear();
            //             continue
            //         }
            //     };

            //     let mut instructions = match azurite_compiler::parse(tokens.into_iter()) {
            //         Ok(instructions) => instructions,
            //         Err(errs) => {
            //             println!("{}", errs.build(("repl", &input)));
            //             input.clear();
            //             continue
            //         },
            //     };
                
            //     if let Err(e) = analysis_state.start_analysis(&mut instructions) {
            //         println!("{}", e.build(("repl", &input)));
            //         input.clear();
            //         continue
            //     }

            //     let return_reg = conversion_state.generate(instructions);

            //     let mut codegen = CodeGen::new();
            //     codegen.codegen(std::mem::take(&mut conversion_state.blocks));

            //     vm.constants = conversion_state.constants
            //         .iter()
            //         .map(|x| match x {
            //             Data::Int(v) => VMData::Integer(*v),
            //             Data::Float(v) => VMData::Float(*v),
            //             Data::String(_) => todo!(),
            //             Data::Bool(b) => VMData::Bool(*b),
            //             Data::Empty => todo!(),
            //         })
            //         .collect();

                
            //     vm.run(Code::new(&codegen.bytecode));

            //     println!("{:?}", vm.stack.reg(return_reg.0 as u8));
                

            //     input.clear();
                
            //     print!(" > ");
                
            //     if std::io::Write::flush(&mut std::io::stdout()).is_err() {
            //         println!("failed to flush stdout");
            //         continue;
            //     }

            //     if std::io::stdin().read_line(&mut input).is_err() {
            //         println!("failed to read stdin");
            //         continue;
            //     }

            // }
        }
        "build" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            parse_environments(args);

            let data = compile(&file)?;

            fs::write(format!("{file}urite"), data.as_bytes()).unwrap();
        }
        "run" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };
            parse_environments(args);

            let compiled = if file.ends_with(".azurite") {
                let file_data = fs::read(&file).unwrap();
                Packed::from_bytes(&file_data).unwrap()
            } else { compile(&file)? };

            println!("{} {file}", "Running..".bright_green().bold());
            azurite_runtime::run_file(compiled);
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
                    // let _ = azurite_runtime::run_file(&file);
                }
            }
        }
        "disassemble" => {
            let file = match args.next() {
                Some(v) => v,
                None => invalid_usage(),
            };

            parse_environments(args);

            let packed = compile(&file)?;

            println!("{} {file}", "Disassembling..".bright_green().bold());

            let mut data: Vec<_> = packed.into();

            disassemble(std::mem::take(&mut data[0].0))
        }
        _ => invalid_usage(),
    }
    Ok(())

    // Some(())
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

    let raw_data = fs::read(file).expect("cant open fine");
    let file_data = String::from_utf8_lossy(&raw_data);

    
    let (bytecode, constants, symbol_table) = match azurite_compiler::compile(&file_data) {
        Ok(v) => v,
        Err(e) => {
            print!("{}", e.build((file, &file_data)));
            return Err(ExitCode::FAILURE)
        }
    };


    let mut constants_bytes = vec![];

    for constant in constants {
        match constant {
            Data::Int(v) => {
                constants_bytes.push(0);
                constants_bytes.append(&mut v.to_le_bytes().into());
            },
            
            Data::Float(v) => {
                constants_bytes.push(1);
                constants_bytes.append(&mut v.to_le_bytes().into());
            },
            
            Data::Bool(v) => {
                constants_bytes.push(2);
                constants_bytes.push(v as u8);
            },
            
            Data::String(v) => {
                constants_bytes.push(3);
                constants_bytes.append(&mut symbol_table.get(v).as_bytes().to_vec());
                constants_bytes.push(0);
            },
            
            Data::Empty => panic!("empty data type shouldn't be constants"),
        }
    }

    
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
                let _ = write!(lock, "{} (", arg_count);
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
            Bytecode::LesserEquals => writeln!(lock, "lt {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::LoadConst => writeln!(lock, "load {} {}", d.next(), d.next()),
            Bytecode::Jump => writeln!(lock, "jmp {}", d.u32()),
            Bytecode::JumpCond => writeln!(lock, "cond-jump {} {} {}", d.next(), d.u32(), d.u32()),
            Bytecode::Struct => writeln!(lock, "struct {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::Unit => writeln!(lock, "unit {}", d.next()),
            Bytecode::AccStruct => writeln!(lock, "accstruct {} {} {}", d.next(), d.next(), d.next()),
            Bytecode::SetField => writeln!(lock, "setfield {} {} {}", d.next(), d.next(), d.next()),
        
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
}