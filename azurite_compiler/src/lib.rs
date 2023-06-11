use std::collections::{HashMap, HashSet, BTreeSet, BTreeMap};
use std::env;

use azurite_common::{environment, CompilationMetadata};

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

type DebugHashmap = HashMap<SymbolIndex, (String, String)>;
type ReturnValue = Result<(CompilationMetadata, Vec<u8>, Vec<Data>, SymbolTable), Error>;

pub fn compile(file_name: String, data: String) -> (ReturnValue, DebugHashmap) {
    let mut symbol_table = SymbolTable::new();
    let file_name = symbol_table.add(file_name[..file_name.len()-3].to_string());
    
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
            let mut temp : DebugHashmap = global_state.files.into_iter().map(|x| (x.0, (symbol_table.get(x.0), x.1.2))).collect();
            temp.insert(file_name, (symbol_table.get(file_name), data));
            return (Err(e), temp)
        },
    };

    global_state.files.insert(file_name, (analysis, instructions, data));


    let (files, files_data) : (Vec<(SymbolIndex, Vec<Instruction>)>, DebugHashmap) = 
        global_state.files.
            into_iter().
            map(|x| 
                (
                    (x.0, x.1.1),
                    (x.0, (symbol_table.get(x.0), x.1.2))
                )
            ).unzip();
    

    let mut ir = ConversionState::new(symbol_table);

    ir.generate(file_name, files);

    ir.sort();

    #[cfg(not(features = "afl"))]
    if env::var(environment::RELEASE_MODE).unwrap_or("0".to_string()) == *"1" {
        ir.optimize();
    }

    #[cfg(features = "afl")]
    ir.optimize();

    ir.sort();

    let mut functions : Vec<_> = std::mem::take(&mut ir.functions).into_iter().map(|x| x.1).collect();
    functions.sort_unstable_by_key(|x| x.function_index.0);

    #[cfg(not(features = "afl"))]
    if env::var(environment::DUMP_IR).unwrap_or("0".to_string()) == *"1" {
        let mut string = String::new();
        for f in &functions {
            f.pretty_print(&ir, &mut string);
        }
        
        if let Ok(v) = env::var(environment::DUMP_IR_FILE) {
            std::fs::write(v, string.as_bytes()).unwrap()
        } else {
            println!("{string}");
        }
    }
    
    
    
    let (externs, extern_counter) = {
        let mut map = BTreeMap::new();
        let mut max = 0;

        for (_, e) in ir.extern_functions {
            if e.function_index.0 > max {
                max = e.function_index.0;
            }

            map.entry(e.file).or_insert_with(BTreeSet::new);
            map.get_mut(&e.file).unwrap().insert((e.path, e.function_index.0));
        }

        (map, max)
    };

    
    let constants = ir.constants;
    let mut codegen = CodeGen::new();

    
    codegen.codegen(&ir.symbol_table, externs, functions);

    let metadata = CompilationMetadata {
        extern_count: extern_counter,
    };

    (Ok((metadata, codegen.bytecode, constants, ir.symbol_table)), files_data)
}



pub fn convert_constants_to_bytes(constants: Vec<Data>, symbol_table: &SymbolTable) -> Vec<u8> {
    let mut constants_bytes = vec![];

    for constant in constants {
        match constant {
            Data::Float(v) => {
                constants_bytes.push(0);
                constants_bytes.append(&mut v.to_le_bytes().into());
            },
            
            Data::Bool(v) => {
                constants_bytes.push(1);
                constants_bytes.push(v.try_into().unwrap());
            },
            
            Data::String(v) => {
                constants_bytes.push(2);
                constants_bytes.append(&mut (symbol_table.get(v).as_bytes().len() as u64).to_le_bytes().to_vec());
                constants_bytes.append(&mut symbol_table.get(v).as_bytes().to_vec());
            },
            
            Data::Empty => panic!("empty data type shouldn't be constants"),

            Data::I8 (v) => {
                constants_bytes.push(3);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::I16(v) => {
                constants_bytes.push(4);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::I32(v) => {
                constants_bytes.push(5);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::I64(v) => {
                constants_bytes.push(6);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::U8 (v) => {
                constants_bytes.push(7);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::U16(v) => {
                constants_bytes.push(8);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::U32(v) => {
                constants_bytes.push(9);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },
            Data::U64(v) => {
                constants_bytes.push(10);
                constants_bytes.append(&mut v.to_le_bytes().into())
            },

        }
    }

    constants_bytes
}