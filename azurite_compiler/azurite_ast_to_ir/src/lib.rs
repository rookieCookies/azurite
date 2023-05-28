pub mod optimizations;

use std::{mem::replace, fmt::{Display, Write}, collections::HashMap};

use azurite_parser::ast::{Instruction, Expression, BinaryOperator, Statement, InstructionKind, Declaration};
use common::{Data, SymbolIndex, SymbolTable};
use rayon::prelude::{ParallelIterator, IntoParallelRefMutIterator};

#[derive(Debug, PartialEq)]
pub struct ConversionState {
    pub constants: Vec<Data>,
    pub functions: Vec<Function>,
    pub externs: Vec<ExternFunction>,

    files: HashMap<SymbolIndex, HashMap<SymbolIndex, FunctionLookup>>,
    function_counter: u32,
    extern_counter: u32,
    
    pub symbol_table: SymbolTable,
    current_file: SymbolIndex,
}


#[derive(Debug, PartialEq)]
pub struct Function {
    pub function_index: FunctionIndex,
    variable_lookup: Vec<(SymbolIndex, Variable)>,
    
    pub argument_count: usize,
    pub variable_counter: u32,
    pub stack_size: u32,
    block_counter: u32,

    breaks: Vec<BlockIndex>,
    continues: Vec<BlockIndex>,
    explicit_ret: Vec<BlockIndex>,

    pub blocks: Vec<Block>,
    entry: BlockIndex,
    
}


#[derive(Debug, PartialEq)]
pub struct ExternFunction {
    pub path: SymbolIndex,
    pub functions: Vec<SymbolIndex>,
}


#[derive(Debug, PartialEq)]
pub struct Block {
    pub block_index: BlockIndex,
    pub instructions: Vec<IR>,
    pub ending: BlockTerminator,
}


#[derive(Debug, PartialEq)]
pub enum FunctionLookup {
    Normal(FunctionIndex),
    Extern(u32),
}

impl FunctionLookup {
    pub fn without_type(&self) -> u32 {
        match self {
            FunctionLookup::Normal(v) => v.0,
            FunctionLookup::Extern(v) => *v,
        }
    }
}


#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)] pub struct FunctionIndex(pub u32);
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)] pub struct BlockIndex(pub u32);
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)] pub struct Variable(pub u32);


impl Display for FunctionIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for BlockIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}


#[derive(Debug, Clone, PartialEq)]
pub enum BlockTerminator {
    Goto(BlockIndex),
    SwitchBool { cond: Variable, op1: BlockIndex, op2: BlockIndex },
    Return,
}


#[derive(Debug, Clone, PartialEq)]
pub enum IR {
    Copy     { dst: Variable, src: Variable },
    Swap     { v1: Variable, v2: Variable },
    
    Load     { dst: Variable, data: u32 },
    Unit     { dst: Variable },
    
    Add           { dst: Variable, left: Variable, right: Variable },
    Subtract      { dst: Variable, left: Variable, right: Variable },
    Multiply      { dst: Variable, left: Variable, right: Variable },
    Divide        { dst: Variable, left: Variable, right: Variable },
    Equals        { dst: Variable, left: Variable, right: Variable },
    NotEquals     { dst: Variable, left: Variable, right: Variable },
    GreaterThan   { dst: Variable, left: Variable, right: Variable },
    LesserThan    { dst: Variable, left: Variable, right: Variable },
    GreaterEquals { dst: Variable, left: Variable, right: Variable },
    LesserEquals  { dst: Variable, left: Variable, right: Variable },

    Call          { dst: Variable, id: FunctionIndex,  args: Vec<Variable> },
    ExtCall       { dst: Variable, index: u32,         args: Vec<Variable> },
    
    Struct        { dst: Variable, fields: Vec<Variable> },
    AccStruct     { dst: Variable, val: Variable, index: u8 },
    SetField      { dst: Variable, data: Variable, index: u8},

    Noop,
}


pub enum Result {
    Variable(Variable),
}


