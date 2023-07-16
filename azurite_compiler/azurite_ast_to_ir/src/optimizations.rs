mod register_alloc;

use std::collections::HashMap;

use common::{Data, DataType};

use crate::{ConversionState, Function, Block, BlockIndex, BlockTerminator, IR, FunctionIndex, Variable};

impl ConversionState {
    pub fn optimize(&mut self) {
        loop {
            let mut has_changed = false;
            {
                let mut used_functions = HashMap::from([(FunctionIndex(0), FunctionIndex(0))]);
                let mut counter = 1;

                for f in self.functions.iter_mut() {
                    for b in f.1.blocks.iter_mut() {
                        for i in b.instructions.iter_mut() {
                            match i {
                                IR::Call { id, .. } => {
                                    let val = match used_functions.entry(*id) {
                                        std::collections::hash_map::Entry::Occupied(v) => *v.get(),
                                        std::collections::hash_map::Entry::Vacant(v) => {
                                            counter += 1;
                                            *v.insert(FunctionIndex(counter-1))
                                        }
                                    };

                                    *id = val;
                                },
                                _ => continue,
                            }
                        }
                    }
                }

                for f in self.functions.iter().map(|x| (*x.0, x.1.function_index)).collect::<Vec<_>>() {
                    if let Some(mapping) = used_functions.get(&f.1) {
                        self.functions.get_mut(&f.0).unwrap().function_index = *mapping;
                    } else {
                        has_changed = true;
                        self.functions.remove(&f.0);
                    }
                }
            }


            if self.functions.iter_mut().map(|x| x.1.optimize(true)).any(|x| x) {
                has_changed = true
            }

            for f in self.functions.iter_mut() {
                f.1.register_alloc();
            }


            
            if !has_changed {
                break
            }
        }

        
        for f in self.functions.iter_mut() {
            f.1.register_alloc();
        }

        let mut used_consts = HashMap::new();
        let mut constant_counter = 0;
        for f in self.functions.iter_mut() {
            for b in f.1.blocks.iter_mut() {
                for i in b.instructions.iter_mut() {
                    if let IR::Load { data, .. } = i {
                        if let Some(v) = used_consts.get(data) {
                            *data = *v;
                        }

                        used_consts.insert(*data, constant_counter);
                        *data = constant_counter;
                        
                        constant_counter += 1;
                    }
                }
            }
        }


        let mut vec = used_consts.into_iter().collect::<Vec<(u32, u32)>>();
        vec.sort_unstable_by_key(|x| x.1);

        let mut new_constants = Vec::with_capacity(self.constants.len());
        let mut old_constants = std::mem::take(&mut self.constants);
        for i in vec {
            new_constants.push(std::mem::replace(&mut old_constants[i.0 as usize], Data::I8(i8::MAX)));
        }

        self.constants = new_constants;


        for f in self.functions.iter() {
            for b in f.1.blocks.iter() {
                for i in b.instructions.iter() {
                    if let IR::Struct { id, .. } = i {
                        self.structures.get_mut(id).unwrap().is_used = true;
                    }
                }
            }
        }


        for f in self.functions.iter_mut() {
            f.1.remove_unused_registers();
        }

    }
}

