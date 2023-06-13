use std::mem::{transmute, size_of};

use azurite_archiver::Packed;
use azurite_common::CompilationMetadata;

#[macro_use]
extern crate afl;

fn main() {
    fuzz!(|data: &[u8]| {
        if let Ok(s) = std::str::from_utf8(data) {
            let (val, _) = azurite_compiler::compile(String::new(), s.replace('\t', "    "));
            if let Ok((metadata, bytecode, constants, symbol_table)) = val {
                let constants_bytes = azurite_compiler::convert_constants_to_bytes(constants, &symbol_table);
                let packed = Packed::new()
                    .with(azurite_archiver::Data(Vec::from(unsafe { transmute::<_, [u8; size_of::<CompilationMetadata>()]>(metadata) } )))
                    .with(azurite_archiver::Data(bytecode))
                    .with(azurite_archiver::Data(constants_bytes));

                azurite_runtime::run_packed(packed);
            }
        }
    });
}