impl ConversionState {
    pub fn new(mut symbol_table: SymbolTable) -> Self { 
        Self {
            current_file: symbol_table.add(String::from(":root")),
            constants: vec![],
            functions: vec![],
            symbol_table,
            function_counter: 0,
            files: HashMap::new(),
            externs: vec![],
            extern_counter: 0,

        }
    }

    pub fn generate(&mut self, mut files: Vec<(SymbolIndex, Vec<Instruction>)>) {
        files.sort_by_key(|x| x.0);
        
        let root_index = self.symbol_table.add(String::from(":root"));
        let mut function = Function::new(self.function(), 0);

        files.iter().for_each(|x| { self.files.insert(x.0, HashMap::new()); });
        for file in files.iter() {
            if file.0 == root_index {
                continue
            }
            self.declaration_process(file.0, &file.1);
        }
        

        let mut root = None;
        let mut vec = vec![];
        for file in files {
            if file.0 == root_index {
                root = Some(file.1);
                continue;
            }

            self.current_file = file.0;

            let mut function = Function::new(self.function(), 0);
            function.generate(self, file.1);
            vec.push(function.function_index);
            self.functions.push(function);
        }

        let vec = vec.into_iter().map(|x| IR::Call { dst: Variable(0), id: x, args: vec![] }).collect();
        let block = Block { block_index: function.block(), instructions: vec, ending: BlockTerminator::Goto(BlockIndex(1)) };
        function.blocks.push(block);
        function.generate(self, root.unwrap());

        self.functions.push(function);
    }

    pub fn pretty_print(&mut self) -> String {
        let mut lock = String::new();
        for i in &self.functions {
            let _ = writeln!(lock, "fn {}", i.function_index);
            for block in &i.blocks {
                let _ = writeln!(lock, "  bb{}:", block.block_index.0);
                for ir in &block.instructions {
                    let _ = write!(lock, "    ");
                    let _ = match ir {
                        IR::Load { dst, data }                 => writeln!(lock, "load {dst} {}", self.constants[*data as usize].to_string(&self.symbol_table)),
                        IR::Add { dst, left, right }           => writeln!(lock, "add {dst} {left} {right}"),
                        IR::Subtract { dst, left, right }      => writeln!(lock, "sub {dst} {left} {right}"),
                        IR::Multiply { dst, left, right }      => writeln!(lock, "mul {dst} {left} {right}"),
                        IR::Divide { dst, left, right }        => writeln!(lock, "div {dst} {left} {right}"),
                        IR::Copy { src, dst }                  => writeln!(lock, "copy {src} {dst}"),
                        IR::Swap { v1, v2 }                    => writeln!(lock, "swap {v1} {v2}"),
                        IR::Equals { dst, left, right }        => writeln!(lock, "eq {dst} {left} {right}"),
                        IR::NotEquals { dst, left, right }     => writeln!(lock, "neq {dst} {left} {right}"),
                        IR::GreaterThan { dst, left, right }   => writeln!(lock, "gt {dst} {left} {right}"),
                        IR::LesserThan { dst, left, right }    => writeln!(lock, "lt {dst} {left} {right}"),
                        IR::GreaterEquals { dst, left, right } => writeln!(lock, "ge {dst} {left} {right}"),
                        IR::LesserEquals { dst, left, right }  => writeln!(lock, "le {dst} {left} {right}"),
                        IR::Call { id, dst, args }             => writeln!(lock, "call {id} {dst} ({} )", args.iter().map(|x| format!(" {x}")).collect::<String>()),
                        IR::ExtCall { index, dst, args }       => writeln!(lock, "ecall {index} {dst} ({} )", args.iter().map(|x| format!(" {x}")).collect::<String>()),
                        IR::Unit { dst }                       => writeln!(lock, "unit {dst}"),
                        IR::Struct { dst, fields }             => writeln!(lock, "struct {dst} ({} )", fields.iter().map(|x| format!(" {x}")).collect::<String>()),
                        IR::AccStruct { dst, val, index }      => writeln!(lock, "accstruct, {dst} {val} {index}"),
                        IR::SetField { dst, data, index }      => writeln!(lock, "setfield {dst} {data} {index}"),
                        IR::Noop                               => writeln!(lock, "noop"),
                    };
                }
            
                let _ = write!(lock, "    ");
                let _ =  match block.ending {
                    BlockTerminator::Goto(v) => writeln!(lock, "goto {v}"),
                    BlockTerminator::SwitchBool { cond, op1, op2 } => writeln!(lock, "switch-bool {cond} {op1} {op2}"),
                    BlockTerminator::Return => writeln!(lock, "return"),
                };
            
                let _ = writeln!(lock);
            }
        }
        lock
    }


