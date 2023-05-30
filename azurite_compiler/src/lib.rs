use std::collections::HashMap;
use std::env;

use azurite_common::environment;

use azurite_errors::Error;

pub use azurite_lexer::lex;
use azurite_parser::ast::Instruction;
pub use azurite_parser::parse;
pub use azurite_ast_to_ir::ConversionState;
pub use azurite_semantic_analysis::AnalysisState;
pub use azurite_codegen::CodeGen;
use azurite_semantic_analysis::GlobalState;
pub use common::Data;
use common::SymbolIndex;
pub use common::SymbolTable;

pub fn compile(file_name: String, data: String) -> (Result<(Vec<u8>, Vec<Data>, SymbolTable), Error>, HashMap<SymbolIndex, (String, String)>) {
    let mut symbol_table = SymbolTable::new();
    let file_name = symbol_table.add(file_name);
    
    let tokens = match lex(&data, file_name, &mut symbol_table) {
        Ok(v) => v,
        Err(e) => return (Err(e), HashMap::from([(file_name, (symbol_table.get(file_name), data.to_string()))])),
    };

    let mut instructions = match parse(tokens.into_iter(), file_name, &mut symbol_table) {
        Ok(v) => v,
        Err(e) => return (Err(e), HashMap::from([(file_name, (symbol_table.get(file_name), data.to_string()))])),
    };
    
    
    let mut global_state = GlobalState::new(&mut symbol_table);
    
    let mut analysis = AnalysisState::new(file_name);
    match analysis.start_analysis(&mut global_state, &mut instructions) {
        Ok(v) => v,
        Err(e) => {
            let mut temp : HashMap<SymbolIndex, (String, String)> = global_state.files.into_iter().map(|x| (x.0, (symbol_table.get(x.0), x.1.2))).collect();
            temp.insert(file_name, (symbol_table.get(file_name), data));
            return (Err(e), temp)
        },
    };

    global_state.files.insert(file_name, (analysis, instructions, data));


    let (files, files_data) : (Vec<(SymbolIndex, Vec<Instruction>)>, HashMap<SymbolIndex, (String, String)>) = 
        global_state.files.
            into_iter().
            map(|x| 
                (
                    (x.0, x.1.1),
                    (x.0, (symbol_table.get(x.0), x.1.2))
                )
            ).unzip();
    

    let mut ir = ConversionState::new(symbol_table, file_name);

    ir.generate(file_name, files);

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
    
    let externs = ir.externs;
    let functions = ir.functions;
    let constants = ir.constants;
    let mut codegen = CodeGen::new();
    codegen.codegen(&ir.symbol_table, externs, functions);

    (Ok((codegen.bytecode, constants, ir.symbol_table)), files_data)
}