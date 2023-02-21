use azurite_common::{Data, DataType};

use self::{binary_operation::BinaryOperator, unary_operation::UnaryOperator};

pub mod binary_operation;
pub mod unary_operation;

#[derive(Debug, Clone)]
pub enum InstructionType {
    Using(String),
    Data(Data),
    BinaryOperation {
        left: Box<Instruction>,
        right: Box<Instruction>,
        operator: BinaryOperator,
    },
    UnaryOperation {
        data: Box<Instruction>,
        operator: UnaryOperator,
    },
    LoadVariable(String, u16),
    DeclareVariable {
        identifier: String,
        data: Box<Instruction>,
        type_declaration: Option<DataType>,
        overwrite: Option<u16>,
    },
    UpdateVarOnStack {
        identifier: String,
        data: Box<Instruction>,
        index: u16,
    },
    Block {
        body: Vec<Instruction>,
        pop: u16,
    },
    IfExpression {
        condition: Box<Instruction>,
        body: Box<Instruction>,
        else_part: Option<Box<Instruction>>,
    },
    Return(Option<Box<Instruction>>),
    WhileStatement {
        condition: Box<Instruction>,
        body: Box<Instruction>,
    },
    FunctionDeclaration {
        identifier: String,
        body: Box<Instruction>,
        arguments: Vec<(String, DataType)>,
        return_type: DataType,
        inlined: bool,
    },
    FunctionCall {
        identifier: String,
        arguments: Vec<Instruction>,
        index: FunctionInline,
        created_by_accessing: bool,
    },
    StructDeclaration {
        identifier: String,
        fields: Vec<(String, DataType)>,
    },
    CreateStruct {
        identifier: String,
        variables: Vec<(String, Instruction)>,
    },
    AccessVariable {
        identifier: String,
        data: Box<Instruction>,
        id: u32,
    },
    ImplBlock {
        datatype: DataType,
        functions: Vec<Instruction>,
    },
    RawCall(i64),
}

#[derive(Debug, Clone)]
pub enum FunctionInline {
    None(usize),
    Inline {
        instructions: Box<Instruction>,
        variable_offset: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub instruction_type: InstructionType,
    pub start: u32,
    pub end: u32,
    pub line: u32,
    pub pop_after: bool,
}
