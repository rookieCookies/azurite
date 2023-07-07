use std::{collections::HashMap, slice::Iter};

use azurite_parser::ast::Instruction;
use rayon::prelude::{IntoParallelRefMutIterator, ParallelIterator};

use crate::{BlockIndex, Variable, Function, BlockTerminator, IR};

impl Function {
    pub fn register_alloc(&mut self) {
        // A map of registers used by a block
        let block_map = self.generate_block_map();
        
        self.blocks.par_iter_mut().for_each(|b| {
            let mut storage = vec![];

            let iterator = b.instructions.iter();

            {
                for (index, i) in iterator.clone().enumerate() {
                    let result = match i {
                        
                        IR::Swap { v1, v2 } => {
                            !(is_register_used_later(*v1, &b.ending, &iterator, &block_map))
                            && !(is_register_used_later(*v2, &b.ending, &iterator, &block_map))
                        },

                        
                        | IR::Copy { dst, .. }
                        | IR::Struct { dst, .. }
                        | IR::AccStruct { dst, .. }
                        | IR::SetField { dst, .. }
                        | IR::CastToI8 { dst, .. }
                        | IR::CastToI16 { dst, .. }
                        | IR::CastToI32 { dst, .. }
                        | IR::CastToI64 { dst, .. }
                        | IR::CastToU8 { dst, .. }
                        | IR::CastToU16 { dst, .. }
                        | IR::CastToU32 { dst, .. }
                        | IR::CastToU64 { dst, .. }
                        | IR::CastToFloat { dst, .. }
                        | IR::Add { dst, .. }
                        | IR::Subtract { dst, .. }
                        | IR::Multiply { dst, .. }
                        | IR::Divide { dst, .. }
                        | IR::Modulo { dst, .. }
                        | IR::Equals { dst, .. }
                        | IR::NotEquals { dst, .. }
                        | IR::GreaterThan { dst, .. }
                        | IR::LesserThan { dst, .. }
                        | IR::GreaterEquals { dst, .. }
                        | IR::LesserEquals { dst, .. }
                        | IR::UnaryNot { dst, .. }
                        | IR::UnaryNeg { dst, .. }
                        | IR::Load { dst, .. } => {
                            !(is_register_used_later(*dst, &b.ending, &iterator, &block_map))
                        },

                        
                        _ => false
                    };

                    if result {
                        storage.push(index);
                    }
                }
            }

            for index in storage.into_iter().rev() {
                b.instructions.remove(index);
            }
        });


        
    }
}

fn is_register_used_later(reg: Variable, term: &BlockTerminator, iter: &Iter<IR>, block_map: &HashMap<BlockIndex, (Box<[Variable]>, BlockTerminator)>) -> bool {
    // Function return register
    if reg == Variable(0) {
        return true
    }

    
    let mut storage = Vec::with_capacity(10);
    for f in iter.clone() {
        instruction_used_registers(f, &mut storage);
        if storage.contains(&reg) {
            return true
        }

        storage.clear();
    }


    fn recursive_block_search(terminator: BlockTerminator, register: Variable, block_map: &HashMap<BlockIndex, (Box<[Variable]>, BlockTerminator)>) -> bool {
        match terminator {
            BlockTerminator::Goto(v) => {
                let (used, term) = block_map.get(&v).unwrap();
                if used.contains(&register) {
                    return true
                }

                recursive_block_search(term.clone(), register, block_map)
            },

            
            BlockTerminator::SwitchBool { cond, op1, op2 } => {
                if cond == register {
                    return true
                }

                let (used, term) = block_map.get(&op1).unwrap();
                if used.contains(&register) {
                    return true
                }

                if recursive_block_search(term.clone(), register, block_map) {
                    return true
                }

                
                let (used, term) = block_map.get(&op2).unwrap();
                if used.contains(&register) {
                    return true
                }

                recursive_block_search(term.clone(), register, block_map)
            },

            
            BlockTerminator::Return => false,
        }
    }


    recursive_block_search(term.clone(), reg, block_map)
}



impl Function {
    fn generate_block_map(&self) -> HashMap<BlockIndex, (Box<[Variable]>, BlockTerminator)> {
        let mut block_map : HashMap<BlockIndex, (Box<[Variable]>, BlockTerminator)> = HashMap::new();
        {
            let mut storage = vec![];
            for b in self.blocks.iter() {
                for i in b.instructions.iter() {
                    instruction_used_registers(i, &mut storage)
                }

                block_map.insert(b.block_index, (storage.clone().into_boxed_slice(), b.ending.clone()));
            }
        }

        block_map
    }
}


fn instruction_used_registers(i: &IR, storage: &mut Vec<Variable>) {
    match i {
        crate::IR::Copy { src, .. } => {
            storage.push(*src);
        },

    
        crate::IR::Swap { v1, v2 } => {
            storage.push(*v1);
            storage.push(*v2);
        },

        | crate::IR::Add { left, right, .. }
        | crate::IR::Subtract { left, right, .. }
        | crate::IR::Multiply { left, right, .. }
        | crate::IR::Divide { left, right, .. }
        | crate::IR::Modulo { left, right, .. }
        | crate::IR::Equals { left, right, .. }
        | crate::IR::NotEquals { left, right, .. }
        | crate::IR::GreaterThan { left, right, .. }
        | crate::IR::LesserThan { left, right, .. }
        | crate::IR::GreaterEquals { left, right, .. }
        | crate::IR::LesserEquals { left, right, .. } => {
            storage.push(*left);
            storage.push(*right);
        }

    
        crate::IR::AccStruct { val, .. } => {
            storage.push(*val)
        },

    
        crate::IR::SetField { dst, data, .. } => {
            storage.push(*data);
            storage.push(*dst);
        },

    
        | crate::IR::CastToI8  { val, .. }
        | crate::IR::CastToI16 { val, .. }
        | crate::IR::CastToI32 { val, .. }
        | crate::IR::CastToI64 { val, .. }
        | crate::IR::CastToU8  { val, .. }
        | crate::IR::CastToU16 { val, .. }
        | crate::IR::CastToU32 { val, .. }
        | crate::IR::CastToU64 { val, .. }
        | crate::IR::CastToFloat { val, .. } => {
            storage.push(*val);
        },

    
        | crate::IR::ExtCall { args, .. }
        | crate::IR::Struct { fields: args, .. }
        | crate::IR::Call { args, .. } => {
            args.iter().copied().for_each(|x| storage.push(x))
        },


        _ => ()
    }
}