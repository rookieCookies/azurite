use std::process::ExitCode;

use azurite_common::{Bytecode, Data, DataType, FileData};

use crate::{
    ast::{
        binary_operation::BinaryOperator, unary_operation::UnaryOperator, Instruction,
        InstructionType,
    },
    error::Error,
    lexer::lex,
    parser::Parser,
    static_analysis::{AnalysisState, Scope},
};

const NATIVE_LIBRARY: &str = include_str!("../native.az");

pub fn generate_instructions(file_data: &FileData) -> Result<Vec<Instruction>, Vec<Error>> {
    let tokens = lex(file_data.data.chars().collect(), file_data.path.clone())?;

    let instructions = Parser::parse_tokens(tokens, file_data.path.clone())?;

    Ok(instructions)
}

pub fn compile(file_data: FileData) -> Result<Compilation, ExitCode> {
    let process = Instruction {
        instruction_type: InstructionType::Using(file_data.path),
        start: 0,
        end: 0,
        line: 0,
        pop_after: false,
    };

    let mut analyzer_state = AnalysisState::new();

    let mut root_scope = Scope::new_raw(
        FileData::new("root".to_string(), "".to_string()),
        vec![process],
    );

    {
        let native_file_data = FileData::new("::native".to_string(), NATIVE_LIBRARY.to_string());
        let generated_instructions = generate_instructions(&native_file_data);

        let generated_instructions = match generated_instructions {
            Ok(v) => v,
            Err(errs) => {
                errs.into_iter()
                    .for_each(|x| x.trigger(&analyzer_state.loaded_files));
                return Err(ExitCode::FAILURE);
            }
        };

        let mut new_scope = Scope::new_raw(native_file_data, generated_instructions);

        analyzer_state.analyze_scope(&mut new_scope);

        analyzer_state
            .loaded_files
            .insert("::native".to_string(), new_scope);
    }

    analyzer_state.analyze_scope(&mut root_scope);
    if !analyzer_state.errors.is_empty() {
        for error in analyzer_state.errors {
            error.trigger(&analyzer_state.loaded_files);
        }
        return Err(ExitCode::FAILURE);
    }

    let mut compiler = Compilation::new();
    for function in analyzer_state.function_stack {
        let mut instruction = Instruction {
            instruction_type: InstructionType::Data(Data::Bool(false)),
            ..function.instructions
        };

        instruction.instruction_type = InstructionType::FunctionDeclaration {
            identifier: function.identifier,
            body: Box::new(function.instructions),
            arguments: function.arguments,
            return_type: function.return_type,
            inlined: false,
        };

        compiler.compile_to_bytes(instruction);
    }

    compiler.compiled_all_functions = true;
    for i in analyzer_state.loaded_files {
        for instruction in i.1.instructions {
            compiler.compile_to_bytes(instruction);
        }
    }
    compiler.emit_byte(Bytecode::Return as u8, 0);
    Ok(compiler)
}

#[derive(Debug)]
pub struct Compilation {
    pub constants: Vec<Data>,
    pub line_table: Vec<u32>,
    pub bytecode: Vec<u8>,

    #[cfg(feature = "readable")]
    pub text: Vec<String>,

    compiled_all_functions: bool,
    variable_offset: usize,
}

impl Compilation {
    fn new() -> Self {
        Self {
            constants: Vec::with_capacity(256),
            bytecode: Vec::with_capacity(256),
            line_table: Vec::new(),
            compiled_all_functions: false,
            variable_offset: 0,
        }
    }

