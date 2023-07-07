pub mod optimizations;

use std::{mem::replace, fmt::{Display, Write}, collections::{BTreeMap, BTreeSet, HashMap}};

use azurite_parser::ast::{Instruction, Expression, BinaryOperator, Statement, InstructionKind, Declaration, UnaryOperator};
use common::{Data, SymbolIndex, SymbolTable};
use rayon::prelude::{ParallelIterator, IntoParallelRefMutIterator};

#[derive(Debug, PartialEq)]
pub struct ConversionState {
    pub constants: Vec<Data>,

    pub extern_functions: BTreeMap<SymbolIndex, ExternFunction>,
    pub functions: BTreeMap<SymbolIndex, Function>,
    
    function_counter: u32,
    extern_counter: u32,
    
    pub symbol_table: SymbolTable,
}


#[derive(Debug, PartialEq)]
pub struct Function {
    pub identifier: SymbolIndex,
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
    pub identifier: SymbolIndex,
    pub function_index: FunctionIndex,
    pub file: SymbolIndex,
    pub path: SymbolIndex,
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
    Modulo        { dst: Variable, left: Variable, right: Variable },
    Equals        { dst: Variable, left: Variable, right: Variable },
    NotEquals     { dst: Variable, left: Variable, right: Variable },
    GreaterThan   { dst: Variable, left: Variable, right: Variable },
    LesserThan    { dst: Variable, left: Variable, right: Variable },
    GreaterEquals { dst: Variable, left: Variable, right: Variable },
    LesserEquals  { dst: Variable, left: Variable, right: Variable },

    UnaryNot      { dst: Variable, val:  Variable },
    UnaryNeg      { dst: Variable, val:  Variable },

    Call          { dst: Variable, id: FunctionIndex,  args: Vec<Variable> },
    ExtCall       { dst: Variable, id: FunctionIndex,  args: Vec<Variable> },
    
    Struct        { dst: Variable, fields: Vec<Variable> },
    AccStruct     { dst: Variable, val: Variable, index: u8 },
    SetField      { dst: Variable, data: Variable, index: u8},


    CastToI8      { dst: Variable, val: Variable },
    CastToI16     { dst: Variable, val: Variable },
    CastToI32     { dst: Variable, val: Variable },
    CastToI64     { dst: Variable, val: Variable },

    CastToU8      { dst: Variable, val: Variable },
    CastToU16     { dst: Variable, val: Variable },
    CastToU32     { dst: Variable, val: Variable },
    CastToU64     { dst: Variable, val: Variable },

    CastToFloat   { dst: Variable, val: Variable },
    

    Noop,
}


pub enum Result {
    Variable(Variable),
}


impl ConversionState {
    pub fn new(symbol_table: SymbolTable) -> Self { 
        Self {
            constants: vec![],
            symbol_table,
            function_counter: 0,
            functions: BTreeMap::new(),
            extern_counter: 0,
            extern_functions: BTreeMap::new(),

        }
    }


    pub fn generate(&mut self, root_index: SymbolIndex, mut files: Vec<(SymbolIndex, Vec<Instruction>)>, templates: Vec<Instruction>) {
        files.sort_by_key(|x| x.0);
        let init_function = self.symbol_table.add(String::from("::init"));
        let mut function = Function::new(init_function, self.function(), 0);

        for file in files.iter() {
            let function = Function::new(file.0, self.function(), 0);
            self.functions.insert(file.0, function);
            self.declaration_process(&file.1);
        }
        
        
        for t in &templates {
            assert!(matches!(t.instruction_kind, InstructionKind::Declaration(Declaration::FunctionDeclaration { .. })));
        }

        function.generate(self, templates);
        function.blocks.clear();
        function.block_counter = 0;

        for file in files {
            let function = self.functions.get(&file.0).unwrap().function_index;
            let mut function = Function::new(file.0, function, 0);

            function.generate(self, file.1);
            let result = self.functions.insert(file.0, function);
            assert!(result.is_some());
        }



        let vec = Vec::from([IR::Call { dst: Variable(0), id: self.find_function(root_index).function_index, args: vec![] }]);
        let block = Block { block_index: function.block(), instructions: vec, ending: BlockTerminator::Return };
        function.blocks.push(block);

        self.functions.insert(init_function, function);

        assert_eq!(self.functions.len(), self.function_counter as usize);
    }


