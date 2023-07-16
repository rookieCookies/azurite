use std::collections::{HashMap, BTreeMap};

use azurite_ast_to_ir::{FunctionIndex, IR, Function, BlockTerminator, ExternFunction};
use azurite_common::Bytecode;
use common::{Data, SymbolIndex};

use crate::{CodegenModule, CodeGen};

pub struct BytecodeModule {
    bytecode: Vec<u8>,
    
    function_starts: HashMap<FunctionIndex, u32>,
    function_calls: Vec<(FunctionIndex, usize)>,
}


impl CodegenModule for BytecodeModule {
    fn codegen(
        mut state: crate::CodeGen<Self>,
        symbol_table: &mut common::SymbolTable, 
        externs: BTreeMap<SymbolIndex, Vec<ExternFunction>>, 
        functions: Vec<azurite_ast_to_ir::Function>,
        _: &[Data],
    ) -> Vec<u8> {
        let mut codegen = BytecodeModule {
            function_starts: HashMap::with_capacity(functions.len()),
            function_calls: Vec::new(),
            bytecode: Vec::new(),
        };

        
        for (file, functions) in externs {
            codegen.emit_bytecode(Bytecode::ExternFile);
            codegen.bytecode.append(&mut symbol_table.get(&file).as_bytes().to_vec());
            codegen.emit_byte(0);
            
            codegen.emit_byte(functions.len().try_into().unwrap());

            for func in functions {
                codegen.emit_u32(func.function_index.0);
                codegen.bytecode.append(&mut symbol_table.get(&func.path).as_bytes().to_vec());
                codegen.emit_byte(0);
            }
        }

        
        for function in functions {
            codegen.codegen_blocks(&mut state, function);
        }

        for (function_index, start) in codegen.function_calls.iter() {
            let value = codegen.function_starts.get(function_index).unwrap().to_le_bytes();

            codegen.bytecode[start + 1] = value[0];
            codegen.bytecode[start + 2] = value[1];
            codegen.bytecode[start + 3] = value[2];
            codegen.bytecode[start + 4] = value[3];
        }

        codegen.bytecode
    }
}


impl BytecodeModule {
    fn emit_byte(&mut self, byte: u8) {
        self.bytecode.push(byte)
    }


    fn emit_bytecode(&mut self, bytecode: Bytecode) {
        self.emit_byte(bytecode as u8)
    }


    fn emit_u32(&mut self, value: u32) {
        let bytes = value.to_le_bytes();

        self.emit_byte(bytes[0]);
        self.emit_byte(bytes[1]);
        self.emit_byte(bytes[2]);
        self.emit_byte(bytes[3]);
    }

    
    fn emit_u64(&mut self, value: u64) {
        let bytes = value.to_le_bytes();

        self.emit_byte(bytes[0]);
        self.emit_byte(bytes[1]);
        self.emit_byte(bytes[2]);
        self.emit_byte(bytes[3]);
        self.emit_byte(bytes[4]);
        self.emit_byte(bytes[5]);
        self.emit_byte(bytes[6]);
        self.emit_byte(bytes[7]);
    }


