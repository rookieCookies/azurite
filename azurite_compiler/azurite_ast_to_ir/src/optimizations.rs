use std::collections::HashMap;

use crate::{ConversionState, Function, Block, BlockIndex, BlockTerminator, IR, FunctionIndex};

impl ConversionState {
    pub fn optimize(&mut self) {
        loop {
            let mut has_changed = self.functions.iter_mut().map(|x| x.1.optimize(true)).any(|x| x);

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

            
            if !has_changed {
                break
            }
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
                            | IR::Unit { dst }
                            | IR::Load { dst, .. }
                            | IR::Add { dst, .. } 
                            | IR::Subtract { dst, .. } 
                            | IR::Multiply { dst, .. } 
                            | IR::Divide { dst, .. } 
                            | IR::Equals { dst, .. } 
                            | IR::NotEquals { dst, .. } 
                            | IR::GreaterThan { dst, .. } 
                            | IR::LesserThan { dst, .. } 
                            | IR::GreaterEquals { dst, .. } 
                            | IR::LesserEquals { dst, .. }
                            | IR::Call { dst, ..}
                            | IR::ExtCall { dst, .. }
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