    pub fn pretty_print(&mut self) -> String {
        let mut lock = String::new();
        for function in self.functions.values() {
            function.pretty_print(self, &mut lock);
        }
        lock
    }


    pub fn sort(&mut self) {
        self.functions.par_iter_mut().for_each(|x| x.1.blocks.sort_by_key(|x| x.block_index.0));
    }


    /// PANICS: If the function is an extern function this will panic
    pub fn find_function(&mut self, symbol: SymbolIndex) -> &Function {
        self.functions.get(&symbol).unwrap()
    }


    pub fn take_out_externs(&mut self) -> (BTreeMap<SymbolIndex, BTreeSet<(SymbolIndex, u32)>>, u32) {
        let mut used_externs = HashMap::new();
        let mut extern_counter = 0;
        for f in &mut self.functions {
            for b in &mut f.1.blocks {
                for i in &mut b.instructions {
                    if let IR::ExtCall { id, .. } = i {
                        if let Some(v) = used_externs.get(id) {
                            *id = *v;
                            continue
                        }

                        used_externs.insert(*id, FunctionIndex(extern_counter));
                        *id = FunctionIndex(extern_counter);

                        extern_counter += 1;
                    }
                }
            }
        }


        let iter = self.extern_functions
            .iter_mut()
            .filter_map(|x| if let Some(v) = used_externs.get(&x.1.function_index) {
                x.1.function_index = *v;
                Some(x.1)
            } else { None })
            .collect::<Vec<_>>();
        
        
        let (externs, extern_counter) = {
            let mut map = BTreeMap::new();
            let mut max = 0;

            for e in iter {
                if e.function_index.0 > max {
                    max = e.function_index.0;
                }

                map.entry(e.file).or_insert_with(BTreeSet::new);
                map.get_mut(&e.file).unwrap().insert((e.path, e.function_index.0));
            }

            (map, max)
        };
        
        

        (externs, extern_counter)
    }
}


impl ConversionState {
    fn function(&mut self) -> FunctionIndex {
        self.function_counter += 1;
        FunctionIndex(self.function_counter - 1)
    }

    fn extern_function(&mut self) -> FunctionIndex {
        self.extern_counter += 1;
        FunctionIndex(self.extern_counter - 1)
    }
}


impl Function {
    fn new(identifier: SymbolIndex, index: FunctionIndex, argument_count: usize) -> Self {
        Self {
            identifier,
            function_index: index,
            variable_lookup: vec![],
            variable_counter: 0,
            stack_size: argument_count as u32,
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
        state.declaration_process(&instructions);

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
    fn declaration_process(&mut self, instructions: &[Instruction]) {
        for instruction in instructions.iter() {
            match &instruction.instruction_kind {
                InstructionKind::Declaration(d) => {
                    match d {
                        Declaration::FunctionDeclaration { name, arguments, generics, .. } => {
                            if self.functions.contains_key(name) {
                                continue
                            }

                            if !generics.is_empty() {
                                continue
                            }

                           
                            let function = Function::new(*name, self.function(), arguments.len());
                            self.functions.insert(*name, function);
                        },
                        Declaration::StructDeclaration { .. } => (),
                        Declaration::Namespace { .. } => (),
                        Declaration::Extern { functions, file  } => {
                            for f in functions {
                                if self.extern_functions.contains_key(&f.identifier) {
                                    continue
                                }
                                
                                let t = self.extern_function();
                                self.extern_functions.insert(f.identifier, ExternFunction { identifier: f.identifier, function_index: t, file: *file, path: f.raw_name });
                            }
                        },
                        Declaration::UseFile { .. } => (),
                        Declaration::ImplBlock { body, .. } => {
                            self.declaration_process(body);
                        },
                    }
                },
                _ => continue,
            }
        }
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
                self.declaration(state, block, d);
                Variable(u32::MAX)
            },
        }
    }


