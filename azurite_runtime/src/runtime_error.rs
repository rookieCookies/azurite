use std::{io::Read, process::ExitCode};

use colored::Colorize;

#[derive(Debug)]
pub struct RuntimeError {
    pub bytecode_index: u64,
    message: String,
}

impl RuntimeError {
    #[must_use]
    pub fn new(index: u64, message: &'static str) -> Self {
        Self::new_string(index, message.to_string())
    }

    #[must_use]
    pub fn new_string(index: u64, message: String) -> Self {
        Self {
            bytecode_index: index,
            message,
        }
    }

    /// # Errors
    /// This function will return an error if the
    /// linetable is unable to be read
    pub fn trigger(self, path: &str) -> Result<(), ExitCode> {
        let linetable = load_linetable(path)?;
        let err_line = linetable[self.bytecode_index as usize];
        println!("{} | {}: {}", err_line, "error".bold().red(), self.message);
        Ok(())
    }
}

fn load_linetable(path: &str) -> Result<Vec<u32>, ExitCode> {
    let zipfile = std::fs::File::open(path).unwrap();

    let mut archive = zip::ZipArchive::new(zipfile).unwrap();

    let mut linetable_file = if let Ok(file) = archive.by_name("linetable.azc") {
        file
    } else {
        println!("linetable.azc not found");
        return Err(ExitCode::FAILURE);
    };

    let mut linetable_bytes = vec![];
    match linetable_file.read_to_end(&mut linetable_bytes) {
        Ok(_) => {}
        Err(_) => return Err(ExitCode::FAILURE),
    };

    drop(linetable_file);

    let mut linetable = Vec::with_capacity(linetable_bytes.len() / 4 / 2);
    let mut iter = linetable_bytes.into_iter();
    while let Some(x) = iter.next() {
        let line = u32::from_le_bytes([
            x,
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
        ]);
        // let amount = u32::from_le_bytes([
        //     iter.next().unwrap(),
        //     iter.next().unwrap(),
        //     iter.next().unwrap(),
        //     iter.next().unwrap(),
        // ]);

        linetable.push(line);
    }

    Ok(linetable)
}
