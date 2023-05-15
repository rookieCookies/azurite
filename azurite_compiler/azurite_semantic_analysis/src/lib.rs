#![feature(iter_intersperse)]
pub mod variable_stack;

use hashbrown::HashMap;

use azurite_errors::{SourceRange, Error, CompilerError, ErrorBuilder, CombineIntoError};
use azurite_parser::ast::{Instruction, InstructionKind, Statement, Expression, BinaryOperator, Declaration};
use common::{DataType, SymbolTable, SymbolIndex, SourcedDataType};
use variable_stack::VariableStack;

pub struct GlobalState<'a> {
    symbol_table: &'a SymbolTable,
}

#[derive(Debug)]
pub struct AnalysisState {
    pub variable_stack: VariableStack,
    loop_depth: usize,

    functions: HashMap<SymbolIndex, (Function, usize)>,
    structures: HashMap<SymbolIndex, (Structure, usize)>,
    
    explicit_return: Option<SourcedDataType>,

    depth: usize,
}


#[derive(Debug)]
struct Function {
    return_type: SourcedDataType,
    arguments: Vec<SourcedDataType>,
}


#[derive(Debug)]
struct Structure {
    fields: Vec<(SymbolIndex, SourcedDataType)>,
}


impl<'a> GlobalState<'a> {
    pub fn new(symbol_table: &'a SymbolTable) -> Self { Self { symbol_table } }
}


impl AnalysisState {
    pub fn new() -> Self {
        Self {
            variable_stack: VariableStack::new(),
            loop_depth: 0,
            depth: 0,
            explicit_return: None,
            functions: HashMap::new(),
            structures: HashMap::new(),

        }
    }

    pub fn start_analysis(&mut self, global: &mut GlobalState, instructions: &mut [Instruction]) -> Result<(), Error> {
        self.analyze_block(global, instructions, true)?;

        Ok(())
    }
}


impl AnalysisState {
    fn analyze(&mut self, global: &mut GlobalState, instruction: &mut Instruction) -> Result<SourcedDataType, Error> {
        match &mut instruction.instruction_kind {
            InstructionKind::Statement(s) => {
                self.analyze_statement(global, s, &instruction.source_range)?;
            },
            
            
            InstructionKind::Expression(e) => return self.analyze_expression(global, e, &instruction.source_range),
            
            
            InstructionKind::Declaration(d) => {
                self.analyze_declaration(global, d, &instruction.source_range)?;
            },
        };

        Ok(SourcedDataType::new(instruction.source_range, DataType::Empty))
    }
    

    fn analyze_block(&mut self, global: &mut GlobalState, instructions: &mut [Instruction], reset: bool) -> Result<SourcedDataType, Error> {
        let top = self.variable_stack.len();

        if reset {
            self.depth += 1;
            
        }
        // Declarations
        {
            instructions.iter().for_each(|x| {
                if let InstructionKind::Declaration(d) = &x.instruction_kind {
                    self.declaration_early_process(d)
                }
            })
        }
        
        // dbg!(&self, &global.symbol_table);

        
        let mut errors = vec![];
        let return_val = instructions.iter_mut().map(|x| match self.analyze(global, x) {
            Ok(r) => r,
            Err(e) => {
                errors.push(e);
                SourcedDataType::new(x.source_range, DataType::Any)
            },
        }).last().unwrap_or(SourcedDataType::new(SourceRange::new(0, 0), DataType::Empty));

        self.variable_stack.pop(self.variable_stack.len() - top);

        if reset {
            self.functions.retain(|_, y| self.depth > y.1);
            self.structures.retain(|_, y| self.depth > y.1);
            self.depth -= 1;
        }
        

        if errors.is_empty() {
            Ok(return_val)
        } else {
            Err(errors.combine_into_error())
        }
    }


    fn declaration_early_process(&mut self, declaration: &Declaration) {
        match declaration {
            Declaration::FunctionDeclaration { name, arguments, return_type, .. } => {
                self.declare_function(*name, Function {
                    return_type: *return_type,
                    arguments: arguments.iter().map(|x| x.1).collect(),
                });
            },

            
            Declaration::StructDeclaration { name, fields } => {
                self.declare_struct(*name, Structure {
                    fields: fields.clone(),
                })
            },

            
            Declaration::Namespace { body, .. } => {
                for i in body.iter() {
                    if let InstructionKind::Declaration(d) = &i.instruction_kind {
                        self.declaration_early_process(d)
                    }
                }

            },

            
            Declaration::Extern { functions, .. } => {
                for f in functions {
                    self.declare_function(f.0, Function {
                        return_type: f.1,
                        arguments: f.2.clone(),
                    })
                }
            },
        }
    }
    

