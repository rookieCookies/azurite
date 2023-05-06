use std::collections::HashMap;

use azurite_ast_to_ir::{IR, BlockTerminator, FunctionIndex, Function};
use azurite_common::Bytecode;

#[derive(Debug)]
pub struct CodeGen {
    pub bytecode: Vec<u8>,

    function_starts: HashMap<FunctionIndex, u32>,
    function_calls: Vec<(FunctionIndex, usize)>,
}

impl CodeGen {
    pub fn codegen(&mut self, functions: Vec<Function>) {
        for function in functions {
            self.codegen_blocks(function);
        }

        for (function_index, start) in self.function_calls.iter() {
            let value = self.function_starts.get(function_index).unwrap().to_le_bytes();

            self.bytecode[start + 1] = value[0];
            self.bytecode[start + 2] = value[1];
            self.bytecode[start + 3] = value[2];
            self.bytecode[start + 4] = value[3];
        }
    }

    fn codegen_blocks(&mut self, function: Function) {
        self.function_starts.insert(function.function_index, self.bytecode.len() as u32);
        self.emit_bytecode(Bytecode::Push);
        self.emit_byte((function.stack_size - function.argument_count as u32) as u8);


        let mut block_starts = HashMap::with_capacity(function.blocks.len() * 2);
        let mut block_endings = Vec::with_capacity(function.blocks.len());
        
        for block in function.blocks {
            let block_start = self.bytecode.len();
            
            block.instructions.into_iter().for_each(|x| self.ir(x));

            let mut ending_buffer = match block.ending {
                BlockTerminator::Goto(_) => vec![255; 5],
                BlockTerminator::SwitchBool { .. } => vec![255; 10],
                BlockTerminator::Return => {
                    self.emit_bytecode(Bytecode::Pop);
                    self.emit_byte((function.stack_size + function.argument_count as u32 - 1) as u8);
                    vec![255]
                },
            };

            block_endings.push((self.bytecode.len(), block.ending));
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

    pub fn ir(&mut self, ir: IR) {
        match ir {
            IR::Copy { src, dst } => {
                self.emit_bytecode(Bytecode::Copy);
                self.emit_byte(src.0 as u8);
                self.emit_byte(dst.0 as u8);
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

            
            IR::Struct { dst, r1, r2 } => {
                self.emit_bytecode(Bytecode::Struct);
                self.emit_byte(dst.0 as u8);
                self.emit_byte(r1.0 as u8);
                self.emit_byte(r2.0 as u8);
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

            
        }
    }

    pub fn new() -> Self {
        Self {
            bytecode: Vec::new(),
            function_starts: HashMap::new(),
            function_calls: vec![],
        }
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}

