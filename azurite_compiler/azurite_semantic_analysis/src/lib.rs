#![allow(clippy::map_entry)]
#![feature(hash_drain_filter)]
#![feature(iter_intersperse)]
pub mod variable_stack;

use std::{collections::HashMap, fs, path::{PathBuf, Path}};

use azurite_errors::{Error, CompilerError, ErrorBuilder, CombineIntoError};
use azurite_parser::ast::{Instruction, InstructionKind, Statement, Expression, BinaryOperator, Declaration, UnaryOperator};
use common::{DataType, SymbolTable, SymbolIndex, Data, SourceRange, SourcedDataType, generic_map::GenericMap, SourcedData, generic_declaration_suffix, get_generic_args_symbol};
use variable_stack::VariableStack;

const STD_LIBRARY : &str = include_str!("../../../builtin_libraries/azurite_api_files/std.az");


mod rename {
    pub const GENERIC : &str = "@";
}


#[derive(Debug, PartialEq)]
pub struct GlobalState<'a> {
    pub symbol_table: &'a mut SymbolTable,
    pub files: HashMap<SymbolIndex, (AnalysisState, Vec<Instruction>, String)>,

    functions: HashMap<SymbolIndex, Function>,
    pub template_functions: HashMap<SymbolIndex, TemplateFunction>,
    template_structures: HashMap<SymbolIndex, TemplateStructure>,
    structures: HashMap<SymbolIndex, Structure>,
}


#[derive(Debug, PartialEq)]
pub struct AnalysisState {
    pub variable_stack: VariableStack,
    loop_depth: usize,

    functions: HashMap<SymbolIndex, (SymbolIndex, usize)>,
    structures: HashMap<SymbolIndex, (SymbolIndex, usize)>,
    generics: Vec<SymbolIndex>,

    available_files: HashMap<SymbolIndex, SymbolIndex>,
    
    explicit_return: Option<SourcedDataType>,

    depth: usize,
    file: SymbolIndex,
    custom_path: SymbolIndex,

    cache_pieces_vec: Vec<SymbolIndex>,
}


#[derive(Debug, PartialEq)]
struct Function {
    return_type: SourcedDataType,
    arguments: Vec<SourcedDataType>,

    is_template_function: bool,
}


#[derive(Debug, PartialEq)]
pub struct TemplateFunction {
    name: SymbolIndex,
    
    return_type: SourcedDataType,
    arguments: Vec<(SymbolIndex, SourcedDataType)>,
    instructions: Vec<Instruction>,
    generics: Vec<SymbolIndex>,
    source_range: SourceRange,

    pub generated_funcs: Vec<Instruction>,
}


#[derive(Debug, PartialEq)]
pub struct TemplateStructure {
    name: SymbolIndex,

    fields: Vec<(SymbolIndex, SourcedDataType)>,
    generics: Vec<SymbolIndex>,
    source_range: SourceRange,
}


#[derive(Debug, PartialEq)]
struct Structure {
    fields: Vec<(SymbolIndex, SourcedDataType)>,
    is_template_structure: bool, 
}


impl<'a> GlobalState<'a> {
    pub fn new(symbol_table: &'a mut SymbolTable) -> Self { 
        Self {
            symbol_table, 
            files: HashMap::new(),
            functions: HashMap::new(),
            structures: HashMap::new(),
            template_functions: HashMap::new(),
            template_structures: HashMap::new(),
        }
    }
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
            available_files: HashMap::new(),
            file,
            custom_path: file,
            cache_pieces_vec: vec![],
            generics: vec![],

        }
    }


    pub fn start_analysis(&mut self, global: &mut GlobalState, instructions: &mut [Instruction]) -> Result<(), Error> {
        // {
        //     let file_name = global.symbol_table.add(String::from("std"));
        //     self.available_files.insert(file_name, file_name);
            
        //     if !global.files.contains_key(&file_name) {
        //         let file = STD_LIBRARY.replace('\t', "    ").replace('\r', "");
        
        //         let tokens = azurite_lexer::lex(&file, file_name, global.symbol_table);
        //         global.files.insert(file_name, (AnalysisState::new(file_name), vec![], file));

        //         let tokens = tokens?;
        //         let mut instructions = azurite_parser::parse(tokens.into_iter(), file_name, global.symbol_table)?;
        //         let mut analysis = AnalysisState::new(file_name);
        //         analysis.start_analysis(global, &mut instructions)?;

        //         let temp = global.files.get_mut(&file_name).unwrap(); 
        //         temp.0 = analysis;
        //         temp.1 = instructions;


        //     }
        // }
        
        self.analyze_block(global, instructions, false, true, None)?;

        Ok(())
    }
}


impl AnalysisState {
    fn analyze(&mut self, global: &mut GlobalState, instruction: &mut Instruction, expected: Option<DataType>) -> Result<SourcedDataType, Error> {
        match &mut instruction.instruction_kind {
            InstructionKind::Statement(s) => {
                self.analyze_statement(global, s, &instruction.source_range)?;
            },
            
            
            InstructionKind::Expression(e) => return self.analyze_expression(global, e, &instruction.source_range, expected),
            
            
            InstructionKind::Declaration(d) => {
                self.analyze_declaration(global, d, &instruction.source_range)?;
            },
        };

        Ok(SourcedDataType::new(instruction.source_range, DataType::Empty))
    }
    

