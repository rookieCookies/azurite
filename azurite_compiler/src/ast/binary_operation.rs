use std::fmt::Display;

use crate::lexer::TokenType;

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Addition,
    Subtraction,
    Multiplication,
    Division,

    EqualsEquals,
    GreaterThan,
    LessThan,
    GreaterEquals,
    LesserEquals,
    NotEquals,
}

impl From<&TokenType> for BinaryOperator {
    fn from(value: &TokenType) -> Self {
        match value {
            TokenType::Plus => Self::Addition,
            TokenType::Minus => Self::Subtraction,
            TokenType::Star => Self::Multiplication,
            TokenType::Slash => Self::Division,

            TokenType::EqualsEquals => Self::EqualsEquals,
            TokenType::GreaterEquals => Self::GreaterEquals,
            TokenType::LesserEquals => Self::LesserEquals,
            TokenType::NotEquals => Self::NotEquals,
            
            TokenType::LeftAngle => Self::LessThan,
            TokenType::RightAngle => Self::GreaterThan,
            _ => panic!("invalid binary operator {value:?}"),
        }
    }
}

impl Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BinaryOperator::Addition => "add",
                BinaryOperator::Subtraction => "subtract",
                BinaryOperator::Multiplication => "multiply",
                BinaryOperator::Division => "divide",
                BinaryOperator::EqualsEquals => "equals to",
                BinaryOperator::GreaterThan => "greater than",
                BinaryOperator::LessThan => "less than",
                BinaryOperator::GreaterEquals => "equals to or greater than",
                BinaryOperator::LesserEquals => "equals to or less than",
                BinaryOperator::NotEquals => "not equals to",
            }
        )
    }
}