    pub fn sort(&mut self) {
        self.functions.par_iter_mut().for_each(|x| x.blocks.sort_by_key(|x| x.block_index.0));
        self.functions.sort_by_key(|x| x.function_index.0);
    }


    pub fn find_function(&mut self, symbol: SymbolIndex) -> &FunctionLookup {
        match self.files.get(&self.current_file).unwrap().get(&symbol) {
            Some(v) => v,
            None => {
                let (mut root, root_excluded) = self.symbol_table.find_root(symbol);
                let mut root_excluded = root_excluded.unwrap();

                loop {
                    match self.files.get(&root).unwrap().get(&root_excluded) {
                        Some(v) => return v,
                        None => {
                            println!("{} {}", self.symbol_table.get(root), self.symbol_table.get(root_excluded));
                            let (root_t, root_excluded_t) = self.symbol_table.find_root(root_excluded);
                            root = root_t;
                            root_excluded = root_excluded_t.unwrap();
                        },
                    }
                    
                }
            },
        }
        
    }
}


impl ConversionState {
    fn function(&mut self) -> FunctionIndex {
        self.function_counter += 1;
        FunctionIndex(self.function_counter - 1)
    }

    fn extern_function(&mut self) -> u32 {
        self.extern_counter += 1;
        self.extern_counter - 1
    }
}


impl Function {
    fn new(index: FunctionIndex, argument_count: usize) -> Self {
        Self {
            function_index: index,
            variable_lookup: vec![],
            variable_counter: 0,
            stack_size: 0,
            block_counter: 0,
            breaks: vec![],
            continues: vec![],
            blocks: vec![],
            entry: BlockIndex(0),
            argument_count,
            explicit_ret: vec![], 
        }
    }
    
    pub fn generate(&mut self, state: &mut ConversionState, instructions: Vec<Instruction>) -> Variable {
        self.convert_block(state, instructions).2
    }


    fn generate_and_write_to(&mut self, state: &mut ConversionState, instructions: Vec<Instruction>, return_val: Variable) {
        self.blocks.clear();
        
        let start_index = self.block();
        self.entry = start_index;
        
        let mut block = Block { block_index: start_index, instructions: vec![], ending: BlockTerminator::Return };
        
        let mut final_value = return_val;

        if self.evaluate(state, &mut block, instructions, &mut final_value) {
            block.ir(IR::Copy { src: final_value, dst: Variable(0) });
        } else {
            block.ir(IR::Copy { src: final_value, dst: return_val });
        }

        self.blocks.push(block);
        
    }


    fn evaluate(&mut self, state: &mut ConversionState, block: &mut Block, instructions: Vec<Instruction>, final_value: &mut Variable) -> bool {
        state.declaration_process(state.current_file, &instructions);

        for instruction in instructions {
            let statement = matches!(instruction.instruction_kind, InstructionKind::Statement(_) | InstructionKind::Declaration(_));

            if let InstructionKind::Statement(Statement::Return(e)) = instruction.instruction_kind {
                let val = self.convert(state, block, *e);
                *final_value = val;
                
                self.explicit_ret.push(block.block_index);
                return true
            }
            
            let variable = self.convert(state, block, instruction);

            *final_value = if statement { *final_value } else { variable }
        }
        false
    }

}


