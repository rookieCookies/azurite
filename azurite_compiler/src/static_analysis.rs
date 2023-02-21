use std::{
    collections::HashMap,
    fs::File,
    io::Read,
};

use azurite_common::{DataType, FileData};

use crate::{
    ast::{
        binary_operation::BinaryOperator, unary_operation::UnaryOperator, FunctionInline,
        Instruction, InstructionType,
    },
    compiler::generate_instructions,
    error::{Error, Highlight, FATAL},
};

#[derive(Debug)]
pub struct AnalysisState {
    pub errors: Vec<Error>,
    pub loaded_files: HashMap<String, Scope>,

    pub function_stack: Vec<Function>,
    pub inline_functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub identifier: String,
    pub instructions: Instruction,
    is_static: bool,
    pub arguments: Vec<(String, DataType)>,
    pub return_type: DataType,
}

#[derive(Debug)]
pub struct Scope {
    pub current_file: FileData,
    pub instructions: Vec<Instruction>,

    pub stack_emulation: Vec<DataType>,

    pub variable_map: HashMap<String, usize>,
    pub function_map: HashMap<String, (usize, bool)>,
    pub structure_map: HashMap<String, Vec<(String, DataType)>>,
}

impl Scope {
    pub fn new(
        state: &AnalysisState,
        current_file: FileData,
        instructions: Vec<Instruction>,
    ) -> Self {
        let native = match state.loaded_files.get("::native") {
            Some(v) => v,
            None => return Self::new_raw(current_file, instructions),
        };
        Self {
            current_file,
            instructions,
            stack_emulation: native.stack_emulation.clone(),
            variable_map: native.variable_map.clone(),
            function_map: native.function_map.clone(),
            structure_map: native.structure_map.clone(),
        }
    }

    pub fn new_raw(current_file: FileData, instructions: Vec<Instruction>) -> Self {
        Self {
            current_file,
            instructions,
            stack_emulation: Vec::new(),
            variable_map: HashMap::new(),
            function_map: HashMap::new(),
            structure_map: HashMap::new(),
        }
    }
}

// TODO: Maybe make the multi-file-loading multi-threaded

impl AnalysisState {
    pub fn analyze(
        &mut self,
        scope: &mut Scope,
        instruction: &mut Instruction,
    ) -> DataType {
        self.analyze_with_type_hint(scope, instruction, None)
    }

    fn analyze_function_definition(
        &mut self,
        scope: &mut Scope,
        function_declaration: &mut Instruction,
    ) {
        let (identifier, arguments, return_type, is_inlined, body) =
            match &function_declaration.instruction_type {
                InstructionType::FunctionDeclaration {
                    identifier,
                    arguments,
                    return_type,
                    inlined,
                    body,
                } => (identifier, arguments, return_type, *inlined, body),
                _ => panic!(),
            };

        let function = Function {
            identifier: identifier.clone(),
            instructions: *body.clone(),
            is_static: {
                if let Some(x) = arguments.get(0) {
                    x.0.as_str() != "self"
                } else {
                    true
                }
            },
            arguments: arguments.clone(),
            return_type: return_type.clone(),
        };
        // println!("Hello \n|>{:?}", scope.function_map);
        if is_inlined {
            scope
                .function_map
                .insert(identifier.clone(), (self.inline_functions.len(), true));
            self.inline_functions.push(function);
            return;
        }
        scope
            .function_map
            .insert(identifier.clone(), (self.function_stack.len(), false));
        self.function_stack.push(function);
    }

    pub fn analyze_scope(&mut self, scope: &mut Scope) -> DataType {
        self.analyze_scope_with_hint(scope, &None, false).0
    }