impl Function {
    pub fn optimize(&mut self, inline: bool) -> bool {
        let mut has_changed = false;

        // Dead block analysis
        {
            let mut block_stack = vec![self.entry];
            let mut new_blocks : Vec<Block> = Vec::with_capacity(self.blocks.len());

            while let Some(block_id) = block_stack.pop() {
                if new_blocks.iter().any(|x| x.block_index == block_id) {
                    continue
                }


                let raw_block_index = self.blocks.iter().enumerate().find(|x| x.1.block_index == block_id).unwrap().0;
                let block = self.blocks.remove(raw_block_index);

                match &block.ending {
                    crate::BlockTerminator::Goto(v) => block_stack.push(*v),
                    crate::BlockTerminator::SwitchBool { op1, op2, .. } => {
                        block_stack.push(*op1);
                        block_stack.push(*op2);
                    },
                    crate::BlockTerminator::Return => (),
                };

                new_blocks.push(block)
            }
            
            self.blocks = new_blocks;
        }
        

        if inline {
            let block_ids = self.blocks.iter().map(|x| x.block_index).collect::<Vec<_>>();
            'out: for block_id in &block_ids {
                let block = self.find_block(*block_id);

                let goto_id = match block.ending {
                    crate::BlockTerminator::Goto(v) => v,
                    _ => continue,
                };

                for bi in &block_ids {
                    if block_id == bi {
                        continue
                    }
                    
                    let b = self.find_block(*bi);

                    let is_matching = match b.ending {
                        BlockTerminator::Goto(v) => v == goto_id,
                        BlockTerminator::SwitchBool { op1, op2, .. } => op1 == goto_id || op2 == goto_id,
                        BlockTerminator::Return => false,
                    };

                    if is_matching {
                        continue 'out;
                    }
                }

                let goto_block = self.find_block_mut(goto_id);
                
                let ending = goto_block.ending.clone();
                let mut instructions = std::mem::take(&mut goto_block.instructions);

                let block = self.find_block_mut(*block_id);
                block.ending = ending;
                block.instructions.append(&mut instructions);
                has_changed = true;
                // break
            }


        }


        {
            let block_ids = self.blocks.iter().map(|x| x.block_index).collect::<Vec<_>>();
            // let block_used_registers = HashMap::with_capacity(self.blocks.len());

            for block_id in &block_ids {
                let block = self.find_block_mut(*block_id);

                loop {
                    let mut remove = None;
                    let mut last_instruction = None;
                    for (index, instruction) in block.instructions.iter_mut().enumerate().rev() {
                        let val = match instruction {
                            IR::Copy { dst, src } => Some((*dst, *src)),
                            _ => None,
                        };

                        let copy = last_instruction;
                        last_instruction = val;
                    
                        let Some((last_dst, last_src)) = copy else { continue };

                        match instruction {
                            IR::Copy { dst, .. }
                            | IR::CastToI8 { dst, .. }
                            | IR::CastToI16 { dst, .. }
                            | IR::CastToI32 { dst, .. }
                            | IR::CastToI64 { dst, .. }
                            | IR::CastToU8 { dst, .. }
                            | IR::CastToU16 { dst, .. }
                            | IR::CastToU32 { dst, .. }
                            | IR::CastToU64 { dst, .. }
                            | IR::CastToFloat { dst, .. }
                            | IR::Unit { dst }
                            | IR::Load { dst, .. }
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
                            | IR::Call { dst, ..}
                            | IR::ExtCall { dst, .. }
                            | IR::UnaryNot { dst, .. }
                            | IR::UnaryNeg { dst, .. }
                            | IR::Struct { dst, .. }
                            | IR::AccStruct { dst, ..  } 
                            | IR::SetField { dst, .. } => {
                                if *dst == last_src {
                                    *dst = last_dst;
                                    remove = Some(index + 1);
                                    break
                                }
                            },

                            
                            | IR::Swap { .. }
                            | IR::Noop => (),
                        }
                    }
                    let Some(v) = remove else { break };
                    block.instructions.remove(v);
                }
            } 
            
        }



        {
            let mut remove = vec![];
            for block in self.blocks.iter_mut() {
                remove.clear();

                for (index, i) in block.instructions.iter().enumerate() {
                    match i {
                        | IR::Copy { dst: v1, src: v2 }
                        | IR::Swap { v1, v2 } => {
                            if v1 == v2 {
                                remove.push(index)
                            }
                        },

                        _ => continue,
                    }
                }

                for i in remove.iter().rev() {
                    block.instructions.remove(*i);
                }
            }
        }



        {
            let mut block_mapping = HashMap::with_capacity(self.blocks.len());

            for (block_counter, block) in self.blocks.iter_mut().enumerate() {
                block_mapping.insert(block.block_index, BlockIndex(block_counter as u32));
                block.block_index = BlockIndex(block_counter as u32);
            }

            for block in self.blocks.iter_mut() {
                match &mut block.ending {
                    crate::BlockTerminator::Goto(v) => {
                        *v = *block_mapping.get(v).unwrap()
                    },
                    
                    crate::BlockTerminator::SwitchBool { op1, op2, .. } => {
                        *op1 = *block_mapping.get(op1).unwrap();
                        *op2 = *block_mapping.get(op2).unwrap();
                    },
                    
                    crate::BlockTerminator::Return => (),
                }
            }
            
        }