impl ConversionState {
    fn declaration_process(&mut self, file: SymbolIndex, instructions: &Vec<Instruction>) {
        let mut files = std::mem::take(&mut self.files);
        let file = files.get_mut(&file).unwrap();
        for instruction in instructions.iter() {
            match &instruction.instruction_kind {
                InstructionKind::Declaration(d) => {
                    match d {
                        Declaration::FunctionDeclaration { name, arguments, .. } => {
                            if file.contains_key(name) {
                                    continue
                                }
                            let function = Function::new(self.function(), arguments.len());
                            file.insert(*name, FunctionLookup::Normal(function.function_index));

                            self.functions.push(function);
                        },
                        Declaration::StructDeclaration { .. } => (),
                        Declaration::Namespace { .. } => (),
                        Declaration::Extern { functions, .. } => {
                            for f in functions {
                                if file.contains_key(&f.identifier) {
                                    continue
                                }

                                let t = self.extern_function();
                                file.insert(f.identifier, FunctionLookup::Extern(t));
                            }
                        },
                        Declaration::UseFile { file_name } => (),
                    }
                },
                _ => continue,
            }
        }

        self.files = files;
    }
        
}


impl Function {
    fn convert_block(&mut self, state: &mut ConversionState, instructions: Vec<Instruction>) -> (BlockIndex, BlockIndex, Variable) {
        let start_index = self.block();
        let mut block = Block { block_index: start_index, instructions: vec![], ending: BlockTerminator::Return };

        let return_val = self.variable();

        let lookup_len = self.variable_lookup.len();
        let var_count = self.variable_counter;
        let mut final_value = return_val;
        

        if self.evaluate(state, &mut block, instructions, &mut final_value) {
            block.ir(IR::Copy { src: final_value, dst: Variable(0) });
        } else {
            block.ir(IR::Copy { src: final_value, dst: return_val });
        }


        let index = block.block_index;
        self.blocks.push(block);
        self.variable_counter = var_count;
        self.variable_lookup.resize_with(lookup_len, || panic!());
        
        (start_index, index, return_val)
    }


    fn convert(&mut self, state: &mut ConversionState, block: &mut Block, instruction: Instruction) -> Variable {
        match instruction.instruction_kind {
            InstructionKind::Statement(s)  => {
                self.statement(state, block, s);
                Variable(u32::MAX)
            },
            InstructionKind::Expression(e) => self.expression(state, block, e),
            InstructionKind::Declaration(d) => {
                self.declaration(state, d);
                Variable(u32::MAX)
            },
        }
    }


    fn declaration(&mut self, state: &mut ConversionState, declaration: Declaration) {
        match declaration {
             Declaration::FunctionDeclaration { arguments, body, name, .. } => {
                let function_index = *match state.files.get(&state.current_file).unwrap().get(&name).unwrap() {
                    FunctionLookup::Normal(v) => v,
                    FunctionLookup::Extern(_) => unreachable!(),
                };

                
                let mut function = Function::new(function_index, arguments.len());

                let return_addrs = function.variable();
                
                for argument in arguments {
                    let var = function.variable();
                    function.variable_lookup.push((argument.0, var))
                }
                
                function.generate_and_write_to(state, body, return_addrs);

                for i in function.explicit_ret.clone() {
                    let block = function.find_block_mut(i);
                    block.ending = BlockTerminator::Return;
                }

                *state.functions.iter_mut().find(|x| x.function_index == function_index).unwrap() = function;
            },

            
            Declaration::Namespace { body, .. } => {
                self.convert_block(state, body);
            },

            
            Declaration::StructDeclaration { .. } => (),
            
            
            Declaration::Extern { file, functions } => {
                state.externs.push(ExternFunction { path: file, functions: functions.into_iter().map(|x| x.raw_name).collect() })
            },

            
            Declaration::UseFile { file_name } => (),
        }
    }