    pub fn analyze_scope_with_hint(
        &mut self,
        scope: &mut Scope,
        hint: &Option<DataType>,
        dont_pop_last: bool,
    ) -> (DataType, bool) {
        let mut instructions = std::mem::take(&mut scope.instructions);
        let mut return_type = DataType::Empty;
        for instruction in &mut instructions {
            match &mut instruction.instruction_type {
                InstructionType::StructDeclaration { .. } => {
                    self.analyze_struct_definition(scope, instruction);
                }
                _ => continue,
            }
        }
        for instruction in &mut instructions {
            match &mut instruction.instruction_type {
                InstructionType::FunctionDeclaration { .. } => {
                    self.analyze_function_definition(scope, instruction);
                }
                InstructionType::ImplBlock { functions, .. } => functions
                    .iter_mut()
                    .for_each(|x| self.analyze_function_definition(scope, x)),
                _ => continue,
            }
        }

        for instruction in &mut instructions {
            return_type = self.analyze_with_type_hint(scope, instruction, hint.clone());
        }
        

        if dont_pop_last {
            if let Some(v) = instructions.last_mut() {
                v.pop_after = false;
            }
        // } else {
        //     false
        };

        debug_assert!(scope.instructions.is_empty());
        scope.instructions = std::mem::take(&mut instructions);
        (return_type, true)
    }

    fn analyze_struct_definition(
        &mut self,
        scope: &mut Scope,
        structure_declaration: &mut Instruction,
    ) {
        let (identifier, fields) = match &mut structure_declaration.instruction_type {
            InstructionType::StructDeclaration { identifier, fields } => (identifier, fields),
            _ => panic!(),
        };

        if scope.structure_map.contains_key(identifier) {
            self.errors.push(error_structure_already_exists(
                scope,
                (structure_declaration.start, structure_declaration.end),
            ));
        }

        fields.sort_by_key(|x| x.0.clone());
        scope
            .structure_map
            .insert(identifier.clone(), fields.clone());
    }

