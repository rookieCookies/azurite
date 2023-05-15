use std::env;

use azurite_common::environment;

use azurite_errors::Error;

pub use azurite_lexer::lex;
pub use azurite_parser::parse;
pub use azurite_ast_to_ir::ConversionState;
pub use azurite_semantic_analysis::AnalysisState;
pub use azurite_codegen::CodeGen;
use azurite_semantic_analysis::GlobalState;
pub use common::Data;
pub use common::SymbolTable;

pub fn compile(data: &str) -> Result<(Vec<u8>, Vec<Data>, SymbolTable), Error> {
    let mut symbol_table = SymbolTable::new();
    
    let tokens = lex(data, &mut symbol_table)?;

    let mut instructions = parse(tokens.into_iter(), &mut symbol_table)?;

    let symbol_table = symbol_table;
    
    let mut global_state = GlobalState::new(&symbol_table);
    
    AnalysisState::new().start_analysis(&mut global_state, &mut instructions)?;

    let mut ir = ConversionState::new(&symbol_table);

    ir.generate(instructions);

    ir.sort();
    if env::var(environment::RELEASE_MODE).unwrap_or("0".to_string()) == *"1" {
        ir.optimize();
    }

    ir.sort();

    if env::var(environment::DUMP_IR).unwrap_or("0".to_string()) == *"1" {
        let dump = ir.pretty_print();
        if let Ok(v) = env::var(environment::DUMP_IR_FILE) {
            std::fs::write(v, dump.as_bytes()).unwrap()
        } else {
            println!("{dump}");
        }
    }
    

    let mut codegen = CodeGen::new();
    codegen.codegen(&symbol_table, ir.externs, ir.functions);

    Ok((codegen.bytecode, ir.constants, symbol_table))
}