    fn compile_to_bytes_with_variable_offset(
        &mut self,
        instruction: Instruction,
        offset: usize,
    ) -> Option<u8> {
        let temp = self.variable_offset;
        self.variable_offset = offset;
        let v = self.compile_to_bytes(instruction);
        self.variable_offset = temp;
        v
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn compile_to_bytes(&mut self, instruction: Instruction) -> Option<u8> {
        match instruction.instruction_type {
            InstructionType::BinaryOperation {
                left,
                right,
                operator,
            } => {
                self.compile_to_bytes(*left);
                self.compile_to_bytes(*right);
                let operator_byte = match operator {
                    BinaryOperator::Addition => Bytecode::Add,
                    BinaryOperator::Subtraction => Bytecode::Subtract,
                    BinaryOperator::Multiplication => Bytecode::Multiply,
                    BinaryOperator::Division => Bytecode::Divide,
                    BinaryOperator::EqualsEquals => Bytecode::EqualsTo,
                    BinaryOperator::GreaterThan => Bytecode::GreaterThan,
                    BinaryOperator::LessThan => Bytecode::LesserThan,
                    BinaryOperator::GreaterEquals => Bytecode::GreaterEquals,
                    BinaryOperator::LesserEquals => Bytecode::LesserEquals,
                    BinaryOperator::NotEquals => Bytecode::NotEqualsTo,
                } as u8;

                self.emit_byte(operator_byte, instruction.line);
            }
            InstructionType::Data(v) => {
                self.constants.push(v);
                self.emit_byte(Bytecode::LoadConst as u8, instruction.line);
                self.emit_byte((self.constants.len() - 1) as u8, instruction.line);
            }
            InstructionType::LoadVariable(_, index) => {
                let index = index + self.variable_offset as u16;
                if index > u16::from(u8::MAX) {
                    self.emit_byte(Bytecode::GetVar as u8, instruction.line);
                    let bytes = index.to_le_bytes();
                    self.emit_byte(bytes[0], instruction.line);
                    self.emit_byte(bytes[1], instruction.line);
                } else {
                    self.emit_byte(Bytecode::GetVarFast as u8, instruction.line);
                    self.emit_byte(index as u8, instruction.line);
                }
            }
            InstructionType::UnaryOperation { data, operator } => {
                self.compile_to_bytes(*data);
                match operator {
                    UnaryOperator::Minus => {
                        self.emit_byte(Bytecode::Negative as u8, instruction.line);
                    }
                    UnaryOperator::Not => self.emit_byte(Bytecode::Not as u8, instruction.line),
                }
            }
            InstructionType::DeclareVariable {
                data, overwrite, ..
            } => {
                self.compile_to_bytes(*data);
                if let Some(index) = overwrite {
                    if index > u16::from(u8::MAX) {
                        self.emit_byte(Bytecode::ReplaceVar as u8, instruction.line);

                        let bytes = index.to_le_bytes();
                        self.emit_byte(bytes[0], instruction.line);
                        self.emit_byte(bytes[1], instruction.line);
                    } else {
                        self.emit_byte(Bytecode::ReplaceVarFast as u8, instruction.line);
                        self.emit_byte(index as u8, instruction.line);
                    }
                }
            }
            InstructionType::UpdateVarOnStack { data, index, .. } => {
                let index = index + self.variable_offset as u16;
                self.compile_to_bytes(*data);

                if index > u16::from(u8::MAX) {
                    self.emit_byte(Bytecode::ReplaceVar as u8, instruction.line);

                    let bytes = index.to_le_bytes();
                    self.emit_byte(bytes[0], instruction.line);
                    self.emit_byte(bytes[1], instruction.line);
                } else {
                    self.emit_byte(Bytecode::ReplaceVarFast as u8, instruction.line);

                    self.emit_byte(index as u8, instruction.line);
                }
            }
            InstructionType::Block { body, pop } => {
                let mut other = Vec::with_capacity(body.len());
                for i in body {
                    match i.instruction_type {
                        InstructionType::FunctionDeclaration { .. } => {
                            self.compile_to_bytes(i);
                        }
                        _ => other.push(i),
                    }
                }
                for i in other {
                    self.compile_to_bytes(i);
                }

                match pop {
                    0 => (),
                    1 => self.emit_byte(Bytecode::Pop as u8, instruction.line),
                    _ => {
                        self.emit_byte(Bytecode::PopMulti as u8, instruction.line);

                        self.emit_byte(pop as u8, instruction.line);
                    }
                }
            }
            InstructionType::IfExpression {
                body,
                condition,
                else_part,
            } => {
                self.compile_to_bytes(*condition);
                self.emit_byte(Bytecode::JumpIfFalse as u8, instruction.line);

                let start = self.bytecode.len();
                self.emit_byte(0, instruction.line); // placeholder

                self.compile_to_bytes(*body);
                if let Some(x) = else_part {
                    self.emit_byte(Bytecode::Jump as u8, x.line);

                    let start_of_jump = self.bytecode.len();
                    self.emit_byte(0, instruction.line);

                    self.bytecode[start] = (self.bytecode.len() - start - 1) as u8; // if its false we jump here and execute the else branch

                    self.compile_to_bytes(*x);

                    self.bytecode[start_of_jump] = (self.bytecode.len() - start_of_jump - 1) as u8;
                    // if its true we just jump over the else branch
                } else {
                    self.bytecode[start] = (self.bytecode.len() - start - if instruction.pop_after { 0 } else { 1 }) as u8;
                }
            }
            InstructionType::WhileStatement { condition, body } => {
                let start_of_loop = self.bytecode.len();
                self.compile_to_bytes(*condition);
                self.emit_byte(Bytecode::JumpIfFalse as u8, instruction.line);
                let start_of_loop_skip = self.bytecode.len();
                self.emit_byte(0, instruction.line); // placeholder

                self.compile_to_bytes(*body);

                self.emit_byte(Bytecode::JumpBack as u8, instruction.line);
                self.emit_byte(
                    (self.bytecode.len() - start_of_loop + 1) as u8,
                    instruction.line,
                );
                self.bytecode[start_of_loop_skip] =
                    (self.bytecode.len() - start_of_loop_skip - 1) as u8;
            }
            InstructionType::FunctionDeclaration {
                body,
                arguments,
                return_type,
                inlined: _,
                identifier: _,
                ..
            } => {
                if self.compiled_all_functions {
                    return None;
                }
                // TODO: Remove the inlined function fom the bytecode
                let _function_creation = self.bytecode.len();
                self.emit_byte(Bytecode::LoadFunction as u8, instruction.line);
                self.emit_byte(arguments.len() as u8, instruction.line);
                self.emit_byte(u8::from(return_type != DataType::Empty), instruction.line);
                let start = self.bytecode.len();
                self.emit_byte(0, instruction.line); // the amount

                self.compile_to_bytes(*body);

                self.emit_byte(Bytecode::Return as u8, instruction.line);
                self.bytecode[start] = (self.bytecode.len() - start - 1) as u8;
            }
            InstructionType::FunctionCall {
                index,
                arguments,
                identifier: _,
                created_by_accessing: _,
            } => {
                let argument_count = arguments.len();
                for x in arguments {
                    self.compile_to_bytes(x);
                }

                match index {
                    crate::ast::FunctionInline::None(x) => {
                        self.emit_byte(Bytecode::CallFunction as u8, instruction.line);
                        self.emit_byte(x as u8, instruction.line);
                    }
                    crate::ast::FunctionInline::Inline {
                        instructions,
                        variable_offset,
                        has_return,
                    } => {
                        self.compile_to_bytes_with_variable_offset(*instructions, variable_offset);
                        if has_return {
                            self.emit_byte(
                                Bytecode::ReturnWithoutCallStack as u8,
                                instruction.line,
                            );
                            self.emit_byte(argument_count as u8, instruction.line);
                        } else {
                            match argument_count {
                                0 => (),
                                1 => self.emit_byte(Bytecode::Pop as u8, instruction.line),
                                _ => {
                                    self.emit_byte(Bytecode::PopMulti as u8, instruction.line);

                                    self.emit_byte(argument_count as u8, instruction.line);
                                }
                            }
                        }
                    }
                }
            }
            InstructionType::Return(optional_return_value) => {
                if let Some(return_value) = optional_return_value {
                    self.compile_to_bytes(*return_value);
                }
                self.emit_byte(Bytecode::ReturnFromFunction as u8, instruction.line);
            }
            InstructionType::CreateStruct { variables, .. } => {
                let amount = variables.len();
                for (_, instruction) in variables {
                    self.compile_to_bytes(instruction);
                }
                self.emit_byte(Bytecode::CreateStruct as u8, instruction.line);
                self.emit_byte(amount as u8, instruction.line);
            }
            InstructionType::AccessVariable { data, id, .. } => {
                self.compile_to_bytes(*data);
                self.emit_byte(Bytecode::AccessData as u8, instruction.line);
                self.emit_byte(id as u8, instruction.line);
            }
            InstructionType::RawCall(v) => {
                self.emit_byte(Bytecode::RawCall as u8, instruction.line);
                self.emit_byte(v as u8, instruction.line);
            }
            _ => (),
        }
        if instruction.pop_after {
            self.emit_byte(Bytecode::Pop as u8, instruction.line);
        }
        None
    }

    fn emit_byte(&mut self, byte: u8, line: u32) {
        // if let std::collections::btree_map::Entry::Vacant(e) = self.line_table.entry(line) {
        //     e.insert(1);
        // } else {
        //     *self.line_table.get_mut(&line).unwrap() += 1;
        // }
        self.line_table.push(line);
        self.bytecode.push(byte);
    }
}
