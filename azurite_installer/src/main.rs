use std::{process::ExitCode, io::Write, path::{PathBuf, Path}};

use directories::ProjectDirs;
use include_dir::{include_dir, Dir};

const AZURITE_CLI_BINARY : &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/release/azurite_cli.exe"));

static AZURITE_LIBRARIES_FOLDER: Dir = include_dir!("$CARGO_MANIFEST_DIR/../builtin_libraries/azurite_libraries/");
static AZURITE_LIBRAR_API_FOLDER: Dir = include_dir!("$CARGO_MANIFEST_DIR/../builtin_libraries/azurite_api_files/");

fn main() -> ExitCode {
    println!("-----------------------------------");
    println!("|                                 |");
    println!("|        AZURITE INSTALLER        |");
    println!("|                                 |");
    println!("-----------------------------------");

    let mut input_string = String::new();

    print!("installation directory (continue for default): ");
    std::io::stdout().flush().unwrap();
    if std::io::stdin().read_line(&mut input_string).is_err() {
        eprintln!("failed to read stdin");
        return ExitCode::FAILURE;
    }

    input_string = input_string.trim().to_string();

    
    let dir = if input_string.is_empty() {
        match ProjectDirs::from("", "", "azurite") {
            Some(v) => v,
            None => {
                eprintln!("unable to access the installation directory");
                return ExitCode::FAILURE;
            },
        }.data_dir().to_path_buf()
    } else {
        PathBuf::from(&input_string)
    };
    

    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("failed to create a directory at {:?}", dir.to_string_lossy());
        return ExitCode::FAILURE;
    }


    {
        println!("- unwrapping the command line tool");

        let cli_path = dir.join("azurite.exe");
        if std::fs::write(cli_path, AZURITE_CLI_BINARY).is_err() {
            eprintln!("failed to unwrap the command line tool");
            return ExitCode::FAILURE;
        }


        println!("successfully unwrapped the command line tool");
    }
    

    {
        println!("- creating the standard library");
        
        {
            let library_dir = dir.join("runtime");
            if std::fs::create_dir_all(&library_dir).is_err() {
                eprintln!("failed to create directory at {}", library_dir.to_string_lossy());
                return ExitCode::FAILURE;
            }

            for file in AZURITE_LIBRARIES_FOLDER.files() {
                let file_name = file.path().file_name().unwrap();
                let contents = file.contents();

                let mut path = library_dir.clone();
                path.push(file_name);

                if std::fs::write(&path, contents).is_err() {
                    eprintln!("failed to write {}", path.as_path().to_string_lossy());
                    return ExitCode::FAILURE;
                }
            
            }
        }

        
        {
            let library_dir = dir.join("api");
            if std::fs::create_dir_all(&library_dir).is_err() {
                eprintln!("failed to create directory at {}", library_dir.to_string_lossy());
                return ExitCode::FAILURE;
            }

            for file in AZURITE_LIBRAR_API_FOLDER.files() {
                // let file_name = file.path().file_name().unwrap();
                let contents = file.contents();

                let path = library_dir.join(file.path());
                // path.push(file_name);

                if std::fs::write(&path, contents).is_err() {
                    eprintln!("failed to write {}", path.as_path().to_string_lossy());
                    return ExitCode::FAILURE;
                }
            
            }
            
        }

        println!("successfully created the standard library");
    }


    print!("should the command line tool be appended to the PATH variable (y/n): ");
    let add_to_path;

    loop {
        input_string.clear();
        std::io::stdout().flush().unwrap();
        if std::io::stdin().read_line(&mut input_string).is_err() {
            eprintln!("failed to read stdin");
            return ExitCode::FAILURE;
        }

        if input_string.as_str().trim() == "y" {
            add_to_path = true;
            break
        } else if input_string.as_str().trim() == "n" {
            add_to_path = false;
            break
        } else {
            println!("{input_string} is not a valid input");
        }
    }


    if add_to_path && set_env::append("PATH", dir.to_str().unwrap()).is_err() {
        eprintln!("unable to append to the PATH variable");
        return ExitCode::FAILURE;
    }

    println!("thanks for installing azurite!");
    
    ExitCode::SUCCESS
}