    fn statement(&mut self, state: &mut ConversionState, block: &mut Block, statement: Statement) {
        match statement {
            Statement::DeclareVar { identifier, data, ..} => {
                let variable = self.convert(state, block, Instruction { source_range: data.source_range, instruction_kind: InstructionKind::Expression(Expression::Block { body: vec![*data] }) } );
                self.variable_lookup.push((identifier, variable));
                block.ir(IR::Noop);
            },

            
            Statement::VariableUpdate { left, right } => {
                let left_variable = self.convert(state, block, *left);
                let right_variable = self.convert(state, block, *right);

                block.ir(IR::Copy { src: right_variable, dst: left_variable });
                block.ir(IR::Noop);
            },


            Statement::FieldUpdate { structure, right, index_to, .. } => {
                let dst = self.convert(state, block, *structure);
                let data = self.convert(state, block, *right);

                block.ir(IR::SetField { dst, data, index: index_to as u8 })
            },

            
            Statement::Loop { body } => {
                let body_block = self.convert_block(state, body);
                self.find_block_mut(body_block.1).ending = BlockTerminator::Goto(body_block.0);
                
                let mut continue_block = Block { block_index: self.block(), instructions: vec![], ending: BlockTerminator::Return};
                continue_block.ending = replace(&mut block.ending, BlockTerminator::Goto(body_block.0));
                self.blocks.push(replace(block, continue_block));

                for break_block in std::mem::take(&mut self.breaks) {
                    self.find_block_mut(break_block).ending = BlockTerminator::Goto(block.block_index);
                }

                for continue_block in std::mem::take(&mut self.continues) {
                    self.find_block_mut(continue_block).ending = BlockTerminator::Goto(body_block.0);
                }
                
                
            },

            
            Statement::Break => {
                self.breaks.push(block.block_index);

                let mut continue_block = Block { block_index: self.block(), instructions: vec![], ending: BlockTerminator::Return};
                continue_block.ending = replace(&mut block.ending, BlockTerminator::Goto(BlockIndex(u32::MAX))); // placeholder terminator
                self.blocks.push(replace(block, continue_block)); 
            },

            
            Statement::Continue => {
                self.continues.push(block.block_index);

                let mut continue_block = Block { block_index: self.block(), instructions: vec![], ending: BlockTerminator::Return };
                continue_block.ending = replace(&mut block.ending, BlockTerminator::Goto(BlockIndex(u32::MAX))); // placeholder terminator
                self.blocks.push(replace(block, continue_block));   
                
            },
            
            Statement::Return(_) => panic!("returns should be handled when evaluating the block"),
        }
    }
    
    
    fn expression(&mut self, state: &mut ConversionState, block: &mut Block, expression: Expression) -> Variable {
        match expression {
            Expression::Data(data) => {
                let variable = self.variable();
                if matches!(data.data, Data::Empty) {
                    block.ir(IR::Unit { dst: variable });
                    return variable
                }

                block.ir(IR::Load { dst: variable, data: state.constants.len() as u32 });
                state.constants.push(data.data);
                variable
            },

            
            Expression::BinaryOp { operator, left, right } => {
                let left_var = self.convert(state, block, *left);
                let right_var = self.convert(state, block, *right);
                let dst = self.variable();

                
                match operator {
                    BinaryOperator::Add           => block.ir(IR::Add           { dst, left: left_var, right: right_var }),
                    BinaryOperator::Subtract      => block.ir(IR::Subtract      { dst, left: left_var, right: right_var }),
                    BinaryOperator::Multiply      => block.ir(IR::Multiply      { dst, left: left_var, right: right_var }),
                    BinaryOperator::Divide        => block.ir(IR::Divide        { dst, left: left_var, right: right_var }),
                    BinaryOperator::Equals        => block.ir(IR::Equals        { dst, left: left_var, right: right_var }),
                    BinaryOperator::NotEquals     => block.ir(IR::NotEquals     { dst, left: left_var, right: right_var }),
                    BinaryOperator::GreaterThan   => block.ir(IR::GreaterThan   { dst, left: left_var, right: right_var }),
                    BinaryOperator::LesserThan    => block.ir(IR::LesserThan    { dst, left: left_var, right: right_var }),
                    BinaryOperator::GreaterEquals => block.ir(IR::GreaterEquals { dst, left: left_var, right: right_var }),
                    BinaryOperator::LesserEquals  => block.ir(IR::LesserEquals  { dst, left: left_var, right: right_var }),
                };

                dst
            },
            
            
            Expression::Block { body } => {
                let body = self.convert_block(state, body);
                let mut continue_block = Block { block_index: self.block(), instructions: vec![], ending: BlockTerminator::Return };
                
                continue_block.ending = replace(&mut block.ending, BlockTerminator::Goto(body.0));
                self.find_block_mut(body.1).ending = BlockTerminator::Goto(continue_block.block_index);
                
                self.blocks.push(replace(block, continue_block));

                body.2
            },

            
            Expression::IfExpression { body, condition, else_part } => {
                let condition = self.convert(state, block, *condition);

                let body_block_index = self.convert_block(state, body);
                
                let mut continue_block = Block { block_index: self.block(), instructions: vec![], ending: BlockTerminator::Return };
                
                let switch = BlockTerminator::SwitchBool {
                    cond: condition,
                    op1: body_block_index.0, 
                    op2: match else_part {
                        Some(else_part) => {
                            let else_body_index = self.convert_block(state, vec![*else_part]);
                            let else_body = self.find_block_mut(else_body_index.1);
                            
                            else_body.ending = BlockTerminator::Goto(continue_block.block_index);

                            // Copy the result of the block
                            else_body.ir(IR::Copy { src: else_body_index.2, dst: body_block_index.2 });
                        
                            else_body_index.0
                        },
                        None => continue_block.block_index,
                    }
                };                

                continue_block.ending = replace(&mut block.ending, switch);
                self.find_block_mut(body_block_index.1).ending = BlockTerminator::Goto(continue_block.block_index);
                self.blocks.push(replace(block, continue_block));

                body_block_index.2
            },

            
            Expression::Identifier(v) => self.variable_lookup.iter().rev().find(|x| x.0 == v).unwrap().1,

            
            Expression::FunctionCall { identifier, arguments } => {
                let dst = self.variable();
                let mut variables = Vec::with_capacity(arguments.len());

                for argument in arguments.into_iter() {
                    let argument_reg = self.convert(state, block, argument);
                    variables.push(argument_reg);
                }

                let function_lookup = state.find_function(identifier);
                
                match function_lookup {
                    FunctionLookup::Normal(v) => block.ir(IR::Call    { dst, id: *v, args: variables }),
                    FunctionLookup::Extern(v) => block.ir(IR::ExtCall { dst, index: *v, args: variables }),
                }


                dst
            },

            
            Expression::StructureCreation { fields, .. } => {
                let dst = self.variable();

                if fields.is_empty() {
                    block.ir(IR::Unit { dst });
                    return dst;
                }

                let mut variables = Vec::with_capacity(fields.len());
                for argument in fields.into_iter() {
                    let argument_reg = self.convert(state, block, argument.1);
                    variables.push(argument_reg);
                }


                block.ir(IR::Struct { dst, fields: variables });

                
                dst
            },

            
            Expression::AccessStructureData { structure, index_to, .. } => {
                let struct_at = self.convert(state, block, *structure);
                let dst = self.variable();
                
                block.ir(IR::AccStruct { dst, val: struct_at, index: index_to as u8 });

                dst
            },

            
            Expression::WithinNamespace { do_within, .. } => {
                self.convert(state, block, *do_within)
            },
        }
    }

    
    fn variable(&mut self) -> Variable {
        self.variable_counter += 1;

        if self.stack_size < self.variable_counter {
            self.stack_size = self.variable_counter;
        }

        Variable(self.variable_counter - 1)
    }


    fn block(&mut self) -> BlockIndex {
        self.block_counter += 1;

        BlockIndex(self.block_counter - 1)
    }


    fn find_block_mut(&mut self, index: BlockIndex) -> &mut Block {
        self.blocks.iter_mut().find(|x| x.block_index == index).unwrap()
    }

    fn find_block(&self, index: BlockIndex) -> &Block {
        self.blocks.iter().find(|x| x.block_index == index).unwrap()
    }

}

impl Block {
    fn ir(&mut self, ir: IR) {
        self.instructions.push(ir);
    }
}