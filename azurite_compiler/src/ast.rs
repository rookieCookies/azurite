use std::fmt::Display;

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

impl Display for InstructionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                InstructionType::Using(v) => format!("using {v}"),
                InstructionType::Data(v) => format!("{v:?}"),
                InstructionType::BinaryOperation {
                    left,
                    right,
                    operator,
                } => format!(
                    "{} {operator} {}",
                    left.instruction_type, right.instruction_type
                ),
                InstructionType::UnaryOperation { data, operator } =>
                    format!("{operator}{}", data.instruction_type),
                InstructionType::LoadVariable(v, index) => format!("load var {v} {index}"),
                InstructionType::DeclareVariable {
                    identifier,
                    data,
                    type_declaration,
                    overwrite,
                } => format!("declare var {identifier} {}", data.instruction_type),
                InstructionType::UpdateVarOnStack {
                    identifier,
                    data,
                    index,
                } => format!(
                    "update var on stack {identifier} {} {index}",
                    data.instruction_type
                ),
                InstructionType::Block { body, pop } => format!(
                    "body: {} pop: {pop}",
                    body.iter()
                        .map(|x| format!("{} ", x.instruction_type))
                        .collect::<String>()
                ),
                InstructionType::IfExpression {
                    condition,
                    body,
                    else_part,
                } => format!(
                    "if {} do {} else {}",
                    condition.instruction_type,
                    body.instruction_type,
                    else_part
                        .as_ref()
                        .map(|x| x.instruction_type.to_string())
                        .unwrap_or("nothing".to_string())
                ),
                InstructionType::Return(v) => format!(
                    "return {}",
                    v.as_ref()
                        .map(|x| x.instruction_type.to_string())
                        .unwrap_or("nothing".to_string())
                ),
                InstructionType::WhileStatement { condition, body } => format!(
                    "while {} do {}",
                    condition.instruction_type, body.instruction_type
                ),
                InstructionType::FunctionDeclaration {
                    identifier,
                    body,
                    arguments,
                    return_type,
                    inlined,
                } => format!(
                    "declare function {identifier} with {} returns {} is {}inlined and does {}",
                    arguments
                        .iter()
                        .map(|x| format!("{}: {}", x.0, x.1))
                        .collect::<String>(),
                    return_type,
                    if *inlined {
                        "".to_string()
                    } else {
                        "not ".to_string()
                    },
                    body.instruction_type
                ),
                InstructionType::FunctionCall {
                    identifier,
                    arguments,
                    index,
                    created_by_accessing,
                } => format!(
                    "call {identifier} with {}",
                    arguments
                        .iter()
                        .map(|z| format!("{} ", z.instruction_type))
                        .collect::<String>()
                ),
                InstructionType::StructDeclaration { identifier, fields } => format!(
                    "declare struct {identifier} with fields {}",
                    fields
                        .iter()
                        .map(|x| format!("{}: {}", x.0, x.1))
                        .collect::<String>()
                ),
                InstructionType::CreateStruct {
                    identifier,
                    variables,
                } => format!(
                    "create struct {identifier}: {}",
                    variables
                        .iter()
                        .map(|x| format!("{}: {}", x.0, x.1.instruction_type))
                        .collect::<String>()
                ),
                InstructionType::AccessVariable {
                    identifier,
                    data,
                    id,
                } => format!("access var {identifier}"),
                InstructionType::ImplBlock {
                    datatype,
                    functions,
                } => "impl block".to_string(),
                InstructionType::RawCall(index) => format!("raw call {index}"),
            }
        )
    }
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
