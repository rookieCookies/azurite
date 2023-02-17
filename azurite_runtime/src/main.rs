use std::{env, io::Read, process::ExitCode};

use azurite_common::{parse_args, prepare};
use azurite_runtime::{load_constants, vm::VM};

fn main() -> ExitCode {
    prepare();
    let (file, environemnt_parameters) =
        match parse_args(env::args().collect::<Vec<_>>().into_iter()) {
            Ok(v) => v,
            Err(e) => {
                println!("{e}");
                return ExitCode::FAILURE;
            }
        };

    for parameter in environemnt_parameters {
        env::set_var(parameter.identifier, parameter.value)
    }

    let _ = run_file(file);
    ExitCode::SUCCESS
}

fn run_file(path: String) -> Result<(), ExitCode> {
    let zipfile = std::fs::File::open(&path).unwrap();

    let mut archive = zip::ZipArchive::new(zipfile).unwrap();

    let mut bytecode_file = match archive.by_name("bytecode.azc") {
        Ok(file) => file,
        Err(..) => {
            println!("bytecode.azc not found");
            return Err(ExitCode::FAILURE);
        }
    };

    let mut bytecode = vec![];
    match bytecode_file.read_to_end(&mut bytecode) {
        Ok(_) => {}
        Err(_) => return Err(ExitCode::FAILURE),
    };

    drop(bytecode_file);

    let mut constants_file = match archive.by_name("constants.azc") {
        Ok(file) => file,
        Err(..) => {
            println!("constants.azc not found");
            return Err(ExitCode::FAILURE);
        }
    };

    let mut constants = vec![];
    match constants_file.read_to_end(&mut constants) {
        Ok(_) => {}
        Err(_) => return Err(ExitCode::FAILURE),
    };

    drop(constants_file);

    let mut vm = match VM::new() {
        Ok(v) => v,
        Err(err) => return err.trigger(path),
    };

    vm.constants = match load_constants(constants, &mut vm) {
        Ok(v) => v,
        Err(err) => {
            err.trigger(path)?;
            return Err(ExitCode::FAILURE);
        }
    };
    // let instant = Instant::now();

    let runtime = vm.run(&bytecode);

    // let end = instant.elapsed().as_millis();
    // println!("\n\nit took {}ms", end);

    #[cfg(feature = "hotspot")]
    {
        use azurite_common::Bytecode;
        use std::cmp::Ordering;
        let mut x = vm
            .hotspots
            .into_iter()
            .map(|x| (x.1 .0, x.0, x.1 .1))
            .collect::<Vec<(usize, Bytecode, f64)>>();
        x.sort_by(|x, y| {
            if x.0 < y.0 {
                Ordering::Greater
            } else if x.0 > y.0 {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        println!("---------------------------------------------");
        x.into_iter().for_each(|(x, y, time)| {
            println!("| {:>15} -> {x:>10} - {time:>9.3} |", format!("{:?}", y))
        });
        println!("---------------------------------------------");
    }
    if let Err(runtime) = runtime {
        println!("runtime err");
        runtime.trigger(path)?;
        return Err(ExitCode::FAILURE);
    }

    Ok(())
}