    fn codegen_blocks<T: CodegenModule>(&mut self, codegen: &mut CodeGen<T>, function: Function) {
        self.function_starts.insert(function.function_index, self.bytecode.len() as u32);
        self.emit_bytecode(Bytecode::Push);

        let temp = function.register_lookup.len() - function.arguments.len();
        self.emit_byte(temp.try_into().unwrap());


        let mut block_starts = HashMap::with_capacity(function.blocks.len() * 2);
        let mut block_endings = Vec::with_capacity(function.blocks.len());
        
        for block in &function.blocks {
            let block_start = self.bytecode.len();
            
            block.instructions.iter().for_each(|x| self.ir(codegen, x.clone()));

            let mut ending_buffer = match block.ending {
                BlockTerminator::Goto(_) => vec![255; 5],
                BlockTerminator::SwitchBool { .. } => vec![255; 10],
                BlockTerminator::Return => {
                    self.emit_bytecode(Bytecode::Pop);
                    self.emit_byte((function.register_lookup.len() - 1).try_into().unwrap());
                    vec![255]
                },
            };

            block_endings.push((self.bytecode.len(), block.ending.clone()));
            self.bytecode.append(&mut ending_buffer);
            
            block_starts.insert(block.block_index.0, block_start);
        }
        

        for (index, ending) in block_endings {
            match ending {
                BlockTerminator::Goto(v) => {
                    let goto_index = block_starts.get(&v.0).unwrap().to_le_bytes();

                    self.bytecode[index] = Bytecode::Jump as u8;
                    self.bytecode[index + 1] = goto_index[0];
                    self.bytecode[index + 2] = goto_index[1];
                    self.bytecode[index + 3] = goto_index[2];
                    self.bytecode[index + 4] = goto_index[3];
                },

                
                BlockTerminator::SwitchBool { cond, op1, op2 } => {
                    let goto_index_true = block_starts.get(&op1.0).unwrap().to_le_bytes();
                    let goto_index_false = block_starts.get(&op2.0).unwrap().to_le_bytes();

                    self.bytecode[index] = Bytecode::JumpCond as u8;
                    self.bytecode[index + 1] = cond.0 as u8;

                    self.bytecode[index + 2] = goto_index_true[0];
                    self.bytecode[index + 3] = goto_index_true[1];
                    self.bytecode[index + 4] = goto_index_true[2];
                    self.bytecode[index + 5] = goto_index_true[3];
                    
                    self.bytecode[index + 6] = goto_index_false[0];
                    self.bytecode[index + 7] = goto_index_false[1];
                    self.bytecode[index + 8] = goto_index_false[2];
                    self.bytecode[index + 9] = goto_index_false[3];
                    
                },

                
                BlockTerminator::Return => self.bytecode[index] = Bytecode::Return as u8,
                
                
            }
        }
    }

    
    pub fn ir<T: CodegenModule>(&mut self, state: &mut CodeGen<T>, ir: IR) {
        macro_rules! cast_to {
            ($v:ident, $dst: expr, $src: expr) => {
                {
                    self.emit_bytecode((Bytecode::$v));
                    self.emit_byte($dst.0 as u8);
                    self.emit_byte($src.0 as u8);
                }
            }
        }
        
        
        match ir {
            IR::Copy { src, dst } => {
                self.emit_bytecode(Bytecode::Copy);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(src.0 as u8);
            },

            
            IR::Swap { v1, v2 } => {
                self.emit_bytecode(Bytecode::Swap);
                self.emit_byte(v1.0 as u8);
                self.emit_byte(v2.0 as u8);
            },

            
            IR::Load { dst, data } => {
                self.emit_bytecode(Bytecode::LoadConst);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(data as u8);
            },
            

            IR::Call { id, dst, args } => {
                self.function_calls.push((id, self.bytecode.len()));
                
                self.emit_bytecode(Bytecode::Call);
                self.emit_u32(u32::MAX);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(args.len() as u8);

                for i in args {
                    self.emit_byte(i.0 as u8);
                }
                
            },

            
            IR::ExtCall { id: index, dst, args } => {
                self.emit_bytecode(Bytecode::ExtCall);
                self.emit_u32(index.0);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(args.len() as u8);

                for i in args {
                    self.emit_byte(i.0 as u8);
                }
                
            },

            
            IR::Add { dst, left, right } => {
                self.emit_bytecode(Bytecode::Add);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },

            
            IR::Subtract { dst, left, right } => {
                self.emit_bytecode(Bytecode::Subtract);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },

            
            IR::Multiply { dst, left, right } => {
                self.emit_bytecode(Bytecode::Multiply);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },

            
            IR::Divide { dst, left, right } => {
                self.emit_bytecode(Bytecode::Divide);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::Modulo { dst, left, right } => {
                self.emit_bytecode(Bytecode::Modulo);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::Equals { dst, left, right } => {
                self.emit_bytecode(Bytecode::Equals);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::NotEquals { dst, left, right } => {
                self.emit_bytecode(Bytecode::NotEquals);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::GreaterThan { dst, left, right } => {
                self.emit_bytecode(Bytecode::GreaterThan);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::LesserThan { dst, left, right } => {
                self.emit_bytecode(Bytecode::LesserThan);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::GreaterEquals { dst, left, right } => {
                self.emit_bytecode(Bytecode::GreaterEquals);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },
            
            
            IR::LesserEquals { dst, left, right } => {
                self.emit_bytecode(Bytecode::LesserEquals);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(left.0 as u8);
                self.emit_byte(right.0 as u8);
            },

            
            IR::Unit { dst } => {
                self.emit_bytecode(Bytecode::Unit);
                self.emit_byte(dst.0 as u8);
            },

            
            IR::Struct { dst, fields, id } => {
                self.emit_bytecode(Bytecode::Struct);
                self.emit_byte(dst.0 as u8);
                self.emit_u64(state.structures.get(&id).unwrap().id);
                self.emit_byte(fields.len() as u8);
                
                for i in fields {
                    self.emit_byte(i.0 as u8);
                }
                
            },

            
            IR::AccStruct { dst, val, index } => {
                self.emit_bytecode(Bytecode::AccStruct);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(val.0 as u8);
                self.emit_byte(index);
            },

            
            IR::SetField { dst, data, index } => {
                self.emit_bytecode(Bytecode::SetField);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(data.0 as u8);
                self.emit_byte(index);
            },

            IR::Noop => (),

            
            IR::UnaryNot { dst, val } => {
                self.emit_bytecode(Bytecode::UnaryNot);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(val.0 as u8);
            },

            
            IR::UnaryNeg { dst, val } => {
                self.emit_bytecode(Bytecode::UnaryNeg);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(val.0 as u8);
            },

            
            IR::CastToI8  { dst, val } => cast_to!(CastToI8,  dst, val),
            IR::CastToI16 { dst, val } => cast_to!(CastToI16, dst, val),
            IR::CastToI32 { dst, val } => cast_to!(CastToI32, dst, val),
            IR::CastToI64 { dst, val } => cast_to!(CastToI64, dst, val),
            IR::CastToU8  { dst, val } => cast_to!(CastToU8,  dst, val),
            IR::CastToU16 { dst, val } => cast_to!(CastToU16, dst, val),
            IR::CastToU32 { dst, val } => cast_to!(CastToU32, dst, val),
            IR::CastToU64 { dst, val } => cast_to!(CastToU64, dst, val),
            IR::CastToFloat { dst, val } => cast_to!(CastToFloat, dst, val),
        }
    }
}