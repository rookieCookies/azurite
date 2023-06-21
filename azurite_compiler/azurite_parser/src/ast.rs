use std::fmt::Display;

use azurite_lexer::TokenKind;
use common::{SymbolIndex, SourcedDataType, SourceRange, SourcedData};

#[derive(Debug, PartialEq, Clone)]
pub struct Instruction {
    pub instruction_kind: InstructionKind,
    pub source_range: SourceRange,
}

#[derive(Debug, PartialEq, Clone)]
pub enum InstructionKind {
    Statement   (Statement),
    Expression  (Expression),
    Declaration (Declaration),
}

#[derive(Debug, PartialEq, Clone)]
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


#[derive(Debug, PartialEq, Clone)]
pub enum Expression {
    AsCast {
        value: Box<Instruction>,
        cast_type: SourcedDataType,
    },
    
    Data(SourcedData),
    
    BinaryOp {
        operator: BinaryOperator,
        left: Box<Instruction>,
        right: Box<Instruction>,
    },

    UnaryOp {
        operator: UnaryOperator,
        value: Box<Instruction>,
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
        generics: Vec<SourcedDataType>,

        created_by_accessing: bool,
    },

    StructureCreation {
        identifier: SymbolIndex,
        identifier_range: SourceRange,
        fields: Vec<(SymbolIndex, Instruction)>,
        generics: Vec<SourcedDataType>,
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


#[derive(Debug, PartialEq, Clone)]
pub enum Declaration {
    FunctionDeclaration {
        name: SymbolIndex,
        arguments: Vec<(SymbolIndex, SourcedDataType)>,
        return_type: SourcedDataType,
        body: Vec<Instruction>,
        generics: Vec<SymbolIndex>,
        
        source_range_declaration: SourceRange,
    },


    StructDeclaration {
        name: SymbolIndex,
        fields: Vec<(SymbolIndex, SourcedDataType)>,
        generics: Vec<SymbolIndex>,
    },


    Namespace {
        body: Vec<Instruction>,
        identifier: SymbolIndex,
    },


    ImplBlock {
        body: Vec<Instruction>,
        datatype: SourcedDataType,
    },


    Extern {
        file: SymbolIndex,
        functions: Vec<ExternFunctionAST>,
    },


    UseFile {
        file_name: SymbolIndex,
    }
}


#[derive(Debug, PartialEq, Clone)]
pub struct ExternFunctionAST {
    pub raw_name: SymbolIndex,
    pub identifier: SymbolIndex,
    pub return_type: SourcedDataType,
    pub arguments: Vec<SourcedDataType>,
}


#[derive(Debug, PartialEq, Clone)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,

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
            BinaryOperator::Modulo   => "modulo",

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
            TokenKind::Plus    => Some(BinaryOperator::Add),
            TokenKind::Minus   => Some(BinaryOperator::Subtract),
            TokenKind::Star    => Some(BinaryOperator::Multiply),
            TokenKind::Slash   => Some(BinaryOperator::Divide),
            TokenKind::Percent => Some(BinaryOperator::Modulo),

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


#[derive(Debug, PartialEq, Clone)]
pub enum UnaryOperator {
    Not,
    Negate,
}

impl Display for UnaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            UnaryOperator::Not => "not",
            UnaryOperator::Negate => "negate",
        })
    }
    
}

impl UnaryOperator {
    pub fn from_token(token: &TokenKind) -> Option<Self> {
        match token {
            TokenKind::Minus => Some(UnaryOperator::Negate),
            TokenKind::Bang  => Some(UnaryOperator::Not),
            _ => None
        }
    }
}