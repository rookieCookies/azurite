use colored::Colorize;

#[derive(Debug)]
pub struct RuntimeError {
    pub bytecode_index: u64,
    pub message: String,
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
    pub fn trigger(self, linetable_bytes: Vec<u8>, function_table_bytes: Vec<u8>, callstack: Vec<(usize, usize)>) {
        use colored::Color;

        let linetable = load_linetable(linetable_bytes);
        let function_table = load_function_table(function_table_bytes);


        if linetable.is_empty() {
            println!("{}: {}", "error".bold().red(), self.message);
        } else {
            let padding_size = callstack.len().to_string().len();

            let err_line = linetable.get(self.bytecode_index as usize).unwrap_or(&0);
            println!("{:<width$} | {}: {}", err_line+1, "error".bold().red(), self.message, width=padding_size);

            println!("\nstack trace:");

            for (enumaration, index) in callstack.into_iter().enumerate() {
                println!("    {:<width$} | {}", enumaration, function_table[index.0].to_string().color(Color::TrueColor { r: 130, g: 130, b: 130 }).bold(), width=padding_size);
            }
        }
    }
}

pub(crate) fn load_linetable(linetable_bytes: Vec<u8>) -> Vec<u32> {
    let mut linetable = Vec::with_capacity(linetable_bytes.len() / 4);
    let mut iter = linetable_bytes.into_iter();
    while let Some(x) = iter.next() {
        let line = u32::from_le_bytes([
            x,
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
        ]);

        linetable.push(line);
    }

    linetable
}

pub(crate) fn load_function_table(bytes: Vec<u8>) -> Vec<String> {
    let mut function_table = vec!["::root".to_string()];

    let mut iter = bytes.into_iter();
    while let Some(x) = iter.next() {
        let count = x;

        let mut string_bytes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            string_bytes.push(iter.next().unwrap())
        }

        function_table.push(String::from_utf8(string_bytes).unwrap())
    }
    function_table
}