        has_changed
    }

}


impl Function {
    fn remove_unused_registers(&mut self) {
        // Remove unused registers
        {
            fn update_reg(reg: &mut Variable, mapping: &mut HashMap<Variable, Variable>, counter: &mut u32) {
                if !mapping.contains_key(reg) {
                    assert!(mapping.insert(*reg, Variable(*counter)).is_none());
                    *counter += 1;
                }

                *reg = *mapping.get(reg).unwrap();
                
            }

            
            let mut register_counter = 0u32;
            let mut register_mapping = HashMap::with_capacity(self.register_lookup.len());

            register_mapping.insert(Variable(0), Variable(0));
            register_counter += 1;

            for i in 0..self.arguments.len() {
                register_mapping.insert(Variable(i as u32 + 1), Variable(register_counter as u32));
                register_counter += 1;
            }

            assert_eq!(register_mapping.len(), register_counter as usize);

            
            for b in self.blocks.iter_mut() {
                for i in b.instructions.iter_mut() {
                    match i {
                        | IR::Copy { dst: v1, src: v2 }
                        | IR::Swap { v1, v2 }
                        | IR::CastToI8 { dst: v1, val: v2 }
                        | IR::CastToI16 { dst: v1, val: v2 }
                        | IR::CastToI32 { dst: v1, val: v2 }
                        | IR::CastToI64 { dst: v1, val: v2 }
                        | IR::CastToU8 { dst: v1, val: v2 }
                        | IR::CastToU16 { dst: v1, val: v2 }
                        | IR::CastToU32 { dst: v1, val: v2 }
                        | IR::AccStruct { dst: v1, val: v2, ..}
                        | IR::SetField { dst: v1, data: v2, .. }
                        | IR::CastToU64 { dst: v1, val: v2 }
                        | IR::CastToFloat { dst: v1, val: v2 }
                        | IR::UnaryNot { dst: v1, val: v2 }
                        | IR::UnaryNeg { dst: v1, val: v2 } => {
                            update_reg(v1, &mut register_mapping, &mut register_counter);
                            update_reg(v2, &mut register_mapping, &mut register_counter);
                        }

                        
                        | IR::Add { dst, left, right }
                        | IR::Subtract { dst, left, right }
                        | IR::Multiply { dst, left, right }
                        | IR::Divide { dst, left, right }
                        | IR::Modulo { dst, left, right }
                        | IR::Equals { dst, left, right }
                        | IR::NotEquals { dst, left, right }
                        | IR::GreaterThan { dst, left, right }
                        | IR::LesserThan { dst, left, right }
                        | IR::GreaterEquals { dst, left, right }
                        | IR::LesserEquals { dst, left, right } => {
                            update_reg(dst, &mut register_mapping, &mut register_counter);
                            update_reg(left, &mut register_mapping, &mut register_counter);
                            update_reg(right, &mut register_mapping, &mut register_counter);
                        }

                        | IR::ExtCall { dst, args, .. }
                        | IR::Struct { dst, fields: args, .. }
                        | IR::Call { dst, args, .. } => {
                            update_reg(dst, &mut register_mapping, &mut register_counter);

                            for a in args.iter_mut() {
                                update_reg(a, &mut register_mapping, &mut register_counter);
                            }
                        },
                    


                        | IR::Load { dst, .. } 
                        | IR::Unit { dst } => {
                            update_reg(dst, &mut register_mapping, &mut register_counter);
                        }

                        
                        IR::Noop => (),
                    }
                }

                if let BlockTerminator::SwitchBool { cond, .. } = &mut b.ending {
                    update_reg(cond, &mut register_mapping, &mut register_counter);
                }
            }


            assert_eq!(register_mapping.len(), register_counter as usize);
            let old_lookup = std::mem::replace(&mut self.register_lookup, vec![DataType::Empty; register_counter as usize]);
            for i in register_mapping.iter() {
                self.register_lookup[i.1.0 as usize] = old_lookup[i.0.0 as usize].clone()
            }

        }
    }
}
