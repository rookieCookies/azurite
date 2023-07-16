#![feature(iter_intersperse)]

pub mod bytecode_module;
pub mod c_module;

use std::{collections::{HashMap, BTreeMap}, marker::PhantomData};

use azurite_ast_to_ir::{Function, Structure, ExternFunction};
use common::{SymbolTable, SymbolIndex, Data};

#[derive(Debug)]
pub struct CodeGen<T: CodegenModule> {
    pub bytecode: Vec<u8>,

    structures: HashMap<SymbolIndex, Structure>,

    phantom_data: PhantomData<T>,
}

impl<T: CodegenModule> CodeGen<T> {
    pub fn codegen(
        self,
        symbol_table: &mut SymbolTable, 
        externs: BTreeMap<SymbolIndex, Vec<ExternFunction>>, 
        functions: Vec<Function>, 
        constants: &[Data],
        ) -> Vec<u8> {
            T::codegen(self, symbol_table, externs, functions, constants)
        }
        


    pub fn new(structures: HashMap<SymbolIndex, Structure>) -> Self {
        Self {
            bytecode: Vec::new(),
            structures,
            phantom_data: PhantomData,
        }
    }
}


pub trait CodegenModule: Sized {
    fn codegen(
        state: CodeGen<Self>,
        symbol_table: &mut SymbolTable, 
        externs: BTreeMap<SymbolIndex, Vec<ExternFunction>>, 
        functions: Vec<Function>,
        constants: &[Data],
    ) -> Vec<u8>;
}
