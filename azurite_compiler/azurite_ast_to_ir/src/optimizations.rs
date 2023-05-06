use std::collections::HashMap;

use crate::{ConversionState, Function, Block, BlockIndex, BlockTerminator};

impl ConversionState<'_> {
    pub fn optimize(&mut self) {
         loop {
            let has_changed = self.functions.iter_mut().map(|x| x.optimize(true)).any(|x| x);
            
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