    fn analyze_block(&mut self, global: &mut GlobalState, instructions: &mut [Instruction], reset: bool, pre_declaration: bool, expected: Option<DataType>) -> Result<SourcedDataType, Error> {
        let top = self.variable_stack.len();

        if reset {
            self.depth += 1;
            
        }
        // Declarations
        if pre_declaration {
            let mut errors = vec![];
            for x in instructions.iter_mut() {
                if let InstructionKind::Declaration(d) = &mut x.instruction_kind {
                    if let Err(e) = self.declaration_early_process(global, &x.source_range, d) {
                        errors.push(e);
                    }
                }
            }


            for x in instructions.iter_mut() {
                if let InstructionKind::Declaration(Declaration::StructDeclaration { name, fields, generics }) = &mut x.instruction_kind {
                    for field in fields.iter_mut() {
                        if self.update_type(&mut field.1, global).is_err() {
                            field.1.data_type = DataType::Any;
                        }
                    }

                    global.structures.get_mut(name).unwrap().fields = fields.clone();
                }
            }

            if !errors.is_empty() {
                return Err(errors.combine_into_error())
            }
        }
        
        
        let mut errors = vec![];
        let size = instructions.len();
        instructions.iter_mut().take(size.max(1)-1).for_each(|x| if let Err(e) = self.analyze(global, x, None) {
            errors.push(e);
        });

        let mut return_val = SourcedDataType::new(SourceRange::new(0, 0), DataType::Empty);
        if let Some(v) = instructions.last_mut() {
            match self.analyze(global, v, expected) {
                Ok(v) => return_val = v,
                Err(e) => {
                    return_val = SourcedDataType::new(v.source_range, DataType::Any);
                    errors.push(e)
                },
            }
        }

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


    fn analyze_declaration(&mut self, global: &mut GlobalState, declaration: &mut Declaration, source_range: &SourceRange) -> Result<(), Error> {
        match declaration {
            Declaration::FunctionDeclaration { arguments, return_type, body, source_range_declaration, name, generics } => {
                let mut analysis_state = AnalysisState::new(self.file);

                analysis_state.functions = std::mem::take(&mut self.functions);
                analysis_state.structures = std::mem::take(&mut self.structures);
                analysis_state.available_files = std::mem::take(&mut self.available_files);
                analysis_state.generics = std::mem::take(generics);

                
                analysis_state.depth = self.depth;
                if let Err(e) = analysis_state.update_type(return_type, global) {
                    self.functions = std::mem::take(&mut analysis_state.functions);
                    self.structures = std::mem::take(&mut analysis_state.structures);
                    self.available_files = std::mem::take(&mut analysis_state.available_files);
                    *generics = std::mem::take(&mut analysis_state.generics);

                    return Err(e)
                }
                

                analysis_state.explicit_return = Some(*return_type);

                {

                    let mut errors = vec![];
                    
                    for argument in arguments.iter_mut() {
                        if let Err(e) = analysis_state.update_type(&mut argument.1, global) {
                            errors.push(e);
                            analysis_state.variable_stack.push(argument.0, SourcedDataType::new(argument.1.source_range, DataType::Any));
                            continue;
                        };

                        analysis_state.variable_stack.push(argument.0, argument.1);
                    }

                    if !errors.is_empty() {
                        self.functions = std::mem::take(&mut analysis_state.functions);
                        self.structures = std::mem::take(&mut analysis_state.structures);
                        self.available_files = std::mem::take(&mut analysis_state.available_files);
                        *generics = std::mem::take(&mut analysis_state.generics);

                        return Err(errors.combine_into_error())
                    }

                }


                let body_return_type = analysis_state.analyze_block(global, body, true, true, Some(return_type.data_type));
                let body_return_type = match body_return_type {
                    Ok(v) => v,
                    Err(e) => {
                        self.functions = std::mem::take(&mut analysis_state.functions);
                        self.structures = std::mem::take(&mut analysis_state.structures);
                        self.available_files = std::mem::take(&mut analysis_state.available_files);
                        *generics = std::mem::take(&mut analysis_state.generics);

                        return Err(e)
                        
                    },
                };

                let return_type_is_not_same_as_body_type = (body.last().is_none() && return_type.data_type != DataType::Empty) ||
                    (body.last().is_some() && !analysis_state.is_of_type(global, (body_return_type, body.last_mut().unwrap()), *return_type).unwrap_or(false)); 
                

                self.functions = std::mem::take(&mut analysis_state.functions);
                self.structures = std::mem::take(&mut analysis_state.structures);
                self.available_files = std::mem::take(&mut analysis_state.available_files);
                *generics = std::mem::take(&mut analysis_state.generics);

                if return_type_is_not_same_as_body_type {
                    dbg!(&return_type, &body_return_type);
                    return Err(CompilerError::new(self.file, 211, "function body returns a different type")
                        .highlight(*source_range_declaration)
                            .note(format!("function returns {}", global.to_string(return_type.data_type)))

                        .empty_line()
                        
                        .highlight(body.last().map_or(SourceRange::new(source_range_declaration.end, source_range.end), |x| x.source_range))
                            .note(format!("but the body returns {}", global.to_string(body_return_type.data_type)))
                        
                        .build())
                }

                Ok(())
            },


            Declaration::StructDeclaration { fields, .. } => {
                let errs = fields
                    .iter_mut()
                    .map(|x| self.update_type(&mut x.1, global))
                    .filter_map(|x| if let Err(x) = x { Some(x) } else { None })
                    .collect::<Vec<_>>();

                if !errs.is_empty() {
                    return Err(errs.combine_into_error())
                }

                Ok(())
            },

            
            Declaration::Namespace { body, .. } => {
                self.analyze_block(global, body, false, false, None)?;
                Ok(())
                
            },

            
            Declaration::Extern { .. } => Ok(()),


            Declaration::ImplBlock { body, datatype } => {
                self.update_type(datatype, global)?;
                if let DataType::Struct(v) = &mut datatype.data_type {
                    let (_, name) = self.get_struct(global, source_range, v).unwrap();
                    *v = name;
                }

                self.analyze_block(global, body, false, false, None)?;

                Ok(())
            },

            
            Declaration::UseFile { .. } => Ok(()),
        }
    }
    

    fn analyze_expression(&mut self, global: &mut GlobalState, expression: &mut Expression, source_range: &SourceRange, expected: Option<DataType>) -> Result<SourcedDataType, Error> {
        macro_rules! match_macro {
            ($v: ident) => {
                (DataType::Any, DataType::$v)
                | (DataType::$v, DataType::Any)
                | (DataType::$v, DataType::$v)
            }
        }

        macro_rules! all_integer {
            () => {
                DataType::I8
                | DataType::I16
                | DataType::I32
                | DataType::I64
                | DataType::U8
                | DataType::U16
                | DataType::U32
                | DataType::U64
            }
        }

        match expression {
            Expression::AsCast { value, cast_type } => {
                let value_type = self.analyze(global, &mut *value, expected)?;

                match (value_type.data_type, cast_type.data_type){
                    (
                        all_integer!()
                            | DataType::Float
                            | DataType::Any,
                        all_integer!()
                            | DataType::Float
                            | DataType::Any
                        
                    ) => Ok(*cast_type),

                    _ => Err(CompilerError::new(self.file, 226, "can only cast beteen primitives")
                            .highlight(*source_range)
                                .note(format!("value is of type {}", global.to_string(value_type.data_type)))
                            .build()
                    ),
                }
            }

            
            Expression::Data(v) => {
                let expected = match expected {
                    Some(v) => v,
                    None => return Ok(SourcedDataType::from(v)),
                };

                macro_rules! conversion {
                    ($i: ident) => {
                        match (&v.data, expected) {
                            (Data::$i(n), DataType::I8)  => if let Ok(val) = i8 ::try_from(*n) { v.data = Data::I8 (val); },
                            (Data::$i(n), DataType::I16) => if let Ok(val) = i16::try_from(*n) { v.data = Data::I16(val); },
                            (Data::$i(n), DataType::I32) => if let Ok(val) = i32::try_from(*n) { v.data = Data::I32(val); },
                            (Data::$i(n), DataType::I64) => if let Ok(val) = i64::try_from(*n) { v.data = Data::I64(val); },
                            (Data::$i(n), DataType::U8)  => if let Ok(val) = u8 ::try_from(*n) { v.data = Data::U8 (val); },
                            (Data::$i(n), DataType::U16) => if let Ok(val) = u16::try_from(*n) { v.data = Data::U16(val); },
                            (Data::$i(n), DataType::U32) => if let Ok(val) = u32::try_from(*n) { v.data = Data::U32(val); },
                            (Data::$i(n), DataType::U64) => if let Ok(val) = u64::try_from(*n) { v.data = Data::U64(val); },

                            _ => (),
                            
                        }
                    }
                }

                conversion!(I8);
                conversion!(I16);
                conversion!(I32);
                conversion!(I64);
                conversion!(U8);
                conversion!(U16);
                conversion!(U32);
                conversion!(U64);

                Ok(SourcedDataType::from(v))
            },
            
            Expression::BinaryOp { operator, left, right } => {
                let left_type  = self.analyze(global, left, expected)?;
                let right_type = self.analyze(global, right, Some(left_type.data_type))?;

                let data_type = match *operator {
                    | BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Modulo
                    | BinaryOperator::Divide => {
                        match (left_type.data_type, right_type.data_type) {
                            match_macro!(I8) => DataType::I8,
                            match_macro!(I16) => DataType::I16,
                            match_macro!(I32) => DataType::I32,
                            match_macro!(I64) => DataType::I64,

                            match_macro!(U8) => DataType::U8,
                            match_macro!(U16) => DataType::U16,
                            match_macro!(U32) => DataType::U32,
                            match_macro!(U64) => DataType::U64,

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
                    | BinaryOperator::NotEquals => {
                        if !self.is_of_type(global, (left_type, left), right_type)? {
                            return Err(CompilerError::new(self.file, 202, "comparisson types differ")
                                .highlight(SourceRange::combine(left.source_range, right.source_range))
                                    .note(format!("left side is of type {} while the right side is of type {}", global.to_string(left_type.data_type), global.to_string(right_type.data_type)))
                                .build())
                        }
            
                        DataType::Bool
                    }


                    | BinaryOperator::GreaterThan
                    | BinaryOperator::LesserThan
                    | BinaryOperator::GreaterEquals
                    | BinaryOperator::LesserEquals => {
                        match (left_type.data_type, right_type.data_type) {
                            | match_macro!(I8)
                            | match_macro!(I16)
                            | match_macro!(I32)
                            | match_macro!(I64)
                            | match_macro!(U8)
                            | match_macro!(U16)
                            | match_macro!(U32)
                            | match_macro!(U64)
                            | (DataType::Any, DataType::Float)
                            | (DataType::Float, DataType::Any)
                            | (DataType::Float, DataType::Float)
                            | (DataType::Any, DataType::Any) => DataType::Bool,
                            
                            _ => {
                                return Err(CompilerError::new(self.file, 224, "invalid type order operation")
                                    .highlight(SourceRange::combine(left.source_range, right.source_range))
                                        .note(format!("left side is of type {} while the right side is of type {}", global.to_string(left_type.data_type), global.to_string(right_type.data_type)))
                                    .build())
                            }
                        }
                    }
                    
                };

                Ok(SourcedDataType::new(*source_range, data_type))
            },

            
            Expression::UnaryOp { operator, value } => {
                let value_type = self.analyze(global, &mut *value, expected)?;

                let is_valid = match operator {
                    UnaryOperator::Not => matches!(value_type.data_type, DataType::Bool),
                    UnaryOperator::Negate => matches!(value_type.data_type, DataType::Float) || value_type.data_type.is_signed_integer(),
                };

                if !is_valid {
                    return Err(CompilerError::new(self.file, 225, "invalid type unary operation")
                        .highlight(*source_range)
                        .build());
                }

                let mut value_type = value_type;
                value_type.source_range.start = source_range.start;

                Ok(value_type)
            },

            
            Expression::Block { body } => {
                self.analyze_block(global, body, true, true, expected)
            },


            Expression::IfExpression { body, condition, else_part } => {
                let condition_type = self.analyze(global, condition, Some(DataType::Bool))?;

                if !self.is_of_type(global, (condition_type, condition), SourcedDataType::new(SourceRange::new(0, 0), DataType::Bool))? {
                    return Err(CompilerError::new(self.file, 203, "condition expects a boolean")
                        .highlight(condition.source_range)
                            .note(format!("is of type {}", global.to_string(condition_type.data_type)))
                        .build())
                }


                let body_type = self.analyze_block(global, body, true, true, expected)?;

                if let Some(else_part) = else_part {
                    let else_type = self.analyze(global, else_part, expected)?;

                    if body.last().is_none() || !self.is_of_type(global, (body_type, body.last_mut().unwrap()), else_type)? {
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


            Expression::FunctionCall { identifier, arguments, created_by_accessing, generics } => {
                if *created_by_accessing {
                    let associated_type = self.analyze(global, &mut arguments[0], None)?;
                    if let DataType::Any = associated_type.data_type {
                        return Ok(associated_type)
                    };

                    
                    let associated_type_index = associated_type.data_type.symbol_index(global.symbol_table);

                    {
                        let pieces = &mut self.cache_pieces_vec;
                        let mut temp = associated_type_index;
                        while let (root, Some(v)) = global.symbol_table.find_root(temp) {
                            temp = v;
                            pieces.push(root);
                        }

                        pieces.push(temp);

                        for i in pieces.iter().rev() {
                            *identifier = global.symbol_table.add_combo(*i, *identifier);
                        }

                        pieces.clear();
                    }
                }

                
                let (mut function, mut absolute_identifier) = match self.get_function(global, identifier) {
                    Some(v) => v,
                    None => {
                        return Err(CompilerError::new(self.file, 212, "function isn't declared")
                            .highlight(*source_range)
                                .note(format!("there's no function named {}", global.symbol_table.get(*identifier)))
                            .build())
                    },
                };


                if function.is_template_function {
                    let name = self.create_function_from_template(
                        global,
                        absolute_identifier,
                        &generics.iter().map(|x| x.data_type).collect::<Vec<_>>()
                    );
                    
                    absolute_identifier = name;
                    function = global.functions.get(&name).unwrap();
                }
                

                *identifier = absolute_identifier;
                let return_type = function.return_type;
        
                if function.arguments.len() != arguments.len() {
                    return Err(CompilerError::new(self.file, 214, "invalid number of arguments")
                        .highlight(*source_range)
                            .note(format!("expected {} arguments found {}", function.arguments.len(), arguments.len()))
                        .build())
                }

    
                {

                    let mut errors = vec![];
        
                    let temp = function.arguments.clone();
                    let mut iter = arguments.iter_mut().zip(temp.iter());
                    if *created_by_accessing {
                        iter.next();
                    }

                    for (argument, expected_type) in iter {
                        let argument_type = match self.analyze(global, argument, Some(expected_type.data_type)) {
                            Ok(v) => v,
                            Err(e) => {
                                errors.push(e);
                                continue
                            },
                        };


                        let is_of_type = match self.is_of_type(global, (argument_type, argument), *expected_type) {
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

            
            Expression::StructureCreation { identifier, fields, identifier_range, generics } => {
                let (mut structure, mut full_name) = self.get_struct(global, identifier_range, identifier)?;
                if structure.is_template_structure {
                    full_name = self.create_structure_from_template(
                        global, 
                        full_name, 
                        &generics.iter().map(|x| x.data_type).collect::<Vec<_>>()
                    );

                    structure = global.structures.get(&full_name).unwrap();
                }
                
                *identifier = full_name;
                
                {
                    let mut hashmap = structure.fields.iter().copied().collect::<HashMap<_, _>>();
                    let mut invalid = vec![];
                    let mut field_errors = vec![];


                    for given_field in fields.iter_mut() {
                        if let Some(v) = hashmap.remove(&given_field.0) {
                            let instruction_type = match self.analyze(global, &mut given_field.1, Some(v.data_type)) {
                                Ok(v) => v,
                                Err(e) => {
                                    field_errors.push(e);
                                    continue
                                },
                            };

                            let is_same_type = match self.is_of_type(global, (instruction_type, &mut given_field.1), v) {
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
                let structure_type = self.analyze(global, structure, None)?;
                
                match structure_type.data_type {
                    DataType::Struct(v) => {
                        // The getting straight from the 'global' instead of using
                        // 'get_struct' is intentional. Any value that returns a
                        // type which is of 'DataType::Struct' should've already
                        // converted that to the fully qualified name.
                        let structure = global.structures.get(&v).unwrap();

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

            
            Expression::WithinNamespace { do_within, .. } => {
                self.analyze(global, do_within, None)
            },
        }
    }
    
    
    fn analyze_statement(&mut self, global: &mut GlobalState, statement: &mut Statement, source_range: &SourceRange) -> Result<(), Error> {
        match statement {
            Statement::DeclareVar { identifier, data, type_hint } => {
                if let Some(v) = type_hint {
                    self.update_type(v, global)?;
                }
                let data_type = match self.analyze(global, &mut *data, type_hint.map(|x| x.data_type)) {
                    Ok(v) => v,
                    Err(e) => {
                        self.variable_stack.push(*identifier, SourcedDataType::new(*source_range, DataType::Any));
                        return Err(e)
                    },
                };
                
                self.variable_stack.push(*identifier, if let Some(v) = type_hint { *v } else { data_type });

                if !type_hint.map_or(Ok(true), |x| self.is_of_type(global, (data_type, data), x))? {
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

                        let right_type = self.analyze(global, right, Some(value.data_type))?;

                        if !self.is_of_type(global, (right_type, right), value)? {
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

                self.analyze_block(global, body, true, true, None)?;

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
                let expected_type = match self.explicit_return {
                    Some(v) => v,
                    None =>
                    return Err(CompilerError::new(self.file, 221, "return in main scope")
                        .highlight(*source_range)
                            .note("consider using 'exit(0)'".to_string())
                        .build())
                };

                let datatype = self.analyze(global, v, Some(expected_type.data_type))?;

                if !self.is_of_type(global, (datatype, v), expected_type)? {
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
                let structure_type = self.analyze(global, structure, None)?;
                
                match structure_type.data_type {
                    DataType::Struct(v) => {
                        // The getting straight from the 'global' instead of using
                        // 'get_struct' is intentional. Any value that returns a
                        // type which is of 'DataType::Struct' should've already
                        // converted that to the fully qualified name.
                        let structure = global.structures.get(&v).unwrap();

                        if let Some(v) = structure.fields.iter().enumerate().find(|x| x.1.0 == *identifier) {
                            *index_to = v.0;
                            let field_type = v.1.1;
                            let right_value = self.analyze(global, right, Some(field_type.data_type))?;

                            if !self.is_of_type(global, (right_value, right), field_type)? {
                                return Err(CompilerError::new(self.file, 207, "variable is of different type")
                                    .highlight(*source_range)
                                        .note(format!("{} is of type {} but the assigned value is of type {}", global.symbol_table.get(*identifier), global.to_string(field_type.data_type), global.to_string(right_value.data_type)))
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

    
    fn declaration_early_process(&mut self, global: &mut GlobalState, source_range: &SourceRange, declaration: &mut Declaration) -> Result<(), Error> {
        match declaration {
            Declaration::FunctionDeclaration { name, arguments, return_type, source_range_declaration, generics, body } => {
                let new_name = global.symbol_table.add_combo(self.custom_path, *name);
                self.functions.insert(*name, (new_name, self.depth));
                *name = new_name;
                
                if global.functions.contains_key(name) {
                    return Err(CompilerError::new(self.file, 227, "duplicate function definition")
                        .highlight(*source_range_declaration)
                            .note("this function is already defined".to_string())
                        .build())
                }


                let mut arguments_type : Vec<_> = arguments.iter().map(|x| x.1).collect();
                let mut return_type = *return_type;


                if self.update_type(&mut return_type, global).is_err() {
                    return_type.data_type = DataType::Any;
                }

                arguments_type.iter_mut().for_each(|x| {
                    if self.update_type(x, global).is_err() {
                        x.data_type = DataType::Any;
                    }
                });
                
                
                if !generics.is_empty() {
                    let function = TemplateFunction { return_type, arguments: arguments.clone(), instructions: body.clone(), name: *name, generics: generics.clone(), generated_funcs: vec![], source_range: *source_range };
                    global.template_functions.insert(*name, function);
                }


                let function = Function { return_type, arguments: arguments_type, is_template_function: !generics.is_empty() };
                global.functions.insert(*name, function);
            },

            
            Declaration::StructDeclaration { name, generics, fields  } => {
                {
                    let new_name = global.symbol_table.add_combo(self.custom_path, *name);
                    self.structures.insert(*name, (new_name, self.depth));
                    *name = new_name;
                }
                
                if global.functions.contains_key(name) {
                    return Err(CompilerError::new(self.file, 228, "duplicate struct definition")
                        .highlight(*source_range)
                            .note("this structure is already defined".to_string())
                        .build())
                }


                if !generics.is_empty() {
                    let structure = TemplateStructure { name: *name, fields: std::mem::take(fields), generics: generics.clone(), source_range: *source_range  };
                    global.template_structures.insert(*name, structure);
                }

                let mut structure = Structure {
                    fields: vec![],
                    is_template_structure: !generics.is_empty(),
                };

                structure.fields.sort_by_key(|x| x.0);
                global.structures.insert(*name, structure);
            },

            
            Declaration::Namespace { body, .. } => {
                for i in body.iter_mut() {
                    if let InstructionKind::Declaration(d) = &mut i.instruction_kind {
                        self.declaration_early_process(global, &i.source_range, d)?
                    }
                }

            },

            
            Declaration::Extern { functions, .. } => {
                for f in functions.iter_mut() {
                    let new_name = global.symbol_table.add_combo(self.custom_path, f.identifier);
                    self.functions.insert(f.identifier, (new_name, self.depth));
                    f.identifier = new_name;

                    if self.update_type(&mut f.return_type, global).is_err() {
                        f.return_type.data_type = DataType::Any;
                    }
                    

                    for argument in f.arguments.iter_mut() {
                        if self.update_type(argument, global).is_err() {
                            argument.data_type = DataType::Any;
                        }
                    }


                    global.functions.insert(f.identifier, Function {
                        return_type: f.return_type,
                        arguments: f.arguments.clone(),
                        is_template_function: false,
                    });
                }
            },

            
            Declaration::UseFile { file_name } => {
                let path = global.symbol_table.get(*file_name);
                let mut path = PathBuf::from(path);
                path.set_extension("az");

                let current_file_path = global.symbol_table.find_root(self.custom_path).0;
                let current_file_path = PathBuf::from(global.symbol_table.get(current_file_path));
                let path_local_to_file = Path::join(current_file_path.parent().unwrap(), &path);

                if let Some(v) = global.symbol_table.find(path_local_to_file.to_string_lossy().to_string().as_str()) {
                    if global.files.contains_key(&v) {
                        self.available_files.insert(*file_name, v);
                        *file_name = v;
                        return Ok(())
                    }
                } else {
                    let new_path = std::env::current_exe().unwrap().parent().unwrap().join("api").join(&path);

                    if let Some(v) = global.symbol_table.find(new_path.to_string_lossy().to_string().as_str()) {
                        if global.files.contains_key(&v) {
                            self.available_files.insert(*file_name, v);
                            *file_name = v;
                            return Ok(())
                        }
                    }
                }


                let (file, path) = match fs::read_to_string(&path_local_to_file) {
                    Ok(v) => (v, path_local_to_file),
                    Err(_) => {
                        let new_path = std::env::current_exe().unwrap().parent().unwrap().join("api").join(&path);
                        match fs::read_to_string(&new_path) {
                            Ok(v) => (v, new_path),
                            Err(_) => return Err(CompilerError::new(self.file, 223, "file doesn't exist")
                                .highlight(*source_range)
                                    .note(format!("can't find a file named {} at any of the following paths: {}, {}",
                                        global.symbol_table.get(*file_name),
                                        path_local_to_file.to_string_lossy(),
                                        new_path.to_string_lossy(),
                                ))
                                .build())
                        }
                    },
                };

                
                let file = file.replace('\t', "    ").replace('\r', "");
                let path = global.symbol_table.add(path.to_string_lossy().to_string());
                self.available_files.insert(*file_name, path);
                
                let tokens = azurite_lexer::lex(&file, path, global.symbol_table);
                global.files.insert(path, (AnalysisState::new(path), vec![], file));
                *file_name = path;

                let tokens = tokens?;
                let mut instructions = azurite_parser::parse(tokens, path, global.symbol_table)?;
                let mut analysis = AnalysisState::new(path);
                analysis.start_analysis(global, &mut instructions)?;

                let temp = global.files.get_mut(&path).unwrap(); 
                temp.0 = analysis;
                temp.1 = instructions;
            },

            
            Declaration::ImplBlock { body, .. } => {
                for i in body.iter_mut() {
                    match &mut i.instruction_kind {
                        InstructionKind::Declaration(v) => self.declaration_early_process(global, &i.source_range, v)?,

                        _ => unreachable!()
                    }
                }
            },
        };
        Ok(())
    }
}

impl AnalysisState {
    #[inline]
    pub fn is_of_type(&self, global: &mut GlobalState, (frst, instr): (SourcedDataType, &mut Instruction), oth: SourcedDataType) -> Result<bool, Error> {
        self.is_valid_type(global, frst)?;
        self.is_valid_type(global, oth)?;

        if frst.data_type == oth.data_type || frst.data_type == DataType::Any || oth.data_type == DataType::Any {
            return Ok(true)
        }

        match (frst.data_type, oth.data_type) {
            | (DataType::U8 , DataType::I16)
            | (DataType::U8 , DataType::I32)
            | (DataType::U8 , DataType::I64)
            | (DataType::U8 , DataType::U8 )
            | (DataType::U8 , DataType::U16)
            | (DataType::U8 , DataType::U32)
            | (DataType::U8 , DataType::U64)
            | (DataType::U16, DataType::I32)
            | (DataType::U16, DataType::I64)
            | (DataType::U16, DataType::U32)
            | (DataType::U16, DataType::U64)
            | (DataType::U32, DataType::I64)
            | (DataType::U32, DataType::U64) => {
                let temp = std::mem::replace(instr, Instruction { instruction_kind: InstructionKind::Expression(
                    Expression::Data(
                        SourcedData::new(SourceRange::new(0, 0), Data::I8(0)),
                    )), source_range: SourceRange::new(0, 0) });

                *instr = Instruction {
                    source_range: instr.source_range,
                    instruction_kind: InstructionKind::Expression(Expression::AsCast { value: Box::new(temp), cast_type: oth }),
                };

                Ok(true)
            },

            _ => Ok(false)
        }
    }


    fn update_type(&self, datatype: &mut SourcedDataType, global: &mut GlobalState) -> Result<(), Error> {
        self.is_valid_type(global, *datatype)?;
        if let DataType::Struct(v) = &mut datatype.data_type {
            if !self.generics.contains(v) {
                *v = self.get_struct(global, &datatype.source_range, v).unwrap().1;
            }

        };

        Ok(())
    }

    
    fn is_valid_type(&self, global: &mut GlobalState, value: SourcedDataType) -> Result<(), Error> {
        let v = match value.data_type {
            DataType::Struct(v) if !self.generics.contains(&v) => {
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

    
    fn get_function_detailed<'a>(
            &self,
            symbol_table: &mut SymbolTable,
            files: &HashMap<SymbolIndex, (AnalysisState, Vec<Instruction>, String)>,
            functions: &'a HashMap<SymbolIndex, Function>,
            symbol: &SymbolIndex,
            implicit_complete: bool
    ) -> Option<(&'a Function, SymbolIndex)> {
        let temp = self.functions.get(symbol);
        match temp.map(|x| (functions.get(&x.0).unwrap(), x.0)) {
            Some((func, absolute_ident)) => Some((func, absolute_ident)),
            None => {
                let (root, root_excluded) = symbol_table.find_root(*symbol);

                if let Some(root_excluded) = root_excluded {
                    if self.available_files.contains_key(&root) {
                        if let Some(v) = files.get(&root)?.0.get_function_detailed(symbol_table, files, functions, &root_excluded, false) {
                            return Some((v.0, v.1))
                        }
                    }
                }

                if !implicit_complete {
                    return None
                }
                
                for namespace in self.available_files.iter() {
                    if let Some(v) = files.get(namespace.1)?.0.get_function_detailed(symbol_table, files, functions, symbol, false) {
                        return Some((v.0, v.1))
                    }

                }


                None 
            },
        }
        
    }

    
    fn get_function<'a>(&'a self, global: &'a mut GlobalState, symbol: &SymbolIndex) -> Option<(&'a Function, SymbolIndex)> {
        if let Some(v) = global.functions.get(symbol) {
            return Some((v, *symbol));
        }
        
        self.get_function_detailed(global.symbol_table, &global.files, &global.functions, symbol, true)
    }

    
    fn get_struct<'a>(&'a self, global: &'a mut GlobalState, range: &SourceRange, symbol: &SymbolIndex) -> Result<(&'a Structure, SymbolIndex), Error> {
        match self.get_struct_option(global.symbol_table, &global.files, &global.structures, symbol, true) {
            Some(v) => Ok(v),
            None => Err(CompilerError::new(self.file, 215, "structure isn't declared")
                .highlight(*range)
                    .note(format!("there's no structure named {}", global.symbol_table.get(*symbol)))
                .build()),
            // None => panic!(),
        }
        
    }


    fn get_struct_option<'a>(
        &self,
        symbol_table: &mut SymbolTable,
        files: &HashMap<SymbolIndex, (AnalysisState, Vec<Instruction>, String)>,
        structures: &'a HashMap<SymbolIndex, Structure>,
        symbol: &SymbolIndex,
        implicit_complete: bool
    ) -> Option<(&'a Structure, SymbolIndex)> {
        if let Some(v) = structures.get(symbol) {
            return Some((v, *symbol));
        }

        let temp = self.structures.get(symbol);
        match temp.map(|x| (structures.get(&x.0).unwrap(), x.0)) {
            Some((structure, absolute_ident)) => Some((structure, absolute_ident)),
            None => {
                let (root, root_excluded) = symbol_table.find_root(*symbol);

                if let Some(root_excluded) = root_excluded {
                    if self.available_files.contains_key(&root) {
                        if let Some(v) = files.get(&root)?.0.get_struct_option(symbol_table, files, structures, &root_excluded, false) {
                            return Some((v.0, v.1))
                        }
                    }
                }

                if !implicit_complete {
                    return None
                }
                
                for namespace in self.available_files.iter() {
                    if let Some(v) = files.get(namespace.1)?.0.get_struct_option(symbol_table, files, structures, symbol, false) {
                        return Some((v.0, v.1))
                    }

                }


                None 
            },
        }
        
    }


    fn create_function_from_template(&mut self, global: &mut GlobalState, base_name: SymbolIndex, generics: &[DataType]) -> SymbolIndex {
        let base = global.template_functions.get(&base_name).unwrap();
        assert_eq!(base.generics.len(), generics.len());

        let declaration_suffix = generic_declaration_suffix(global.symbol_table, generics);

        let name = global.symbol_table.add_combo(base.name, declaration_suffix);

        let mut instructions = base.instructions.clone();
        let mut return_type = base.return_type;
        let mut arguments = base.arguments.clone();
        
        let mut type_conversion_state = TypeConversionState {
            types: base.generics.iter().zip(generics.iter()).map(|x| (*x.0, *x.1)).collect(),
            declaration_suffix,
            symbol_map: global.symbol_table,
        };

        type_conversion_state.convert_types(&mut instructions);
        type_conversion_state.convert_data_type(&mut return_type.data_type);
        arguments.iter_mut().for_each(|x| type_conversion_state.convert_data_type(&mut x.1.data_type));
        
        global.functions.insert(name, Function { return_type, arguments: arguments.iter().map(|x| x.1).collect(), is_template_function: false });

        let mut instruction = Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::FunctionDeclaration {
                name,
                arguments,
                return_type,
                body: instructions,
                generics: vec![],
                source_range_declaration: base.source_range,
            }),
            
            source_range: base.source_range,
        };


        self.depth += 1;
        self.analyze(global, &mut instruction, None).unwrap();
        global.template_functions.get_mut(&base_name).unwrap().generated_funcs.push(instruction);


        let generics_args_symbol = get_generic_args_symbol(global.symbol_table);
        for (_, v) in self.structures.drain_filter(|_, y| y.1 == self.depth) {
            let structure = global.structures.remove(&v.0).unwrap();
            let name = global.symbol_table.add_combo(v.0, generics_args_symbol);
            global.structures.insert(name, structure);
        }

        
        for (_, v) in self.functions.drain_filter(|_, y| y.1 == self.depth) {
            let function = global.functions.remove(&v.0).unwrap();
            let name = global.symbol_table.add_combo(v.0, generics_args_symbol);
            global.functions.insert(name, function);
        }

        
        name
    }

    
    fn create_structure_from_template(&mut self, global: &mut GlobalState, base_name: SymbolIndex, generics: &[DataType]) -> SymbolIndex {
        let base = global.template_structures.get(&base_name).unwrap();
        assert_eq!(base.generics.len(), generics.len());

        let declaration_suffix = generic_declaration_suffix(global.symbol_table, generics);

        let name = global.symbol_table.add_combo(base.name, declaration_suffix);
        let mut fields = base.fields.clone();

        let type_conversion_state = TypeConversionState {
            types: base.generics.iter().zip(generics.iter()).map(|x| (*x.0, *x.1)).collect(),
            declaration_suffix,
            symbol_map: global.symbol_table,
        };

        fields.iter_mut().for_each(|x| type_conversion_state.convert_data_type(&mut x.1.data_type));
        
        global.structures.insert(name, Structure { fields: fields.clone(), is_template_structure: false });

        let mut instruction = Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::StructDeclaration {
                name,
                fields,
                generics: base.generics.clone(),
            }),
            
            source_range: base.source_range,
        };


        self.depth += 1;
        self.analyze(global, &mut instruction, None).unwrap();


        for (_, v) in self.structures.drain_filter(|_, y| y.1 == self.depth) {
            global.structures.remove(&v.0).unwrap();
        }

        
        for (_, v) in self.functions.drain_filter(|_, y| y.1 == self.depth) {
            global.functions.remove(&v.0).unwrap();
        }

        self.depth -= 1;

        
        name
    }
}


struct TypeConversionState<'a> {
    types: HashMap<SymbolIndex, DataType>,
    declaration_suffix: SymbolIndex,
    symbol_map: &'a mut SymbolTable,
}


impl TypeConversionState<'_> {
    fn convert_types(&mut self, instructions: &mut [Instruction]) {
        for i in instructions {
            self.convert_type(i);
        }
    }


    fn convert_type(&mut self, instruction: &mut Instruction) {        
        match &mut instruction.instruction_kind {
            InstructionKind::Statement(v)   => self.convert_statement_types(v),
            InstructionKind::Expression(v)  => self.convert_expression_types(v),
            InstructionKind::Declaration(v) => self.convert_declaration_types(v),
        }
    }


    fn convert_statement_types(&mut self, statement: &mut Statement) {
        match statement {
            Statement::DeclareVar { type_hint, data, ..} => {
                if let Some(type_hint) = type_hint {
                    self.convert_data_type(&mut type_hint.data_type);
                }

                self.convert_type(data);
            },

            
            Statement::VariableUpdate { left, right } => {
                self.convert_type(left);
                self.convert_type(right);
            },

            
            Statement::FieldUpdate { structure, right, .. } => {
                self.convert_type(structure);
                self.convert_type(right);
            },

            
            Statement::Loop { body } => {
                body.iter_mut().for_each(|x| self.convert_type(x));
            },

            
            Statement::Return(v) => self.convert_type(v),


            _ => (),
        }
    }


    fn convert_declaration_types(&mut self, declaration: &mut Declaration) {
        match declaration {
            Declaration::FunctionDeclaration { arguments, return_type, body, .. } => {
                self.convert_data_type(&mut return_type.data_type);
                arguments.iter_mut().for_each(|x| self.convert_data_type(&mut x.1.data_type));

                self.convert_types(body);
            },


            Declaration::StructDeclaration { fields, name, generics  } => {
                fields.iter_mut().for_each(|x| self.convert_data_type(&mut x.1.data_type));
            },

            
            Declaration::Namespace { body, .. } => {
                self.convert_types(body);
            },

            
            Declaration::ImplBlock { body, datatype } => {
                self.convert_data_type(&mut datatype.data_type);
                self.convert_types(body);
            },

            
            Declaration::Extern { functions, .. } => {
                functions.iter_mut().for_each(|x| self.convert_data_type(&mut x.return_type.data_type));
            },

            
            Declaration::UseFile { .. } => (),
        }
    }


    fn convert_expression_types(&mut self, expression: &mut Expression) {
        match expression {
            Expression::AsCast { value, cast_type } => {
                self.convert_type(value);
                self.convert_data_type(&mut cast_type.data_type);
            },

            
            Expression::BinaryOp { left, right, ..} => {
                self.convert_type(left);
                self.convert_type(right);
            },

            
            Expression::UnaryOp { value, .. } => self.convert_type(value),
            
            
            Expression::Block { body } => self.convert_types(body),

            
            Expression::IfExpression { body, condition, else_part } => {
                self.convert_types(body);
                self.convert_type(condition);
                if let Some(v) = else_part {
                    self.convert_type(v);
                }
            },

            
            Expression::FunctionCall { arguments, .. } => arguments.iter_mut().for_each(|x| self.convert_type(x)),

            
            Expression::StructureCreation { fields, .. } => fields.iter_mut().for_each(|x| self.convert_type(&mut x.1)),

            
            Expression::AccessStructureData { structure, .. } => self.convert_type(structure),
            Expression::WithinNamespace { do_within, .. } => self.convert_type(do_within),

            
            Expression::Data(_) => (),
            Expression::Identifier(_) => (),
        }
    }


    fn convert_data_type(&self, datatype: &mut DataType) {
        if let DataType::Struct(v) = datatype {
            if let Some(v) = self.types.get(v) {
                *datatype = *v;
            }

            // let n_generics = self.generics_map.get(*generics).clone();
            // n_generics.iter_mut().for_each(|x| self.convert_data_type(&mut x.data_type));
            // let n_generics = self.generics_map.push(n_generics);
            // *generics = n_generics;
            
         }
    }
}


impl GlobalState<'_> {
    #[inline]
    pub fn to_string(&self, data: DataType) -> String {
        format!("'{}'", data.to_string(self.symbol_table))
    }
    
}