    /// # Panics
    /// # Errors
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn analyze_with_type_hint(
        &mut self,
        scope: &mut Scope,
        instruction: &mut Instruction,
        hint: Option<DataType>,
    ) -> DataType {
        let mut return_type = DataType::Empty;
        match &mut instruction.instruction_type {
            InstructionType::Using(file_name) => {
                if let Some(loaded_file) = self.loaded_files.get(file_name) {
                    scope.function_map.extend(loaded_file.function_map.clone());
                    scope.variable_map.extend(loaded_file.variable_map.clone());
                    scope
                        .structure_map
                        .extend(loaded_file.structure_map.clone());
                    return return_type;
                }
                let mut file = if let Ok(v) = File::open(&file_name) {
                    v
                } else {
                    self.errors.push(error_unable_to_locate_file(
                        scope,
                        (instruction.start, instruction.end),
                        file_name,
                    ));
                    return return_type;
                };

                let mut file_buffer = String::new();
                if file.read_to_string(&mut file_buffer).is_err() {
                    self.errors.push(error_unable_to_read_file(
                        scope,
                        (instruction.start, instruction.end),
                        file_name,
                    ));
                    return return_type;
                };

                let file_data = FileData {
                    path: file_name.clone(),
                    data: file_buffer,
                };

                let generated_instructions = generate_instructions(&file_data);

                let generated_instructions = match generated_instructions {
                    Ok(v) => v,
                    Err(mut errs) => {
                        self.errors.append(&mut errs);
                        vec![]
                    }
                };

                let mut new_scope =
                    Scope::new(self, file_data, generated_instructions);

                self.analyze_scope(&mut new_scope);
                self.loaded_files.insert(file_name.clone(), new_scope);

                // Re-run this function but since it exists
                // it will get stucked and just load the file in
                self.analyze(scope, instruction);
            }
            InstructionType::Data(v) => return_type = v.type_representation(),
            InstructionType::DeclareVariable {
                identifier,
                data,
                type_declaration,
                overwrite: _,
            } => {
                // we place a placeholder value so even if
                // the rest of this fails and early returns
                // the rest of the analysis assumes the variable
                // at least exists
                let is_overriding = scope.variable_map.insert(identifier.clone(), usize::MAX);
                let type_of_data = self.analyze(scope, data);
                let type_of_variable = match type_declaration {
                    Some(v) => {
                        if type_of_data != *v {
                            self.errors.push(error_explicit_type_and_value_differ(
                                scope,
                                (instruction.start, instruction.end),
                                v,
                                &type_of_data,
                            ));
                            return return_type;
                        }
                        v.clone()
                    }
                    None => type_of_data,
                };

                if let Some(index) = is_overriding {
                        scope.variable_map.insert(identifier.clone(), index);
                } else {
                    scope.stack_emulation.push(type_of_variable);
                    let index = scope.stack_emulation.len() - 1; // top
                    scope.variable_map.insert(identifier.clone(), index);
                }
            }
            InstructionType::LoadVariable(identifier, index) => {
                let variable_index = if let Some(variable_index) = scope.variable_map.get(identifier) {
                    *variable_index
                } else {
                    self.errors.push(error_variable_doesnt_exist(
                        scope,
                        (instruction.start, instruction.end),
                        identifier,
                    ));
                    return return_type;
                };
                return_type = scope.stack_emulation[variable_index].clone();
                *index = variable_index as u16;
            }
            InstructionType::UpdateVarOnStack {
                identifier,
                data,
                index,
            } => {
                let variable_index = if let Some(variable_index) = scope.variable_map.get(identifier) {
                    *variable_index
                } else {
                    self.errors.push(error_variable_doesnt_exist(
                        scope,
                        (instruction.start, instruction.end),
                        identifier,
                    ));
                    return return_type;
                };

                let type_of_data = self.analyze(scope, data);
                let type_of_variable = scope.stack_emulation.get(variable_index).unwrap();

                if type_of_data != *type_of_variable {
                    self.errors.push(error_variable_type_and_value_type_differ(
                        scope,
                        (instruction.start, instruction.end),
                        identifier,
                        type_of_variable,
                        &type_of_data,
                    ));
                }

                *index = variable_index as u16;
            }
            InstructionType::Block { body, pop } => {
                let mut new_scope = Scope::new(
                    self,
                    std::mem::take(&mut scope.current_file),
                    std::mem::take(body),
                );
                new_scope.variable_map = scope.variable_map.clone();
                new_scope.function_map = scope.function_map.clone();
                new_scope.structure_map = scope.structure_map.clone();
                new_scope.stack_emulation = scope.stack_emulation.clone();

                let (rt, _) = self.analyze_scope_with_hint(&mut new_scope, &hint, true);
                // if instruction.pop_after && rt == DataType::Empty {
                //     println!("{:#?}", body);
                //     instruction.pop_after = false;
                // } else {
                //     println!("{rt}");
                // }
                return_type = rt;
                *pop = (new_scope.stack_emulation.len() - scope.stack_emulation.len()) as u16;
                // .max(0) as u16;
                scope.current_file = std::mem::take(&mut new_scope.current_file);
                *body = new_scope.instructions;
                
            }
            InstructionType::IfExpression {
                condition,
                body,
                else_part,
            } => {
                let condition_type = self.analyze(scope, condition);
                if condition_type != DataType::Bool {
                    self.errors.push(error_non_expected_type(
                        scope,
                        (condition.start, condition.end),
                        &DataType::Bool,
                        &condition_type,
                    ));
                }

                return_type = self.analyze(scope, body);

                match else_part {
                    Some(else_part) => {
                        let else_type = self.analyze(scope, else_part);
                        if return_type != else_type {
                            self.errors.push(error_else_clause_isnt_of_type(
                                scope,
                                (instruction.start, else_part.end),
                                &return_type,
                                &else_type,
                            ));
                        }
                    }
                    None => (),
                }
            }
            InstructionType::BinaryOperation {
                left,
                right,
                operator,
            } => {
                let left_type = self.analyze(scope, left);
                let right_type = self.analyze(scope, right);

                if let Some(v) = type_check_binary_operation(
                    &left_type,
                    &right_type,
                    operator,
                    &scope.current_file.path,
                    (left.start, right.end),
                ) {
                    self.errors.push(v);
                }

                return_type = match operator {
                    BinaryOperator::Addition
                    | BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division => match (&left_type, &right_type) {
                        (DataType::Empty, _) => right_type,
                        _ => left_type,
                    },
                    BinaryOperator::EqualsEquals
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::LessThan
                    | BinaryOperator::GreaterEquals
                    | BinaryOperator::LesserEquals
                    | BinaryOperator::NotEquals => DataType::Bool,
                }
            }
            InstructionType::UnaryOperation { data, operator } => {
                let data_type = self.analyze(scope, data);
                match (&operator, &data_type) {
                    (UnaryOperator::Minus, DataType::Integer | DataType::Float) | (UnaryOperator::Not, DataType::Bool) => (),
                    _ => {
                        let expected = match operator {
                            UnaryOperator::Minus => DataType::Integer,
                            UnaryOperator::Not => todo!(),
                        };
                        self.errors.push(error_non_expected_type(
                            scope,
                            (instruction.start, instruction.end),
                            &expected,
                            &data_type,
                        ));
                    }
                }
                return_type = data_type;
            }
            InstructionType::Return(Some(v)) => return_type = self.analyze(scope, v),
            InstructionType::WhileStatement { condition, body } => {
                let condition_type = self.analyze(scope, condition);
                if condition_type != DataType::Bool {
                    error_non_expected_type(
                        scope,
                        (condition.start, condition.end),
                        &DataType::Bool,
                        &condition_type,
                    );
                }

                self.analyze(scope, body);
            }
            InstructionType::FunctionDeclaration {
                identifier,
                body,
                arguments,
                return_type: function_return_type,
                inlined,
            } => {
                return_type = function_return_type.clone();
                let mut function_scope = Scope::new(
                    self,
                    std::mem::take(&mut scope.current_file),
                    vec![*body.clone()],
                );
                function_scope.variable_map = arguments
                    .iter()
                    .enumerate()
                    .map(|x| (x.1 .0.clone(), x.0))
                    .collect();
                function_scope.function_map = scope.function_map.clone();
                function_scope.stack_emulation = arguments
                    .iter()
                    .enumerate()
                    .map(|x| x.1 .1.clone())
                    .collect();

                let body_return_type = self
                    .analyze_scope_with_hint(&mut function_scope, &Some(return_type.clone()), true)
                    .0;

                scope.current_file = std::mem::take(&mut function_scope.current_file);
                if body_return_type != *function_return_type {
                    self.errors.push(error_function_return_type_is_different(
                        scope,
                        (instruction.start, instruction.end),
                        function_return_type,
                        &body_return_type,
                    ));
                }

                let mut instruction = function_scope.instructions.remove(0);
                match &mut instruction.instruction_type {
                    InstructionType::Block { body: _, pop: _ } => {
                        // *pop += arguments.len() as u16;
                    }
                    _ => panic!()
                }


                let function_index = scope.function_map.get(identifier).unwrap();
                match inlined {
                    true => &mut self.inline_functions,
                    false => &mut self.function_stack,
                }
                .get_mut(function_index.0)
                .unwrap()
                .instructions = instruction;
            }
            InstructionType::FunctionCall {
                identifier,
                arguments,
                index,
                created_by_accessing,
            } => {
                if *created_by_accessing {
                    let self_type = self.analyze(scope, &mut arguments[0]);
                    if !identifier.contains("::") {
                        *identifier = format!("{self_type}::{identifier}");
                    }
                }
                let function_meta = if let Some(v) = scope.function_map.get(identifier) {
                    v
                } else {
                    self.errors.push(error_function_isnt_declared(
                        scope,
                        (instruction.start, instruction.end),
                        identifier,
                    ));
                    return return_type;
                };

                let function = if function_meta.1 {
                    let function = self.inline_functions[function_meta.0].clone();
                    *index = FunctionInline::Inline {
                        instructions: Box::new(function.instructions.clone()),
                        variable_offset: scope.stack_emulation.len(),
                    };
                    function
                } else {
                    *index = FunctionInline::None(function_meta.0);
                    self.function_stack[function_meta.0].clone()
                };

                return_type = function.return_type.clone();
                // instruction.pop_after = return_type == DataType::Empty;

                if *created_by_accessing && function.is_static {
                    self.errors
                        .push(error_static_function_accessed_non_statically(
                            scope,
                            (instruction.start, instruction.end),
                        ));
                    return return_type;
                }

                if function.arguments.len() != arguments.len() {
                    self.errors.push(error_invalid_function_argument_amount(
                        scope,
                        (instruction.start, instruction.end),
                        function.arguments.len(),
                        arguments.len(),
                    ));
                    return return_type;
                }

                for (index, argument) in arguments.iter_mut().enumerate() {
                    let argument_type = self.analyze(scope, argument);
                    if argument_type == function.arguments[index].1 {
                        continue;
                    }
                    if index == 0 && *created_by_accessing {
                        self.errors.push(error_function_doesnt_exist_for_type(
                            scope,
                            (instruction.start, instruction.end),
                            identifier,
                            &argument_type,
                        ));
                        continue;
                    }
                    self.errors.push(error_function_arguments_differ_in_type(
                        scope,
                        (argument.start, argument.end),
                        &function.arguments[index].1,
                        &argument_type,
                    ));
                }
            }
            InstructionType::StructDeclaration { identifier: _, fields } => {
                for (_, datatype) in fields.iter() {
                    match datatype {
                        DataType::Struct(identifier) => {
                            if !scope.structure_map.contains_key(identifier) {
                                self.errors.push(error_structure_field_type_doesnt_exist(
                                    scope,
                                    (instruction.start, instruction.end),
                                    datatype,
                                ));
                            }
                        }
                        _ => continue,
                    }
                }
            }
            InstructionType::CreateStruct {
                identifier,
                variables,
            } => {
                return_type = DataType::Struct(identifier.clone());
                let structure_fields = if let Some(v) = scope.structure_map.get(identifier) {
                    v
                } else {
                    self.errors.push(error_structure_doesnt_exist(
                        scope,
                        (instruction.start, instruction.end),
                    ));
                    return return_type;
                };

                let existing_fields = structure_fields.iter().cloned().collect::<HashMap<_, _>>();
                let mut variable_map = std::mem::take(variables)
                    .into_iter()
                    .collect::<HashMap<_, _>>();

                for (variable_identifier, variable_data) in &mut variable_map {
                    let field_type = if let Some(v) = existing_fields.get(variable_identifier) {
                        v
                    } else {
                        self.errors.push(error_structure_field_doesnt_exist(
                            scope,
                            (variable_data.start, variable_data.end),
                            identifier,
                            variable_identifier,
                        ));
                        continue;
                    };

                    let variable_type = self.analyze(scope, variable_data);

                    if variable_type != *field_type {
                        self.errors.push(error_structure_fields_differ_in_type(
                            scope,
                            (variable_data.start, variable_data.end),
                            identifier,
                            variable_identifier,
                            field_type,
                            &variable_type,
                        ));
                    }
                }

                let missing: Vec<_> = existing_fields
                    .iter()
                    .filter(|x| variable_map.contains_key(x.0))
                    .map(|x| x.0)
                    .collect();
                if !missing.is_empty() {
                    self.errors.push(error_structure_missing_fields(
                        scope,
                        (instruction.start, instruction.end),
                        &missing,
                    ));
                }
                *variables = std::mem::take(&mut variable_map).into_iter().collect();
                variables.sort_by_key(|x| x.0.clone());
            }
            InstructionType::ImplBlock {
                datatype,
                functions,
            } => {
                match datatype {
                    DataType::Integer
                    | DataType::Float
                    | DataType::String
                    | DataType::Bool
                    | DataType::Empty => (),
                    DataType::Struct(identifier) => {
                        if !scope.structure_map.contains_key(identifier) {
                            self.errors.push(error_structure_doesnt_exist(
                                scope,
                                (instruction.start, instruction.end),
                            ));
                        }
                    }
                }

                for i in functions.iter_mut() {
                    self.analyze(scope, i);
                }
            }
            InstructionType::RawCall(_) => {
                return_type = hint.unwrap_or(DataType::Empty);
            },
            _ => (),
        }
        return_type
    }

    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            loaded_files: HashMap::new(),
            function_stack: Vec::new(),
            inline_functions: Vec::new(),
        }
    }
}

