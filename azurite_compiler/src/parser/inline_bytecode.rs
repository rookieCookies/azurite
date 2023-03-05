use crate::{parser::Parser, lexer::TokenType, ast::{Instruction, InstructionType}, error::{Error, Highlight, FATAL}};

macro_rules! continue_if_none {
    ($v:expr) => { 
        match $v {
            Some(v) => v,
            None => continue,
        }
    }
}

impl Parser {
    // bytecode-statement:
    // |> 'bytecode' '{' bytecode '}'
    pub(super) fn bytecode_statement(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Bytecode)?;
        self.expect_and_advance(&TokenType::LeftCurly)?;
        let mut bytecode_instructions = vec![];
        while let Some(token) = self.current_token() {
            if token.token_type == TokenType::RightCurly || token.token_type == TokenType::EndOfFile {
                break
            }
            let identifier = self.expect_identifier()?.clone();
            
            let context = self.context_of_current_token()?;

            let instruction = match identifier.as_str() {
                "eq"    => BytecodeInstructions::Equals,
                "neq"   => BytecodeInstructions::NotEquals,
                "gt"    => BytecodeInstructions::GreaterThan,
                "lt"    => BytecodeInstructions::LessThan,
                "ge"    => BytecodeInstructions::GreaterEquals,
                "le"    => BytecodeInstructions::LesserEquals,
                "jmp"   => BytecodeInstructions::Jump(continue_if_none!(self.inline_bytecode_index_value())),
                "jif"   => BytecodeInstructions::JumpIfFalse(continue_if_none!(self.inline_bytecode_index_value())),
                "bjmp"  => BytecodeInstructions::BackJump(continue_if_none!(self.inline_bytecode_index_value())),
                "jmpl"  => BytecodeInstructions::JumpLarge(continue_if_none!(self.inline_bytecode_index_value())),
                "jifl"  => BytecodeInstructions::JumpIfFalseLarge(continue_if_none!(self.inline_bytecode_index_value())),
                "bjmpl" => BytecodeInstructions::BackJumpLarge(continue_if_none!(self.inline_bytecode_index_value())),
                "add"   => BytecodeInstructions::Add,
                "sub"   => BytecodeInstructions::Subtract,
                "mul"   => BytecodeInstructions::Multiply,
                "div"   => BytecodeInstructions::Division,
                "takef" => BytecodeInstructions::TakeFast(continue_if_none!(self.inline_bytecode_index_value())),
                "take"  => BytecodeInstructions::Take(continue_if_none!(self.inline_bytecode_index_value())),
                "repf"  => BytecodeInstructions::ReplaceFast(continue_if_none!(self.inline_bytecode_index_value())),
                "rep"   => BytecodeInstructions::Replace(continue_if_none!(self.inline_bytecode_index_value())),
                "not"   => BytecodeInstructions::Not,
                "neg"   => BytecodeInstructions::Negate,
                "raw"   => BytecodeInstructions::Raw(continue_if_none!(self.inline_bytecode_index_value())),
                "rot"   => BytecodeInstructions::Rotate,
                "over"  => BytecodeInstructions::Over,
                "swap"  => BytecodeInstructions::Swap,
                "dup"   => BytecodeInstructions::Duplicate,
                _       => {
                    self.errors.push(Error::new(
                        vec![(context.0, context.1, Highlight::Red)],
                        "invalid bytecode instruction",
                        "refer to the documentation for valid instructions".to_owned(),
                        &FATAL,
                        self.current_file.clone(),
                    ));
                    return None
                },
            };

            bytecode_instructions.push(instruction);
            self.advance();
        }
        self.expect(&TokenType::RightCurly)?;
        Some(Instruction {
            instruction_type: InstructionType::InlineBytecode { bytecode: bytecode_instructions },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    fn inline_bytecode_index_value<T: MaxValue + TryFrom<i64>>(&mut self) -> Option<T> {
        self.advance();
        let context = self.context_of_current_token()?;
        self.expect(&TokenType::Integer(0))?;
        let number = match self.current_token().unwrap().token_type {
            TokenType::Integer(v) => v,
            _ => unreachable!(),
        };
        if let Ok(v) = number.try_into() {
            Some(v)
        } else {
            self.errors.push(Error::new(
                vec![(context.0, context.1, Highlight::Red)],
                "index value too big",
                format!("this instruction requires a value under {}", T::max()),
                &FATAL,
                self.current_file.clone(),
            ));
            self.advance();
            None
        }
    }
}

trait MaxValue {
    fn max() -> usize;
}

impl MaxValue for u8 {
    fn max() -> usize {
        u8::MAX as usize
    }
}

impl MaxValue for u16 {
    fn max() -> usize {
        u16::MAX as usize
    }
}

#[derive(Debug, Clone)]
pub enum BytecodeInstructions {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterEquals,
    LesserEquals,
    Jump(u8),
    JumpIfFalse(u8),
    BackJump(u8),
    JumpLarge(u16),
    JumpIfFalseLarge(u16),
    BackJumpLarge(u16),
    Add,
    Subtract,
    Multiply,
    Division,
    TakeFast(u8),
    Take(u16),
    ReplaceFast(u8),
    Replace(u16),
    Not,
    Negate,
    Raw(u8),
    Rotate,
    Over,
    Swap,
    Duplicate,

}