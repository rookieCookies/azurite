
use colored::Colorize;

#[derive(Debug)]
pub struct RuntimeError {
    pub bytecode_index: u64,
    message: String,
}

impl RuntimeError {
    #[must_use]
    #[cfg(not(tarpaulin_include))]
    pub fn new(index: u64, message: &'static str) -> Self {
        Self::new_string(index, message.to_string())
    }

    #[must_use]
    #[cfg(not(tarpaulin_include))]
    pub fn new_string(index: u64, message: String) -> Self {
        Self {
            bytecode_index: index,
            message,
        }
    }

    /// # Errors
    /// This function will return an error if the
    /// linetable is unable to be read
    #[cfg(not(tarpaulin_include))]
    pub fn trigger(self, linetable_bytes: Vec<u8>) {
        let linetable = load_linetable(linetable_bytes);
        if linetable.is_empty() {
            println!("{}: {}", "error".bold().red(), self.message);
        } else {
            let err_line = linetable.get(self.bytecode_index as usize - 2).unwrap_or(&0);
            println!("{} | {}: {}", err_line+1, "error".bold().red(), self.message);
        }
    }
}

pub(crate) fn load_linetable(linetable_bytes: Vec<u8>) -> Vec<u32> {
    let mut linetable = Vec::with_capacity(linetable_bytes.len() / 4);
    let mut iter = linetable_bytes.into_iter();
    while let Some(x) = iter.next() {
        let count = u32::from_le_bytes([
            x,
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
        ]);
        let line = u32::from_le_bytes([
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
        ]);

        for _ in 0..count {
            linetable.push(line);
        }
    }

    linetable
}
