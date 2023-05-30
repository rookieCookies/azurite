#![feature(iter_intersperse)]
pub mod variable_stack;

use std::{collections::HashMap, fs};

use azurite_errors::{SourceRange, Error, CompilerError, ErrorBuilder, CombineIntoError, SourcedDataType};
use azurite_parser::ast::{Instruction, InstructionKind, Statement, Expression, BinaryOperator, Declaration};
use common::{DataType, SymbolTable, SymbolIndex};
use variable_stack::VariableStack;

#[derive(Debug, PartialEq)]
pub struct GlobalState<'a> {
    symbol_table: &'a mut SymbolTable,
    pub files: HashMap<SymbolIndex, (AnalysisState, Vec<Instruction>, String)>,
}


#[derive(Debug, PartialEq)]
pub struct AnalysisState {
    pub variable_stack: VariableStack,
    loop_depth: usize,

    functions: HashMap<SymbolIndex, (Function, usize)>,
    structures: HashMap<SymbolIndex, (Structure, usize)>,

    available_files: Vec<SymbolIndex>,
    
    explicit_return: Option<SourcedDataType>,

    depth: usize,
    file: SymbolIndex,
}


#[derive(Debug, PartialEq)]
struct Function {
    return_type: SourcedDataType,
    arguments: Vec<SourcedDataType>,
}


#[derive(Debug, PartialEq)]
struct Structure {
    fields: Vec<(SymbolIndex, SourcedDataType)>,
}


impl<'a> GlobalState<'a> {
    pub fn new(symbol_table: &'a mut SymbolTable) -> Self { Self { symbol_table, files: HashMap::new() } }
}


impl AnalysisState {
    pub fn new(file: SymbolIndex) -> Self {
        Self {
            variable_stack: VariableStack::new(),
            loop_depth: 0,
            depth: 0,
            explicit_return: None,
            functions: HashMap::new(),
            structures: HashMap::new(),
            available_files: vec![],
            file,

        }
    }

    pub fn start_analysis(&mut self, global: &mut GlobalState, instructions: &mut [Instruction]) -> Result<(), Error> {
        self.analyze_block(global, instructions, false)?;

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
            for x in instructions.iter() {
                if let InstructionKind::Declaration(d) = &x.instruction_kind {
                    self.declaration_early_process(global, &x.source_range, d)?
                }
            }
        }
        
        
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