fn error_unable_to_locate_file(
    scope: &Scope,
    (start, end): (u32, u32),
    file_name: &String,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "unable to locate file",
        format!("unable to locate the file at {file_name}"),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_unable_to_read_file(
    scope: &Scope,
    (start, end): (u32, u32),
    file_name: &String,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "unable to read file",
        format!("unable to read the file at {file_name}"),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_explicit_type_and_value_differ(
    scope: &Scope,
    (start, end): (u32, u32),
    expected: &DataType,
    found: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "explicit type and assigned value differ in type",
        format!(
            "this variable is explicitly defined as {expected} but the assigned data is of type {found}",
        ),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_variable_doesnt_exist(
    scope: &Scope,
    (start, end): (u32, u32),
    identifier: &String,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "variable doesn't exist",
        format!("the variable {identifier} doesn't exist in the current scope",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_variable_type_and_value_type_differ(
    scope: &Scope,
    (start, end): (u32, u32),
    identifier: &String,
    variable_type: &DataType,
    assigned_type: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "variable doesn't exist",
        format!(
            "the variable {identifier} is of type {variable_type} but the value assigned to it is of type {assigned_type}, consider trying re-declaring the variable with \"var {identifier} = ...\"",
        ),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_non_expected_type(
    scope: &Scope,
    (start, end): (u32, u32),
    expected: &DataType,
    found: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "variable doesn't exist",
        format!("this expression expects a {expected} but it is provided a {found}",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_else_clause_isnt_of_type(
    scope: &Scope,
    (start, end): (u32, u32),
    expected: &DataType,
    found: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "if branches return differ in return types",
        format!("the if statement returns {expected} but the else clause returns {found}",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_function_return_type_is_different(
    scope: &Scope,
    (start, end): (u32, u32),
    function_return: &DataType,
    found_return: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "function return value differns with declaration",
        format!(
            "the function body returns {found_return} but the function declaration expects {function_return}",
        ),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_function_isnt_declared(
    scope: &Scope,
    (start, end): (u32, u32),
    identifier: &String,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "function doesn't exist",
        format!("function {identifier} isn't declared prior to this point",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_static_function_accessed_non_statically(
    scope: &Scope,
    (start, end): (u32, u32),
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "static function accessed non-statically",
        "this function is a static function but you're trying to access it via a reference"
            .to_string(),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_invalid_function_argument_amount(
    scope: &Scope,
    (start, end): (u32, u32),
    expected: usize,
    found: usize,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "invalid function argument amount",
        format!("this function accepts {expected} arguments but you've provided {found}",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_function_doesnt_exist_for_type(
    scope: &Scope,
    (start, end): (u32, u32),
    identifier: &String,
    accessor_type: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "function doesn't exist",
        format!("the function {identifier} doesn't exist for type {accessor_type}",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_function_arguments_differ_in_type(
    scope: &Scope,
    (start, end): (u32, u32),
    expected: &DataType,
    found: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "function argument is of invalid type",
        format!("this argument is of type {expected} but you've provided {found}",),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_structure_already_exists(scope: &Scope, (start, end): (u32, u32)) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "struct with the same name already exists in scope",
        "try changing the name of the structure".to_string(),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_structure_doesnt_exist(scope: &Scope, (start, end): (u32, u32)) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "structure doesn't exist",
        "structure isn't declared prior to this point".to_string(),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_structure_field_doesnt_exist(
    scope: &Scope,
    (start, end): (u32, u32),
    structure_identifier: &String,
    field: &String,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "structure field doesn't exist",
        format!("the structure {structure_identifier} does not have a field named {field}"),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_structure_fields_differ_in_type(
    scope: &Scope,
    (start, end): (u32, u32),
    structure_identifier: &String,
    field: &String,
    field_type: &DataType,
    found_field_type: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "structure field doesn't exist",
        format!("the field {field} of {structure_identifier} is of type {field_type} but the given type is {found_field_type}"),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_structure_missing_fields(
    scope: &Scope,
    (start, end): (u32, u32),
    missing: &[&String],
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "structure field doesn't exist",
        if missing.len() == 1 {
            format!("missing {}", missing[0])
        } else {
            format!(
                "{}and {} are missing",
                (0..missing.len() - 1)
                    .map(|x| format!("{}, ", missing[x]))
                    .collect::<String>(),
                missing.last().unwrap()
            )
        },
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn error_structure_field_type_doesnt_exist(
    scope: &Scope,
    (start, end): (u32, u32),
    datatype: &DataType,
) -> Error {
    Error::new(
        vec![(start, end, Highlight::Red)],
        "type of field does not exist",
        format!("{datatype} is not a valid type"),
        &FATAL,
        scope.current_file.path.clone(),
    )
}

fn type_check_binary_operation(
    left: &DataType,
    right: &DataType,
    operator: &BinaryOperator,
    file_name: &str,
    positions: (u32, u32),
) -> Option<Error> {
    match operator {
        BinaryOperator::Addition
        | BinaryOperator::Subtraction
        | BinaryOperator::Multiplication
        | BinaryOperator::Division => {
            type_check_binary_operation_arithmetic(left, right, operator, file_name, positions)
        }
        BinaryOperator::EqualsEquals | BinaryOperator::NotEquals => {
            type_check_binary_operation_equality(left, right, file_name, positions)
        }
        BinaryOperator::GreaterThan
        | BinaryOperator::LessThan
        | BinaryOperator::GreaterEquals
        | BinaryOperator::LesserEquals => {
            type_check_binary_operation_order(left, right, file_name, positions)
        }
    }
}

fn type_check_binary_operation_arithmetic(
    left: &DataType,
    right: &DataType,
    operation: &BinaryOperator,
    file_name: &str,
    (start, end): (u32, u32),
) -> Option<Error> {
    match (left, right) {
        (DataType::Integer, DataType::Integer) | (DataType::Float, DataType::Float) => None,

        _ => Some(Error::new(
            vec![(start, end, Highlight::Red)],
            "invalid binary operation",
            format!(
                "can't {operation} a {left} and a {right} together, consider casting one of them",
            ),
            &FATAL,
            file_name.to_owned(),
        )),
    }
}

fn type_check_binary_operation_equality(
    left: &DataType,
    right: &DataType,
    file_name: &str,
    (start, end): (u32, u32),
) -> Option<Error> {
    if left != right {
        return Some(Error::new(
            vec![(start, end, Highlight::Red)],
            "different type equality check",
            format!("can't check equality between values of different types ({left}, {right})",),
            &FATAL,
            file_name.to_owned(),
        ));
    }
    None
}

fn type_check_binary_operation_order(
    left: &DataType,
    right: &DataType,
    file_name: &str,
    (start, end): (u32, u32),
) -> Option<Error> {
    match (
        left,
        right,
    ) {
        (DataType::Integer, DataType::Integer) | (DataType::Float, DataType::Float) => None,

        _ => Some(Error::new(
            vec![(
                start,
                end,
                Highlight::Red,
            )],
            "invalid order operation",
            format!(
                "can't check order between values of type {left} and {right}, consider casting one of them",
            ),
            &FATAL,
            file_name.to_owned(),
        )),
    }
}
