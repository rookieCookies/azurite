use std::collections::{HashMap, HashSet};

use crate::{ConversionState, Function, Block, BlockIndex, BlockTerminator, Variable, IR};

impl ConversionState<'_> {
    pub fn optimize(&mut self) {
         loop {
            let has_changed = self.functions.iter_mut().map(|x| x.optimize(true)).any(|x| x);
            
            if !has_changed {
                break
            }
        }

        // self.functions.iter_mut().for_each(Function::reg_alloc);

        // self.functions.iter_mut().for_each(|x| 
        //     {
        //         let mut already_done = HashSet::with_capacity(x.blocks.len());
        //         let mut mapping = Map((0..x.stack_size).map(Variable).collect());
        //         let mut already_done = HashSet::with_capacity(x.blocks.len());

        //         x.copy_prop(x.entry, &mut already_done, &mut rem_buffer, &mut mapping)
        //     }
        // );

        
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


//     fn copy_prop(&mut self, block: BlockIndex, already_done: &mut HashSet<BlockIndex>, rem_buffer: &mut Vec<usize>, mapping: &mut Map ) {
//         if already_done.contains(&block) {
//             return;
//         }

//         let block = self.find_block_mut(block);

//         for i in block.instructions.iter_mut().enumerate() {
//             match i.1 {
//                 crate::IR::Copy { dst, src } => {
//                     mapping.set(*dst, *src);
//                     // rem_buffer.push(i.0);
//                 },

                
//                 crate::IR::Swap { v1, v2 } => {
//                     *v1 = mapping.get(*v1);
//                     *v2 = mapping.get(*v2);
//                 },

                
//                 | crate::IR::Unit { dst }
//                 | crate::IR::Load { dst, .. } => {
//                     *dst = mapping.get(*dst);
//                 },

                
//                 | crate::IR::Add { dst, left, right }
//                 | crate::IR::Subtract { dst, left, right }
//                 | crate::IR::Multiply { dst, left, right }
//                 | crate::IR::Divide { dst, left, right }
//                 | crate::IR::Equals { dst, left, right }
//                 | crate::IR::NotEquals { dst, left, right }
//                 | crate::IR::GreaterThan { dst, left, right }
//                 | crate::IR::LesserThan { dst, left, right }
//                 | crate::IR::GreaterEquals { dst, left, right }
//                 | crate::IR::LesserEquals { dst, left, right } => {
//                     dbg!(&mapping);
//                     *dst = mapping.get(*dst);
//                     // *dst = mapping[dst.0 as usize];
//                     *left = mapping.get(*left);
//                     *right = mapping.get(*right);
//                 }

//                 | crate::IR::Struct { dst, fields: args }
//                 | crate::IR::ExtCall { dst, args, .. }
//                 | crate::IR::Call { dst, args, .. } => {
//                     // *dst = mapping[dst.0 as usize];
//                     *dst = mapping.get(*dst);

//                     for i in args.iter_mut() {
//                         *i = mapping.get(*i)
//                     }
//                 },
                
//                 | crate::IR::SetField { dst, data: val, .. }
//                 | crate::IR::AccStruct { dst, val, ..} => {
//                     // *dst = mapping[dst.0 as usize];
//                     *dst = mapping.get(*dst);
                                
//                     *val = mapping.get(*val);
//                 },
//                 IR::Copy { dst, src } => todo!(),
//                 IR::Swap { v1, v2 } => todo!(),
//                 IR::Load { dst, data } => todo!(),
//                 IR::Unit { dst } => todo!(),
//                 IR::Add { dst, left, right } => todo!(),
//                 IR::Subtract { dst, left, right } => todo!(),
//                 IR::Multiply { dst, left, right } => todo!(),
//                 IR::Divide { dst, left, right } => todo!(),
//                 IR::Equals { dst, left, right } => todo!(),
//                 IR::NotEquals { dst, left, right } => todo!(),
//                 IR::GreaterThan { dst, left, right } => todo!(),
//                 IR::LesserThan { dst, left, right } => todo!(),
//                 IR::GreaterEquals { dst, left, right } => todo!(),
//                 IR::LesserEquals { dst, left, right } => todo!(),
//                 IR::Call { dst, id, args } => todo!(),
//                 IR::ExtCall { dst, index, args } => todo!(),
//                 IR::Struct { dst, fields } => todo!(),
//                 IR::AccStruct { dst, val, index } => todo!(),
//                 IR::SetField { dst, data, index } => todo!(),
//                 IR::Noop => todo!(),
//             }
//         }

//         rem_buffer.iter().rev().for_each(|x| { block.instructions.remove(*x); });
//         rem_buffer.clear();

//         already_done.insert(block.block_index);
//         // dbg!(&block, &mapping);
            
//         match block.ending {
//             BlockTerminator::Goto(v) => self.copy_prop(v, already_done, rem_buffer, mapping),
//             BlockTerminator::SwitchBool { op1, op2, .. } => {
//                 self.copy_prop(op1, already_done, rem_buffer, mapping);
//                 self.copy_prop(op2, already_done, rem_buffer, mapping)
//             },
                
//             BlockTerminator::Return => (),
//         }
//     }
}

#[derive(Debug)]
struct Map(Vec<Variable>);
impl Map {
    fn get(&self, reg: Variable) -> Variable {
        let val = self.0[reg.0 as usize];

        if val == reg {
            return reg
        }
        self.get(val)
    }

    fn set(&mut self, reg: Variable, set: Variable) {
        self.0[reg.0 as usize] = set
    }
}