    fn declaration_early_process(&mut self, global: &mut GlobalState, source_range: &SourceRange, declaration: &Declaration) -> Result<(), Error> {
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
                        self.declaration_early_process(global, &i.source_range, d)?
                    }
                }

            },

            
            Declaration::Extern { functions, .. } => {
                for f in functions {
                    self.declare_function(f.identifier, Function {
                        return_type: f.return_type,
                        arguments: f.arguments.clone(),
                    })
                }
            },

            
            Declaration::UseFile { file_name } => {
                self.available_files.push(*file_name);
                if global.files.contains_key(file_name) {
                    return Ok(())
                }
                
                let path = global.symbol_table.get(*file_name);
                let path = format!("{path}.az");
                let file = match fs::read_to_string(&path) {
                    Ok(v) => v,
                    Err(_) => {
                        let new_path = std::env::current_exe().unwrap().parent().unwrap().join("api").join(&path);
                        match fs::read_to_string(new_path) {
                            Ok(v) => v,
                            Err(_) => return Err(CompilerError::new(self.file, 223, "file doesn't exist")
                                .highlight(*source_range)
                                    .note(format!("can't find a file named {}", path))
                                .build())
                        }
                    },
                }.replace('\t', "    ");
                
                
                let tokens = azurite_lexer::lex(&file, *file_name, global.symbol_table);
                global.files.insert(*file_name, (AnalysisState::new(*file_name), vec![], file));

                let tokens = tokens?;
                let mut instructions = azurite_parser::parse(tokens.into_iter(), *file_name, global.symbol_table)?;
                let mut analysis = AnalysisState::new(*file_name);
                analysis.start_analysis(global, &mut instructions)?;

                let temp = global.files.get_mut(file_name).unwrap(); 
                temp.0 = analysis;
                temp.1 = instructions;
            },
        };
        Ok(())
    }
    

    fn analyze_declaration(&mut self, global: &mut GlobalState, declaration: &mut Declaration, source_range: &SourceRange) -> Result<(), Error> {
        match declaration {
            Declaration::FunctionDeclaration { arguments, return_type, body, source_range_declaration, name } => {
                let mut analysis_state = AnalysisState::new(self.file);

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


                let body_return_type = match analysis_state.analyze_block(global, body, true) {
                    Ok(v) => v,
                    Err(e) => {
                        self.functions = std::mem::take(&mut analysis_state.functions);
                        self.structures = std::mem::take(&mut analysis_state.structures);

                        return Err(e)
                        
                    },
                };

                self.functions = std::mem::take(&mut analysis_state.functions);
                self.structures = std::mem::take(&mut analysis_state.structures);

                if !self.is_of_type(global, *return_type, body_return_type)? {
                    return Err(CompilerError::new(self.file, 211, "function body returns a different type")
                        .highlight(*source_range_declaration)
                            .note(format!("function returns {}", global.to_string(return_type.data_type)))

                        .empty_line()
                        
                        .highlight(body.last().map_or(SourceRange::new(source_range.start + source_range_declaration.end, source_range.end - source_range_declaration.end), |x| x.source_range))
                            .note(format!("but the body returns {}", global.to_string(body_return_type.data_type)))
                        
                        .build())
                }

                Ok(())
            },


            Declaration::StructDeclaration { fields, .. } => {
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
                    self.is_valid_type(global, f.return_type)?;

                    for argument in &f.arguments {
                        self.is_valid_type(global, *argument)?;
                    }
                }

                Ok(())
            },

            
            Declaration::UseFile { .. } => Ok(()),
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
                                return Err(CompilerError::new(self.file, 201, "invalid type arithmetic operation")
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
                            return Err(CompilerError::new(self.file, 202, "comparisson types differ")
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
                    return Err(CompilerError::new(self.file, 203, "condition expects a boolean")
                        .highlight(condition.source_range)
                            .note(format!("is of type {}", global.to_string(condition_type.data_type)))
                        .build())
                }


                let body_type = self.analyze_block(global, body, true)?;

                if let Some(else_part) = else_part {
                    let else_type = self.analyze(global, else_part)?;

                    if !self.is_of_type(global, body_type, else_type)? {
                        return Err(CompilerError::new(self.file, 204, "if expressions branches don't return the same type")
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
                        Err(CompilerError::new(self.file, 205, "variable does not exist")
                            .highlight(*source_range)
                            .build()
                        )
                    },
                }
            },


            Expression::FunctionCall { identifier, arguments } => {
                let (function, new_identifier) = match self.get_function(global, identifier) {
                    Some(v) => v,
                    None => {
                        return Err(CompilerError::new(self.file, 212, "function isn't declared")
                            .highlight(*source_range)
                                .note(format!("there's no function named {}", global.symbol_table.get(*identifier)))
                            .build())
                    },
                };

                *identifier = new_identifier;
                let return_type = function.return_type;
        
                if function.arguments.len() != arguments.len() {
                    return Err(CompilerError::new(self.file, 214, "invalid number of arguments")
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
                            errors.push(CompilerError::new(self.file, 213, "argument is of invalid type")
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
                                field_errors.push(CompilerError::new(self.file, 217, "structure field and provided value are not of the same type")
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
                        field_errors.push(CompilerError::new(self.file, 218, "invalid fields")
                            .highlight(*source_range)
                                .note(format!("invalid: {}", invalid.into_iter().map(|x| global.symbol_table.get(x)).intersperse(", ".to_string()).collect::<String>()))
                            .build())
                    }


                    if !hashmap.is_empty() {
                        field_errors.push(CompilerError::new(self.file, 219, "missing fields")
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

                return Err(CompilerError::new(self.file, 220, "structure field doesn't exist")
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
                    return Err(CompilerError::new(self.file, 210, "value differs from type hint")
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
                                return Err(CompilerError::new(self.file, 206, "can't update a variable that does not exist")
                                    .highlight(left.source_range)
                                    .build());
                            },
                        };

                        let right_type = self.analyze(global, right)?;

                        if !self.is_of_type(global, right_type, value)? {
                            return Err(CompilerError::new(self.file, 207, "variable is of different type")
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
                    return Err(CompilerError::new(self.file, 208, "break outside of loop")
                        .highlight(*source_range)
                        .build())
                }
                Ok(())
            },
            
            
            Statement::Continue => {
                if self.loop_depth == 0 {
                    return Err(CompilerError::new(self.file, 209, "continue outside of loop")
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
                    return Err(CompilerError::new(self.file, 221, "return in main scope")
                        .highlight(*source_range)
                            .note("consider using 'exit(0)'".to_string())
                        .build())
                };

                if !self.is_of_type(global, expected_type, datatype)? {
                    return Err(CompilerError::new(self.file, 222, "invalid return type")
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
                                return Err(CompilerError::new(self.file, 207, "variable is of different type")
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

                return Err(CompilerError::new(self.file, 220, "structure field doesn't exist")
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
            return Err(CompilerError::new(self.file, 214, "type doesn't exist")
                .highlight(value.source_range)
                    .note(format!("is of type {} which isn't declared", global.to_string(value.data_type)))
                .build())
            
        }

        Ok(())
    }

    
    fn declare_function(&mut self, symbol: SymbolIndex, function: Function) {
        self.functions.insert(symbol, (function, self.depth));
    }


    fn get_function_detailed<'a>(&'a self, symbol_table: &mut SymbolTable, files: &'a HashMap<SymbolIndex, (AnalysisState, Vec<Instruction>, String)>, symbol: &SymbolIndex, implicit_complete: bool) -> Option<(&'a Function, SymbolIndex)> {
        let temp = self.functions.get(symbol);
        match temp.map(|x| &x.0) {
            Some(v) => Some((v, *symbol)),
            None => {
                let (root, root_excluded) = symbol_table.find_root(*symbol);

                if let Some(root_excluded) = root_excluded {
                    if self.available_files.contains(&root) {
                        if let Some(v) = files.get(&root)?.0.get_function_detailed(symbol_table, files, &root_excluded, false) {
                            return Some((v.0, symbol_table.add_combo(root, v.1)))
                        }
                    }
                }


                if !implicit_complete {
                    return None
                }
                
                for namespace in self.available_files.iter() {
                    if let Some(v) = files.get(namespace)?.0.get_function_detailed(symbol_table, files, symbol, false) {
                        return Some((v.0, symbol_table.add_combo(*namespace, v.1)))
                    }

                }
                
                None 
            },
        }
        
    }

    
    fn get_function<'a>(&'a self, global: &'a mut GlobalState, symbol: &SymbolIndex) -> Option<(&'a Function, SymbolIndex)> {
        self.get_function_detailed(global.symbol_table, &global.files, symbol, true)
    }

    
    fn declare_struct(&mut self, symbol: SymbolIndex, mut structure: Structure) {
        structure.fields.sort_by_key(|x| x.0);
        self.structures.insert(symbol, (structure, self.depth));
    }

    
    fn get_struct<'a>(&'a self, global: &'a GlobalState, range: &SourceRange, symbol: &SymbolIndex) -> Result<&'a Structure, Error> {
        match self.get_struct_option(global, symbol) {
            Some(v) => Ok(v),
            None => Err(CompilerError::new(self.file, 215, "structure isn't declared")
            .highlight(*range)
                .note(format!("there's no structure named {}", global.symbol_table.get(*symbol)))
            .build()),
        }
        
    }


    fn get_struct_option<'a>(&'a self, global: &'a GlobalState, symbol: &SymbolIndex) -> Option<&'a Structure> {
        match self.structures.get(symbol).map(|x| &x.0) {
            Some(v) => Some(v),
            None => {
                let (root, root_excluded) = global.symbol_table.find_root(*symbol);
                let root_excluded = root_excluded?;

                if !self.available_files.contains(&root_excluded) {
                    return None
                }
                
                global.files.get(&root)?.0.get_struct_option(global, &root_excluded)
            },
        }
    }
}


impl GlobalState<'_> {
    #[inline]
    pub fn to_string(&self, data: DataType) -> String {
        format!("'{}'", data.to_string(self.symbol_table))
    }
    
}