    fn analyze_declaration(&mut self, global: &mut GlobalState, declaration: &mut Declaration, source_range: &SourceRange) -> Result<(), Error> {
        match declaration {
            Declaration::FunctionDeclaration { arguments, return_type, body, source_range_declaration, name } => {
                println!("MHM {}", global.symbol_table.get(*name));
                
                let mut analysis_state = AnalysisState::new();

                analysis_state.functions = std::mem::take(&mut self.functions);
                analysis_state.structures = std::mem::take(&mut self.structures);
                
                analysis_state.depth = self.depth;
                analysis_state.explicit_return = Some(*return_type);

                {

                    let mut errors = vec![];
                    
                    for argument in arguments.iter() {
                        if let Err(e) = analysis_state.is_valid_type(global, argument.1) {
                            errors.push(e)
                        }

                        analysis_state.variable_stack.push(argument.0, argument.1);
                    }

                    if !errors.is_empty() {
                        self.functions = std::mem::take(&mut analysis_state.functions);
                        self.structures = std::mem::take(&mut analysis_state.structures);

                        return Err(errors.combine_into_error())
                    }

                }


                let body_return_type = analysis_state.analyze_block(global, body, true)?;

                self.functions = std::mem::take(&mut analysis_state.functions);
                self.structures = std::mem::take(&mut analysis_state.structures);

                if !self.is_of_type(global, *return_type, body_return_type)? {
                    return Err(CompilerError::new(211, "function body returns a different type")
                        .highlight(*source_range_declaration)
                            .note(format!("function returns {}", global.to_string(return_type.data_type)))

                        .empty_line()
                        
                        .highlight(body.last().map_or(SourceRange::new(source_range.start + source_range_declaration.end, source_range.end - source_range_declaration.end), |x| x.source_range))
                            .note(format!("but the body returns {}", global.to_string(body_return_type.data_type)))
                        
                        .build())
                }

                Ok(())
            },


            Declaration::StructDeclaration { fields, name } => {
                println!("HEY {}{:?}", global.symbol_table.get(*name), name);
                let errs = fields
                    .iter()
                    .map(|x| self.is_valid_type(global, x.1))
                    .filter_map(|x| if let Err(x) = x { Some(x) } else { None })
                    .collect::<Vec<_>>();

                if !errs.is_empty() {
                    return Err(errs.combine_into_error())
                }

                Ok(())
            },

            
            Declaration::Namespace { body, .. } => {
                self.analyze_block(global, body, false)?;
                Ok(())
                
            },

            
            Declaration::Extern { functions, .. } => {
                for f in functions {
                    self.is_valid_type(global, f.1)?;

                    for argument in &f.2 {
                        self.is_valid_type(global, *argument)?;
                    }
                }

                Ok(())
            },
        }
    }
    
    
    fn analyze_expression(&mut self, global: &mut GlobalState, expression: &mut Expression, source_range: &SourceRange) -> Result<SourcedDataType, Error> {
        match expression {
            Expression::Data(v) => Ok(SourcedDataType::from(v)),

            
            Expression::BinaryOp { operator, left, right } => {
                let left_type  = self.analyze(global, left)?;
                let right_type = self.analyze(global, right)?;

                let data_type = match *operator {
                    | BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide => {
                        match (left_type.data_type, right_type.data_type) {
                            | (DataType::Any, DataType::Integer)
                            | (DataType::Integer, DataType::Any)
                            | (DataType::Integer, DataType::Integer) => DataType::Integer,

                            | (DataType::Any, DataType::Float)
                            | (DataType::Float, DataType::Any)
                            | (DataType::Float, DataType::Float) => DataType::Float,

                            (DataType::Any, DataType::Any) => DataType::Any,
                            
                             _ => {
                                return Err(CompilerError::new(201, "invalid type arithmetic operation")
                                    .highlight(SourceRange::combine(left.source_range, right.source_range))
                                        .note(format!("left side is of type {} while the right side is of type {}", global.to_string(left_type.data_type), global.to_string(right_type.data_type)))
                                    .build())
                            }
                        }
                    }


                    | BinaryOperator::Equals
                    | BinaryOperator::NotEquals
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::LesserThan
                    | BinaryOperator::GreaterEquals
                    | BinaryOperator::LesserEquals => {
                        if !self.is_of_type(global, left_type, right_type)? {
                            return Err(CompilerError::new(202, "comparisson types differ")
                                .highlight(SourceRange::combine(left.source_range, right.source_range))
                                    .note(format!("left side is of type {} while the right side is of type {}", global.to_string(left_type.data_type), global.to_string(right_type.data_type)))
                                .build())
                        }
            
                        DataType::Bool
                    }
                    
                };

                Ok(SourcedDataType::new(*source_range, data_type))
            },

            
            Expression::Block { body } => {
                self.analyze_block(global, body, true)
            },


            Expression::IfExpression { body, condition, else_part } => {
                let condition_type = self.analyze(global, condition)?;

                if !self.is_of_type(global, condition_type, SourcedDataType::new(SourceRange::new(0, 0), DataType::Bool))? {
                    return Err(CompilerError::new(203, "condition expects a boolean")
                        .highlight(condition.source_range)
                            .note(format!("is of type {}", global.to_string(condition_type.data_type)))
                        .build())
                }


                let body_type = self.analyze_block(global, body, true)?;

                if let Some(else_part) = else_part {
                    let else_type = self.analyze(global, else_part)?;

                    if !self.is_of_type(global, body_type, else_type)? {
                        return Err(CompilerError::new(204, "if expressions branches don't return the same type")
                            .highlight(body.last().map_or(*source_range, |x| x.source_range))
                                .note(format!("is of type {}", global.to_string(body_type.data_type)))
                            
                            .empty_line()
                
                            .highlight(else_part.source_range)
                                .note(format!("but this returns {}", global.to_string(else_type.data_type)))
                            .build())
                    }
                }

                Ok(body_type)
            },


            Expression::Identifier(identifier) => {
                match self.variable_stack.find(*identifier) {
                    Some(v) => Ok(v),
                    None => {
                        Err(CompilerError::new(205, "variable does not exist")
                            .highlight(*source_range)
                            .build()
                        )
                    },
                }
            },


            Expression::FunctionCall { identifier, arguments } => {
                let function = match self.get_function(identifier) {
                    Some(v) => v,
                    None => {
                        // dbg!(&identifier, &global.symbol_table, &self);
                        return Err(CompilerError::new(212, "function isn't declared")
                            .highlight(*source_range)
                                .note(format!("there's no function named {}", global.symbol_table.get(*identifier)))
                            .build())
                    },
                };

                let return_type = function.return_type;
        
                if function.arguments.len() != arguments.len() {
                    return Err(CompilerError::new(214, "invalid number of arguments")
                        .highlight(*source_range)
                            .note(format!("expected {} arguments found {}", function.arguments.len(), arguments.len()))
                        .build())
                }

    
                {

                    let mut errors = vec![];
        
                    for (argument, expected_type) in arguments.iter_mut().zip(function.arguments.clone().iter()) {
                        let argument_type = match self.analyze(global, argument) {
                            Ok(v) => v,
                            Err(e) => {
                                errors.push(e);
                                continue
                            },
                        };


                        let is_of_type = match self.is_of_type(global, *expected_type, argument_type) {
                            Ok(v) => v,
                            Err(e) => {
                                errors.push(e);
                                continue
                            },
                        };

                        if !is_of_type {
                            errors.push(CompilerError::new(213, "argument is of invalid type")
                                .highlight(argument.source_range)
                                    .note(format!("is of type {} while the function expects {}", global.to_string(argument_type.data_type), global.to_string(expected_type.data_type)))

                                .build())
                        }
                    }

                    if !errors.is_empty() {
                        return Err(errors.combine_into_error())
                    }
        
                }

                Ok(return_type)
            },

            
            Expression::StructureCreation { identifier, fields, identifier_range } => {
                let structure = self.get_struct(global, identifier_range, identifier)?;
                
                {
                    let mut hashmap = structure.fields.iter().copied().collect::<HashMap<_, _>>();
                    let mut invalid = vec![];
                    let mut field_errors = vec![];


                    for given_field in fields.iter_mut() {
                        if let Some(v) = hashmap.remove(&given_field.0) {
                            let instruction_type = match self.analyze(global, &mut given_field.1) {
                                Ok(v) => v,
                                Err(e) => {
                                    field_errors.push(e);
                                    continue
                                },
                            };

                            let is_same_type = match self.is_of_type(global, instruction_type, v) {
                                Ok(v) => v,
                                Err(e) => {
                                    field_errors.push(e);
                                    continue
                                },
                            };

                            if !is_same_type {
                                field_errors.push(CompilerError::new(217, "structure field and provided value are not of the same type")
                                    .highlight(v.source_range)
                                        .note(format!("defined here as type {}", global.to_string(v.data_type)))

                                    .empty_line()
                                    
                                    .highlight(given_field.1.source_range)
                                        .note(format!("..but this results in a value of type {}", global.to_string(instruction_type.data_type)))
                                    .build())
                            }
                            
                        } else {
                            invalid.push(given_field.0);
                        }
                    }


                    if !invalid.is_empty() {
                        field_errors.push(CompilerError::new(218, "invalid fields")
                            .highlight(*source_range)
                                .note(format!("invalid: {}", invalid.into_iter().map(|x| global.symbol_table.get(x)).intersperse(", ".to_string()).collect::<String>()))
                            .build())
                    }


                    if !hashmap.is_empty() {
                        field_errors.push(CompilerError::new(219, "missing fields")
                            .highlight(*source_range)
                                .note(format!("missing: {}", hashmap.into_iter().map(|x| global.symbol_table.get(x.0)).intersperse(", ".to_string()).collect::<String>()))
                            .build())
                        
                    }


                    if !field_errors.is_empty() {
                        return Err(field_errors.combine_into_error())
                    }
                }
                

                fields.sort_by_key(|x| x.0);
                Ok(SourcedDataType::new(*source_range, DataType::Struct(*identifier)))
            },

            
            Expression::AccessStructureData { structure, identifier, index_to } => {
                let structure_type = self.analyze(global, structure)?;
                
                match structure_type.data_type {
                    DataType::Struct(v) => {
                        let structure = self.get_struct(global, source_range, &v)?;

                        if let Some(v) = structure.fields.iter().enumerate().find(|x| x.1.0 == *identifier) {
                            *index_to = v.0;
                            return Ok(v.1.1)
                        }
                    },

                    DataType::Any => return Ok(SourcedDataType::new(*source_range, DataType::Any)),
                    _ => ()
                };

                return Err(CompilerError::new(220, "structure field doesn't exist")
                        .highlight(*source_range)
                            .note(format!("is of type {} which doesn't have a field named {}", global.to_string(structure_type.data_type), global.symbol_table.get(*identifier)))
                        .build()
                )
            },

            
            Expression::WithinNamespace { namespace, do_within } => {
                self.analyze(global, do_within)
            },
        }
    }

    
    fn analyze_statement(&mut self, global: &mut GlobalState, statement: &mut Statement, source_range: &SourceRange) -> Result<(), Error> {
        match statement {
            Statement::DeclareVar { identifier, data, type_hint } => {
                let data_type = match self.analyze(global, &mut *data) {
                    Ok(v) => v,
                    Err(e) => {
                        self.variable_stack.push(*identifier, SourcedDataType::new(*source_range, DataType::Any));
                        return Err(e)
                    },
                };
                
                self.variable_stack.push(*identifier, if let Some(v) = type_hint { *v } else { data_type });

                if !type_hint.map_or(Ok(true), |x| self.is_of_type(global, data_type, x))? {
                    return Err(CompilerError::new(210, "value differs from type hint")
                        .highlight(data.source_range)
                            .note(format!("is of type {} but the type hint is {}", global.to_string(data_type.data_type), global.to_string(type_hint.unwrap().data_type)))
                        .build())
                }
                
                Ok(())
            },

            
            Statement::VariableUpdate { left, right } => {
                match &left.instruction_kind {
                    InstructionKind::Expression(Expression::Identifier(v)) => {
                        let value = match self.variable_stack.find(*v) {
                            Some(v) => v,
                            None => {
                                return Err(CompilerError::new(206, "can't update a variable that does not exist")
                                    .highlight(left.source_range)
                                    .build());
                            },
                        };

                        let right_type = self.analyze(global, right)?;

                        if !self.is_of_type(global, right_type, value)? {
                            return Err(CompilerError::new(207, "variable is of different type")
                                .highlight(*source_range)
                                    .note(format!("{} is of type {} but the assigned value is of type {}", global.symbol_table.get(*v), global.to_string(value.data_type), global.to_string(right_type.data_type)))
                                .build())
                        }

                    },
                    _ => unreachable!()
                };

                Ok(())
            },

            
            Statement::Loop { body } => {
                self.loop_depth += 1;

                self.analyze_block(global, body, true)?;

                self.loop_depth -= 1;

                Ok(())
            },
            
            
            Statement::Break => {
                if self.loop_depth == 0 {
                    return Err(CompilerError::new(208, "break outside of loop")
                        .highlight(*source_range)
                        .build())
                }
                Ok(())
            },
            
            
            Statement::Continue => {
                if self.loop_depth == 0 {
                    return Err(CompilerError::new(209, "continue outside of loop")
                        .highlight(*source_range)
                        .build())
                }
                Ok(())
                
            },


            Statement::Return(v) => {
                let datatype = self.analyze(global, v)?;

                let expected_type = match self.explicit_return {
                    Some(v) => v,
                    None =>
                    // TODO: Change when native functions are ready :3
                    
                    return Err(CompilerError::new(221, "return in main scope")
                        .highlight(*source_range)
                            .note("consider using [todo system exit function]".to_string())
                        .build())
                };

                if !self.is_of_type(global, expected_type, datatype)? {
                    return Err(CompilerError::new(222, "invalid return type")
                        .highlight(expected_type.source_range)
                            .note(format!("defined as {}", global.to_string(expected_type.data_type)))
                        
                        .highlight(*source_range)
                            .note(format!("but the value returns {}", global.to_string(datatype.data_type)))
                        
                        .build()
                    )
                }
                Ok(())
            },
            
            
            Statement::FieldUpdate { structure, right, identifier, index_to } => {
                let structure_type = self.analyze(global, structure)?;
                
                match structure_type.data_type {
                    DataType::Struct(v) => {
                        let right_value = self.analyze(global, right)?;
                        let structure = self.get_struct(global, source_range, &v)?;

                        if let Some(v) = structure.fields.iter().enumerate().find(|x| x.1.0 == *identifier) {
                            *index_to = v.0;

                            if !self.is_of_type(global, v.1.1, right_value)? {
                                return Err(CompilerError::new(207, "variable is of different type")
                                    .highlight(*source_range)
                                        .note(format!("{} is of type {} but the assigned value is of type {}", global.symbol_table.get(*identifier), global.to_string(v.1.1.data_type), global.to_string(right_value.data_type)))
                                    .build())
                            }

                            return Ok(())
                        }

                    },

                    DataType::Any => return Ok(()),
                    _ => ()
                };

                return Err(CompilerError::new(220, "structure field doesn't exist")
                        .highlight(*source_range)
                            .note(format!("is of type {} which doesn't have a field named {}", global.to_string(structure_type.data_type), global.symbol_table.get(*identifier)))
                        .build()
                )
            },
        } 
    }
}

impl AnalysisState {
    #[inline]
    pub fn is_of_type(&self, global: &GlobalState, frst: SourcedDataType, oth: SourcedDataType) -> Result<bool, Error> {
        self.is_valid_type(global, frst)?;
        self.is_valid_type(global, oth)?;
        
        Ok(frst.data_type == oth.data_type || frst.data_type == DataType::Any || oth.data_type == DataType::Any)
    }

