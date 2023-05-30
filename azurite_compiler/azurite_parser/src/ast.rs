use std::fmt::Display;

use azurite_errors::{SourceRange, SourcedData, SourcedDataType};
use azurite_lexer::TokenKind;
use common::SymbolIndex;

#[derive(Debug, PartialEq)]
pub struct Instruction {
    pub instruction_kind: InstructionKind,
    pub source_range: SourceRange,
}

#[derive(Debug, PartialEq)]
pub enum InstructionKind {
    Statement   (Statement),
    Expression  (Expression),
    Declaration (Declaration),
}

#[derive(Debug, PartialEq)]
pub enum Statement {
    DeclareVar {
        identifier: SymbolIndex,
        type_hint: Option<SourcedDataType>,
        data: Box<Instruction>,
    },
    
    VariableUpdate {
        left: Box<Instruction>,
        right: Box<Instruction>
    },

    FieldUpdate {
        structure: Box<Instruction>,
        right: Box<Instruction>,
        identifier: SymbolIndex,
        index_to: usize,
    },
    
    Loop {
        body: Vec<Instruction>,
    },

    Break,
    Continue,
    Return(Box<Instruction>),
}


#[derive(Debug, PartialEq)]
pub enum Expression {
    Data(SourcedData),
    
    BinaryOp {
        operator: BinaryOperator,
        left: Box<Instruction>,
        right: Box<Instruction>,
    },
    
    Block {
        body: Vec<Instruction>,
    },
    
    IfExpression {
        body: Vec<Instruction>,
        condition: Box<Instruction>,
        else_part: Option<Box<Instruction>>,
    },
    
    Identifier(SymbolIndex),

    FunctionCall {
        identifier: SymbolIndex,
        arguments: Vec<Instruction>,
    },

    StructureCreation {
        identifier: SymbolIndex,
        identifier_range: SourceRange,
        fields: Vec<(SymbolIndex, Instruction)>,
    },

    AccessStructureData {
        structure: Box<Instruction>,
        identifier: SymbolIndex,
        index_to: usize,
    },

    WithinNamespace {
        namespace: SymbolIndex,
        do_within: Box<Instruction>,
    }
    
}


#[derive(Debug, PartialEq)]
pub enum Declaration {
    FunctionDeclaration {
        name: SymbolIndex,
        arguments: Vec<(SymbolIndex, SourcedDataType)>,
        return_type: SourcedDataType,
        body: Vec<Instruction>,
        
        source_range_declaration: SourceRange,
    },

    StructDeclaration {
        name: SymbolIndex,
        fields: Vec<(SymbolIndex, SourcedDataType)>,
    },

    Namespace {
        body: Vec<Instruction>,
        identifier: SymbolIndex,
    },

    Extern {
        file: SymbolIndex,
        functions: Vec<ExternFunctionAST>,
    },

    UseFile {
        file_name: SymbolIndex,
    }
}


#[derive(Debug, PartialEq)]
pub struct ExternFunctionAST {
    pub raw_name: SymbolIndex,
    pub identifier: SymbolIndex,
    pub return_type: SourcedDataType,
    pub arguments: Vec<SourcedDataType>,
}


#[derive(Debug, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,

    Equals,
    NotEquals,
    GreaterThan,
    LesserThan,
    GreaterEquals,
    LesserEquals,
}

impl Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            BinaryOperator::Add      => "addision",
            BinaryOperator::Subtract => "subtraction",
            BinaryOperator::Multiply => "multiplication",
            BinaryOperator::Divide   => "division",

            BinaryOperator::Equals => "equals",
            BinaryOperator::NotEquals => "not equals",
            BinaryOperator::GreaterThan => "greater than",
            BinaryOperator::LesserThan => "lesser than",
            BinaryOperator::GreaterEquals => "greater equals",
            BinaryOperator::LesserEquals => "lesser equals",
        })
    }
    
}

impl BinaryOperator {
    pub fn from_token(token: &TokenKind) -> Option<Self> {
        match token {
            TokenKind::Plus  => Some(BinaryOperator::Add),
            TokenKind::Minus => Some(BinaryOperator::Subtract),
            TokenKind::Star  => Some(BinaryOperator::Multiply),
            TokenKind::Slash => Some(BinaryOperator::Divide),

            TokenKind::RightAngle => Some(BinaryOperator::GreaterThan),
            TokenKind::LeftAngle => Some(BinaryOperator::LesserThan),
            TokenKind::GreaterEquals => Some(BinaryOperator::GreaterEquals),
            TokenKind::LesserEquals => Some(BinaryOperator::LesserEquals),
            TokenKind::EqualsTo => Some(BinaryOperator::Equals),
            TokenKind::NotEqualsTo => Some(BinaryOperator::NotEquals),
            _ => None
        }
    }
}