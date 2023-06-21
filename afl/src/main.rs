#[cfg(features = "afl")]
use azurite_archiver::Packed;

#[cfg(features = "afl")]
#[macro_use]
extern crate afl;

#[cfg(features = "afl")]
fn main() {
    fuzz!(|data: &[u8]| {
        if let Ok(s) = std::str::from_utf8(data) {
            let (val, _) = azurite_compiler::compile(String::new(), s.replace('\t', "    "));
            if let Ok((metadata, bytecode, constants, symbol_table)) = val {
                let constants_bytes = azurite_compiler::convert_constants_to_bytes(constants, &symbol_table);
                let packed = Packed::new()
                    .with(azurite_archiver::Data(Vec::from(metadata.to_bytes())))
                    .with(azurite_archiver::Data(bytecode))
                    .with(azurite_archiver::Data(constants_bytes));

                azurite_runtime::run_packed(packed).unwrap();
            }
        }
    });
}

#[cfg(not(features = "afl"))]
fn main() {}