    fn is_valid_type(&self, global: &GlobalState, value: SourcedDataType) -> Result<(), Error> {
        let v = match value.data_type {
            DataType::Struct(v) => {
                self.get_struct(global, &value.source_range, &v)?;
                true
            },
            _ => true
        };

        if !v {
            return Err(CompilerError::new(214, "type doesn't exist")
                .highlight(value.source_range)
                    .note(format!("is of type {} which isn't declared", global.to_string(value.data_type)))
                .build())
            
        }

        Ok(())
    }

    
    fn declare_function(&mut self, symbol: SymbolIndex, function: Function) {
        self.functions.insert(symbol, (function, self.depth));
    }


    fn get_function(&self, symbol: &SymbolIndex) -> Option<&Function> {
        self.functions.get(symbol).map(|x| &x.0)
    }

    
    fn declare_struct(&mut self, symbol: SymbolIndex, mut structure: Structure) {
        structure.fields.sort_by_key(|x| x.0);
        self.structures.insert(symbol, (structure, self.depth));
    }

    
    fn get_struct(&self, global: &GlobalState, range: &SourceRange, symbol: &SymbolIndex) -> Result<&Structure, Error> {
        dbg!(&symbol);
        match self.structures.get(symbol).map(|x| &x.0) {
            Some(v) => Ok(v),
            None => Err(CompilerError::new(215, "structure isn't declared")
            .highlight(*range)
                .note(format!("there's no structure named {}", global.symbol_table.get(*symbol)))
            .build()),
        }
        
    }
}


impl GlobalState<'_> {
    #[inline]
    pub fn to_string(&self, data: DataType) -> String {
        format!("'{}'", data.to_string(self.symbol_table))
    }
    
}

