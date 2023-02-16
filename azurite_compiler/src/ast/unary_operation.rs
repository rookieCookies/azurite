use std::fmt::Display;

use crate::lexer::TokenType;

#[derive(Debug, Clone)]
pub enum UnaryOperator {
    Minus,
    Not,
}

impl From<&TokenType> for UnaryOperator {
    fn from(value: &TokenType) -> Self {
        match value {
            TokenType::Minus => Self::Minus,
            TokenType::ExclamationMark => Self::Not,
            _ => panic!("invalid unary operator, compiler bug"),
        }
    }
}

impl Display for UnaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                UnaryOperator::Minus => "minus",
                UnaryOperator::Not => "not",
            }
        )
    }
}