    fn declaration(&mut self, state: &mut ConversionState, block: &mut Block, declaration: Declaration) {
        match declaration {
            Declaration::FunctionDeclaration { arguments, body, name, generics, .. } => {
                if !generics.is_empty() {
                    return
                }
                
                let function_index = state.find_function(name).function_index;

                
                let mut function = Function::new(name, function_index, arguments.len());

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

                let t = state.functions.insert(name, function);
                assert!(t.is_some());
            },

            
            Declaration::Namespace { body, .. } => {
                self.convert_block(state, body);
            },

            
            Declaration::StructDeclaration { .. } => (),
            
            
            Declaration::Extern { .. } => (),

            
            Declaration::UseFile { file_name } => {
                block.ir(IR::Call { dst: self.variable(), id: state.find_function(file_name).function_index, args: vec![] })
            },

            
            Declaration::ImplBlock { body, .. } => {
                self.convert_block(state, body);
            },
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


            Expression::AsCast { value, cast_type } => {
                let dst = self.variable();
                let val = self.convert(state, block, *value);

                match cast_type.data_type {
                    common::DataType::I8    => block.ir(IR::CastToI8 { dst, val } ),
                    common::DataType::I16   => block.ir(IR::CastToI16 { dst, val } ),
                    common::DataType::I32   => block.ir(IR::CastToI32 { dst, val } ),
                    common::DataType::I64   => block.ir(IR::CastToI64 { dst, val } ),
                    common::DataType::U8    => block.ir(IR::CastToU8 { dst, val } ),
                    common::DataType::U16   => block.ir(IR::CastToU16 { dst, val } ),
                    common::DataType::U32   => block.ir(IR::CastToU32 { dst, val } ),
                    common::DataType::U64   => block.ir(IR::CastToU64 { dst, val } ),
                    common::DataType::Float => block.ir(IR::CastToFloat { dst, val } ),

                    _ => unreachable!()
                };

                dst
            }

            
            Expression::BinaryOp { operator, left, right } => {
                let left_var = self.convert(state, block, *left);
                let right_var = self.convert(state, block, *right);
                let dst = self.variable();

                
                match operator {
                    BinaryOperator::Add           => block.ir(IR::Add           { dst, left: left_var, right: right_var }),
                    BinaryOperator::Subtract      => block.ir(IR::Subtract      { dst, left: left_var, right: right_var }),
                    BinaryOperator::Multiply      => block.ir(IR::Multiply      { dst, left: left_var, right: right_var }),
                    BinaryOperator::Divide        => block.ir(IR::Divide        { dst, left: left_var, right: right_var }),
                    BinaryOperator::Modulo        => block.ir(IR::Modulo        { dst, left: left_var, right: right_var }),
                    BinaryOperator::Equals        => block.ir(IR::Equals        { dst, left: left_var, right: right_var }),
                    BinaryOperator::NotEquals     => block.ir(IR::NotEquals     { dst, left: left_var, right: right_var }),
                    BinaryOperator::GreaterThan   => block.ir(IR::GreaterThan   { dst, left: left_var, right: right_var }),
                    BinaryOperator::LesserThan    => block.ir(IR::LesserThan    { dst, left: left_var, right: right_var }),
                    BinaryOperator::GreaterEquals => block.ir(IR::GreaterEquals { dst, left: left_var, right: right_var }),
                    BinaryOperator::LesserEquals  => block.ir(IR::LesserEquals  { dst, left: left_var, right: right_var }),
                };

                dst
            },
            
            
            Expression::UnaryOp { operator, value } => {
                let val = self.convert(state, block, *value);
                let dst = self.variable();

                match operator {
                    UnaryOperator::Not => block.ir(IR::UnaryNot { dst, val }),
                    UnaryOperator::Negate => block.ir(IR::UnaryNeg { dst, val }),
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

            
            Expression::FunctionCall { identifier, arguments, created_by_accessing: _, generics: _ } => {
                let dst = self.variable();
                let mut variables = Vec::with_capacity(arguments.len());

                for argument in arguments.into_iter() {
                    let argument_reg = self.convert(state, block, argument);
                    variables.push(argument_reg);
                }

                if let Some(v) = state.functions.get(&identifier) {
                    block.ir(IR::Call    { dst, id: v.function_index, args: variables })
                } else if let Some(v) = state.extern_functions.get(&identifier) {
                    block.ir(IR::ExtCall { dst, id: v.function_index, args: variables })
                } else { 
                    panic!("huh?")
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


impl Function {
    pub fn pretty_print(&self, state: &ConversionState, lock: &mut impl Write) {
        let _ = writeln!(lock, "fn {} ({})", self.function_index, state.symbol_table.get(&self.identifier));
        for block in &self.blocks {
            let _ = writeln!(lock, "  bb{}:", block.block_index.0);
            for ir in &block.instructions {
                if matches!(ir, IR::Noop) {
                    continue
                }

                let _ = write!(lock, "    ");
                let _ = match ir {
                    IR::Load { dst, data }                 => writeln!(lock, "load {dst} {}", state.constants[*data as usize].to_string(&state.symbol_table)),
                    IR::Add { dst, left, right }           => writeln!(lock, "add {dst} {left} {right}"),
                    IR::Subtract { dst, left, right }      => writeln!(lock, "sub {dst} {left} {right}"),
                    IR::Multiply { dst, left, right }      => writeln!(lock, "mul {dst} {left} {right}"),
                    IR::Divide { dst, left, right }        => writeln!(lock, "div {dst} {left} {right}"),
                    IR::Modulo { dst, left, right }        => writeln!(lock, "mod {dst} {left} {right}"),
                    IR::Copy { src, dst }                  => writeln!(lock, "copy {src} {dst}"),
                    IR::Swap { v1, v2 }                    => writeln!(lock, "swap {v1} {v2}"),
                    IR::Equals { dst, left, right }        => writeln!(lock, "eq {dst} {left} {right}"),
                    IR::NotEquals { dst, left, right }     => writeln!(lock, "neq {dst} {left} {right}"),
                    IR::GreaterThan { dst, left, right }   => writeln!(lock, "gt {dst} {left} {right}"),
                    IR::LesserThan { dst, left, right }    => writeln!(lock, "lt {dst} {left} {right}"),
                    IR::GreaterEquals { dst, left, right } => writeln!(lock, "ge {dst} {left} {right}"),
                    IR::LesserEquals { dst, left, right }  => writeln!(lock, "le {dst} {left} {right}"),
                    IR::Call { id, dst, args }             => writeln!(lock, "call {id} {dst} ({} )", args.iter().map(|x| format!(" {x}")).collect::<String>()),
                    IR::ExtCall { id: index, dst, args }       => writeln!(lock, "ecall {index} {dst} ({} )", args.iter().map(|x| format!(" {x}")).collect::<String>()),
                    IR::Unit { dst }                       => writeln!(lock, "unit {dst}"),
                    IR::Struct { dst, fields }             => writeln!(lock, "struct {dst} ({} )", fields.iter().map(|x| format!(" {x}")).collect::<String>()),
                    IR::AccStruct { dst, val, index }      => writeln!(lock, "accstruct, {dst} {val} {index}"),
                    IR::SetField { dst, data, index }      => writeln!(lock, "setfield {dst} {data} {index}"),
                    IR::Noop                               => continue,
                    IR::UnaryNot { dst, val }              => writeln!(lock, "not {dst} {val}"),
                    IR::UnaryNeg { dst, val }              => writeln!(lock, "neg {dst} {val}"),
                    
                    IR::CastToI8 { dst, val }  => writeln!(lock, "castI8 {dst} {val}"),
                    IR::CastToI16 { dst, val } => writeln!(lock, "castI16 {dst} {val}"),
                    IR::CastToI32 { dst, val } => writeln!(lock, "castI32 {dst} {val}"),
                    IR::CastToI64 { dst, val } => writeln!(lock, "castI64 {dst} {val}"),
                    IR::CastToU8 { dst, val }  => writeln!(lock, "castU8 {dst} {val}"),
                    IR::CastToU16 { dst, val } => writeln!(lock, "castU16 {dst} {val}"),
                    IR::CastToU32 { dst, val } => writeln!(lock, "castU32 {dst} {val}"),
                    IR::CastToU64 { dst, val } => writeln!(lock, "castU64 {dst} {val}"),
                    IR::CastToFloat { dst, val } => writeln!(lock, "castfloat {dst} {val}"),
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
}

impl Block {
    fn ir(&mut self, ir: IR) {
        self.instructions.push(ir);
    }
}