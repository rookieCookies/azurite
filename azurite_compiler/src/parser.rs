use azurite_common::{Data, DataType};

use crate::{
    ast::{
        binary_operation::BinaryOperator, unary_operation::UnaryOperator, FunctionInline,
        Instruction, InstructionType,
    },
    error::{Error, Highlight, FATAL},
    lexer::{Token, TokenType},
};

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,

    current_file: String,

    errors: Vec<Error>,
    panic_errors: Vec<Error>,
    panic_mode: bool,
}

impl Parser {
    pub fn parse_tokens(
        tokens: Vec<Token>,
        current_file: String,
    ) -> Result<Vec<Instruction>, Vec<Error>> {
        let mut parser = Self {
            tokens,
            index: 0,
            errors: Vec::new(),
            panic_errors: Vec::new(),
            panic_mode: false,
            current_file,
        };

        let instructions = parser.parse_till(&TokenType::EndOfFile);

        if parser.panic_mode {
            parser.errors = std::mem::take(&mut parser.panic_errors);
        }
        if parser.errors.is_empty() {
            Ok(instructions)
        } else {
            Err(parser.errors)
        }
    }

    fn parse_till(&mut self, token_type: &TokenType) -> Vec<Instruction> {
        let mut instructions = vec![];
        loop {
            if self.current_token().is_none() {
                break;
            }
            if [token_type].contains(&&self.current_token().unwrap().token_type) {
                break;
            }

            if self.panic_mode
                && [
                    TokenType::Assert,
                    TokenType::Var,
                    TokenType::If,
                    TokenType::Else,
                    TokenType::While,
                    TokenType::Fn,
                    TokenType::Return,
                    TokenType::Struct,
                ]
                .contains(&self.current_token().unwrap().token_type)
            {
                self.panic_mode = false;
                self.errors = std::mem::take(&mut self.panic_errors);
            }

            let instruction = if let Some(instruction) = self.statement() {
                instruction
            } else {
                self.panic_mode = true;
                self.panic_errors = std::mem::take(&mut self.errors);
                self.advance();
                continue;
            };
            instructions.push(instruction);
            self.advance();
        }
        let _ = self.expect(token_type);
        instructions
    }

    fn parse_type(&mut self) -> Option<DataType> {
        let identifier = self.expect_identifier()?;
        Some(DataType::from_string(identifier))
    }
}

// ############################
//
//           UTILS
//
// ############################
static EMPTY_IDENTIFIER: TokenType = TokenType::Identifier(String::new());
impl Parser {
    fn advance(&mut self) {
        self.index += 1;
    }

    fn retract(&mut self) {
        self.index -= 1;
    }

    fn current_token(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index + 1)
    }

    fn context_of_current_token(&mut self) -> Option<(u32, u32, u32)> {
        let token = if let Some(v) = self.current_token() {
            v
        } else {
            self.retract();
            self.error_premature_eof()?;
            return None;
        };

        Some((token.start, token.end, token.line))
    }

    fn expect_identifier(&mut self) -> Option<&String> {
        self.expect(&EMPTY_IDENTIFIER)?;
        match &self.current_token().unwrap().token_type {
            TokenType::Identifier(identifier) => Some(identifier),
            _ => None,
        }
    }

    fn expect_identifier_and_advance(&mut self) -> Option<String> {
        match self.expect_identifier() {
            Some(value) => {
                let value = value.clone();
                self.advance();
                Some(value)
            }
            None => None,
        }
    }

    #[must_use]
    fn expect_without_error(&mut self, value: &TokenType) -> Option<()> {
        let current_token = if let Some(token) = self.current_token() {
            token
        } else {
            self.error_premature_eof()?;
            return None;
        };
        if std::mem::discriminant(&current_token.token_type) != std::mem::discriminant(value) {
            return None;
        }
        Some(())
    }

    #[must_use]
    fn expect_without_error_and_advance(&mut self, value: &TokenType) -> Option<()> {
        match self.expect_without_error(value) {
            Some(_) => {
                self.advance();
                Some(())
            }
            None => None,
        }
    }

    #[must_use]
    fn expect(&mut self, value: &TokenType) -> Option<()> {
        if self.expect_without_error(value).is_none() {
            let current_token = match self.current_token() {
                Some(token) => token,
                None => return None,
            };
            self.errors.push(Error::new(
                vec![(current_token.start, current_token.end, Highlight::Red)],
                "unexpected token",
                format!("expected {value:?}, found {current_token:?}"),
                &FATAL,
                self.current_file.clone(),
            ));
            return None;
        }
        Some(())
    }

    #[must_use]
    fn expect_and_advance(&mut self, value: &TokenType) -> Option<()> {
        match self.expect(value) {
            Some(_) => {
                self.advance();
                Some(())
            }
            None => None,
        }
    }

    fn binary_operation(
        &mut self,
        left_function: fn(&mut Parser, &ExpressionSettings) -> Option<Instruction>,
        right_function: fn(&mut Parser, &ExpressionSettings) -> Option<Instruction>,
        expression_settings: &ExpressionSettings,
        operation_tokens: &[TokenType],
    ) -> Option<Instruction> {
        let mut left = left_function(self, expression_settings)?;
        self.advance();
        while let Some(current_token) = self.current_token() {
            if !operation_tokens.contains(&current_token.token_type) {
                break;
            }
            let operator = BinaryOperator::from(&current_token.token_type);
            self.advance();
            let right = right_function(self, expression_settings)?;

            left = Instruction {
                start: left.start,
                end: right.end,
                line: right.line,
                instruction_type: InstructionType::BinaryOperation {
                    left: Box::new(left),
                    right: Box::new(right),
                    operator,
                },
                pop_after: false,
            };

            self.advance();
        }
        self.retract();
        Some(left)
    }
}

// ############################
//
//        STATEMENTS
//
// ############################
impl Parser {
    // statement:
    // |> variable-declaration
    // |> variable-update
    // |> while-statement
    // |> function-declaration
    // |> return-statement
    // |> structure-declaration
    // |> assert-statement
    // |> impl-block
    // |> raw-call
    // |> expression
    fn statement(&mut self) -> Option<Instruction> {
        let current_token = self.current_token()?;
        match current_token.token_type {
            TokenType::Var => return self.variable_declaration(),
            TokenType::While => return self.while_expression(),
            TokenType::Fn | TokenType::Inline => return self.function_declaration(&None),
            TokenType::Return => return self.return_statement(),
            TokenType::Struct => return self.structure_declaration(),
            TokenType::Impl => return self.impl_block(),
            TokenType::Raw => return self.raw_call(),
            TokenType::Using => return self.using_statement(),
            TokenType::Identifier(_) => {
                if let Some(peeked_token) = self.peek() {
                    if TokenType::Equals == peeked_token.token_type {
                        return self.variable_update();
                    }
                }
            }
            _ => (),
        }
        let mut expression_statement = self.expression(&ExpressionSettings::new())?;
        expression_statement.pop_after = true;
        Some(expression_statement)
    }

    // variable-declaration:
    // |> 'var' identifier '=' expression
    // |> 'var' identifier ':' type '=' expression
    fn variable_declaration(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Var)?;
        let identifier = self.expect_identifier_and_advance()?;
        let type_declaration = match self.expect_without_error_and_advance(&TokenType::Colon) {
            Some(_) => {
                let v = self.parse_type();
                self.advance();
                v
            }
            None => None,
        };
        self.expect_and_advance(&TokenType::Equals)?;
        let data = self.expression(&ExpressionSettings::new())?;
        Some(Instruction {
            instruction_type: InstructionType::DeclareVariable {
                identifier,
                data: Box::new(data),
                type_declaration,
                overwrite: None,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // variable-update:
    // |> identifier '=' expr
    // |> identifier( '.' identifier )* '=' expr
    fn variable_update(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        let identifier = self.expect_identifier_and_advance()?;
        // self.advance();
        // self.retract();
        self.expect_and_advance(&TokenType::Equals)?;
        let data = self.expression(&ExpressionSettings::new())?;
        Some(Instruction {
            instruction_type: InstructionType::UpdateVarOnStack {
                identifier,
                data: Box::new(data),
                index: 0,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // while-statement:
    // |> 'while' comparison-expression body
    fn while_expression(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::While)?;
        let condition =
            self.comparison_expression(&ExpressionSettings::new().remove_struct_parsing())?;
        self.advance();
        let body = self.body()?;
        Some(Instruction {
            instruction_type: InstructionType::WhileStatement {
                condition: Box::new(condition),
                body: Box::new(body),
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // function-declaration:
    // |> 'fn' identifier '(' [identifier : type]* ')' body
    // |> 'fn' identifier '(' [identifier : type]* ')' '->' type body
    fn function_declaration(
        &mut self,
        first_argument_self: &Option<DataType>,
    ) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        let inlined = self
            .expect_without_error_and_advance(&TokenType::Inline)
            .is_some();
        self.expect_and_advance(&TokenType::Fn)?;
        let identifier = self.expect_identifier_and_advance()?;
        self.expect_and_advance(&TokenType::LeftParenthesis)?;
        let mut arguments = vec![];
        loop {
            let current_token = self.current_token().unwrap();
            if [TokenType::RightParenthesis, TokenType::EndOfFile]
                .contains(&current_token.token_type)
            {
                break;
            }
            if !arguments.is_empty() {
                self.expect_and_advance(&TokenType::Comma)?;

                let identifier = self.expect_identifier_and_advance()?;
                self.expect_and_advance(&TokenType::Colon)?;
                let type_declaration = self.parse_type()?;
                self.advance();
                arguments.push((identifier, type_declaration));
                continue;
            }

            let identifier = self.expect_identifier_and_advance()?;

            if let ("self", Some(x)) = (identifier.as_str(), first_argument_self.clone()) {
                arguments.push((identifier, x));
            } else {
                self.expect_and_advance(&TokenType::Colon)?;
                let type_declaration = self.parse_type()?;
                self.advance();
                arguments.push((identifier, type_declaration));
            }
        }
        self.expect_and_advance(&TokenType::RightParenthesis)?;

        let return_type = if self
            .expect_without_error_and_advance(&TokenType::Arrow)
            .is_some()
        {
            let type_decl = self.parse_type()?;
            self.advance();
            type_decl
        } else {
            DataType::Empty
        };

        let body = self.body()?;

        Some(Instruction {
            instruction_type: InstructionType::FunctionDeclaration {
                identifier,
                body: Box::new(body),
                arguments,
                return_type,
                inlined,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // return-statement:
    // |> 'return' expression
    fn return_statement(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Return)?;
        let expression = self.expression(&ExpressionSettings::new())?;
        Some(Instruction {
            instruction_type: InstructionType::Return(Some(Box::new(expression))),
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // structure-declaration:
    // |> 'struct' identifier '{' [identifier ':' type ',']* '}'
    fn structure_declaration(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Struct)?;
        let identifier = self.expect_identifier_and_advance()?;
        self.expect_and_advance(&TokenType::LeftCurly)?;
        let mut fields = vec![];

        loop {
            let current_token = self.current_token().unwrap();
            if [TokenType::RightCurly, TokenType::EndOfFile].contains(&current_token.token_type) {
                break;
            }
            if !fields.is_empty() {
                self.expect_and_advance(&TokenType::Comma)?;

                if [TokenType::RightCurly, TokenType::EndOfFile]
                    .contains(&self.current_token().unwrap().token_type)
                {
                    break;
                }
            }
            let identifier = self.expect_identifier_and_advance()?;
            self.expect_and_advance(&TokenType::Colon)?;
            let type_declaration = self.parse_type()?;
            fields.push((identifier, type_declaration));
            self.advance();
        }

        self.expect(&TokenType::RightCurly)?;

        Some(Instruction {
            instruction_type: InstructionType::StructDeclaration { identifier, fields },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // impl-block:
    // |> 'impl' identifier '{' function-declaration* '}'
    fn impl_block(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Impl)?;
        let datatype = self.parse_type()?;
        self.advance();
        self.expect_and_advance(&TokenType::LeftCurly)?;
        let mut functions = vec![];
        loop {
            let current_token = self.current_token().unwrap();
            if [TokenType::RightCurly, TokenType::EndOfFile].contains(&current_token.token_type) {
                break;
            }
            if self.expect_without_error(&TokenType::Inline).is_some()
                || self.expect(&TokenType::Fn).is_some()
            {
                let mut function = self.function_declaration(&Some(datatype.clone()))?;
                match &mut function.instruction_type {
                    InstructionType::FunctionDeclaration { identifier, .. } => {
                        *identifier = format!("{datatype}::{identifier}");
                    }
                    _ => panic!("unreachable"),
                }
                functions.push(function);
            }
            self.advance();
        }
        self.expect(&TokenType::RightCurly)?;

        Some(Instruction {
            instruction_type: InstructionType::ImplBlock {
                datatype,
                functions,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // raw-call:
    // |> 'raw' INTEGER
    fn raw_call(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Raw)?;
        self.expect(&TokenType::Integer(0))?;
        let integer = match self.current_token().unwrap().token_type {
            TokenType::Integer(v) => v,
            _ => panic!("unreachable"),
        };
        Some(Instruction {
            instruction_type: InstructionType::RawCall(integer),
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // using-statement:
    // |> 'using' STRING
    fn using_statement(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Using)?;
        self.expect(&TokenType::String(String::new()))?;
        let string = match &self.current_token().unwrap().token_type {
            TokenType::String(v) => v.clone(),
            _ => panic!("unreachable"),
        };
        let _end_context = self.context_of_current_token()?;

        Some(Instruction {
            instruction_type: InstructionType::Using(string),
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }
}

// ############################
//
//        EXPRESSIONS
//
// ############################
impl Parser {
    // expression:
    // |> comparison-expression
    // // |> expression 'as' type
    fn expression(&mut self, expression_settings: &ExpressionSettings) -> Option<Instruction> {
        self.comparison_expression(expression_settings)
    }

    // comparison-expression:
    // |> not-operation
    // |> arithmetic-expression '=='|'!='|'>='|'<='|'>'|'<' arithmetic-expression
    fn comparison_expression(
        &mut self,
        expression_settings: &ExpressionSettings,
    ) -> Option<Instruction> {
        if self
            .expect_without_error(&TokenType::ExclamationMark)
            .is_some()
        {
            return self.not_operation(expression_settings);
        }
        self.binary_operation(
            Parser::arithmetic_expression,
            Parser::arithmetic_expression,
            expression_settings,
            &[
                TokenType::EqualsEquals,
                TokenType::NotEquals,
                TokenType::GreaterEquals,
                TokenType::LesserEquals,
                TokenType::GreaterThan,
                TokenType::LessThan,
            ],
        )
    }

    // arithmetic-expression
    // |> product-expression '+'|'-' product-expression
    fn arithmetic_expression(
        &mut self,
        expression_settings: &ExpressionSettings,
    ) -> Option<Instruction> {
        self.binary_operation(
            Parser::product_expression,
            Parser::product_expression,
            expression_settings,
            &[TokenType::Plus, TokenType::Minus],
        )
    }

    // product-expression
    // |> factor-expression '*'|'/' factor-expression
    fn product_expression(
        &mut self,
        expression_settings: &ExpressionSettings,
    ) -> Option<Instruction> {
        self.binary_operation(
            Parser::factor_expression,
            Parser::factor_expression,
            expression_settings,
            &[TokenType::Star, TokenType::Slash],
        )
    }

    // factor-expression
    // |> negation-expression
    // |> power-expression
    fn factor_expression(
        &mut self,
        expression_settings: &ExpressionSettings,
    ) -> Option<Instruction> {
        if self.expect_without_error(&TokenType::Minus).is_some() {
            return self.negation_operation();
        }
        self.power_expression(expression_settings)
    }

    // power-expression:
    // |> unit '^' factor-expression
    fn power_expression(
        &mut self,
        expression_settings: &ExpressionSettings,
    ) -> Option<Instruction> {
        self.binary_operation(
            Parser::unit,
            Parser::factor_expression,
            expression_settings,
            &[TokenType::Carrot],
        )
    }

    // unit:
    // |> atom
    // |> atom ( '.' identifier )*
    // |> atom ( '.' function-call )*
    fn unit(&mut self, expression_settings: &ExpressionSettings) -> Option<Instruction> {
        let mut instruction = self.atom(expression_settings)?;
        self.advance();
        loop {
            if self.current_token().is_none()
                || self.current_token().unwrap().token_type != TokenType::Dot
            {
                break;
            }
            self.advance(); // skip the dot
            let identifier = self.expect_identifier()?.clone();
            if self.peek().is_some()
                && self.peek().unwrap().token_type == TokenType::LeftParenthesis
            {
                let mut function = self.function_call()?;
                match &mut function.instruction_type {
                    InstructionType::FunctionCall {
                        arguments,
                        created_by_accessing,
                        ..
                    } => {
                        let mut a = vec![instruction];
                        a.append(arguments);
                        *arguments = a;
                        *created_by_accessing = true;
                    }
                    _ => panic!("unreachable"),
                }
                instruction = function;
                self.advance();
                continue;
            }
            instruction = Instruction {
                start: instruction.start,
                end: self.context_of_current_token()?.1,
                line: instruction.line,
                pop_after: false,
                instruction_type: InstructionType::AccessVariable {
                    identifier,
                    data: Box::new(instruction),
                    id: 0,
                },
            };
            self.advance();
        }
        self.retract();
        Some(instruction)
    }

    // atom:
    // |> INTEGER
    // |> FLOAT
    // |> STRING
    // |> body
    // |> variable-access
    // |> if-expression
    // |> function-call
    // |> structure-creation
    // |> '(' expression ')'
    fn atom(&mut self, expression_settings: &ExpressionSettings) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        let instruction_type = match &self.current_token()?.token_type {
            TokenType::Integer(int) => InstructionType::Data(Data::Integer(*int)),
            TokenType::Float(float) => InstructionType::Data(Data::Float(*float)),
            TokenType::String(str) => InstructionType::Data(Data::String(str.clone())),
            TokenType::True => InstructionType::Data(Data::Bool(true)),
            TokenType::False => InstructionType::Data(Data::Bool(false)),
            TokenType::LeftCurly => return self.body(),
            TokenType::If => return self.if_expression(),
            TokenType::Identifier(v) => match self.peek() {
                Some(peeked_token) => match peeked_token.token_type {
                    TokenType::LeftParenthesis => return self.function_call(),
                    TokenType::LeftCurly => {
                        if expression_settings.can_parse_struct {
                            return self.structure_creation();
                        }
                        return self.variable_access();
                    }
                    TokenType::DoubleColon => {
                        return {
                            let v = v.clone();
                            self.advance();
                            self.advance();
                            let mut function = self.function_call()?;
                            match &mut function.instruction_type {
                                InstructionType::FunctionCall { identifier, .. } => {
                                    *identifier = format!("{v}::{identifier}");
                                }
                                _ => panic!("unreachable"),
                            };
                            Some(function)
                        }
                    }
                    _ => return self.variable_access(),
                },
                None => return self.variable_access(),
            },
            TokenType::LeftParenthesis => {
                self.advance();
                let expression = self.expression(&ExpressionSettings::new())?;
                self.advance();
                self.expect(&TokenType::RightParenthesis)?;
                return Some(expression);
            }
            TokenType::EndOfFile => {
                self.error_premature_eof();
                return None;
            }

            _ => {
                self.unexpected_token(context.0)?;
                return None;
            }
        };

        Some(Instruction {
            instruction_type,
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // body:
    // |> '{' statement* '}'
    fn body(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::LeftCurly)?;
        let body = self.parse_till(&TokenType::RightCurly);
        self.expect(&TokenType::RightCurly)?;

        Some(Instruction {
            instruction_type: InstructionType::Block { body, pop: 0 },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // variable-access:
    // |> identifier
    fn variable_access(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        let identifier = self.expect_identifier()?.clone();
        Some(Instruction {
            instruction_type: InstructionType::LoadVariable(identifier, 0),
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // if-expression:
    // |> 'if' comparison-expression body
    // |> 'if' comparison-expression body 'else' if-expression
    // |> 'if' comparison-expression body 'else' body
    fn if_expression(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::If)?;
        let comparison =
            self.comparison_expression(&ExpressionSettings::new().remove_struct_parsing())?;
        self.advance();
        let body = self.body()?;
        let mut else_part = None;

        if let Some(peek) = self.peek() {
            if TokenType::Else == peek.token_type {
                self.advance();
                else_part = Some({
                    let start = self.context_of_current_token()?;
                    let mut instruction = if let Some(peek) = self.peek() {
                        if TokenType::If == peek.token_type {
                            self.advance();
                            self.if_expression()?
                        } else {
                            self.advance();
                            self.body()?
                        }
                    } else {
                        self.advance();
                        self.body()?
                    };

                    instruction.start = start.0;
                    Box::new(instruction)
                });
            }
        }

        Some(Instruction {
            instruction_type: InstructionType::IfExpression {
                condition: Box::new(comparison),
                body: Box::new(body),
                else_part,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // function-call:
    // |> identifier '(' expression* ')'
    fn function_call(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        let identifier = self.expect_identifier_and_advance()?;
        self.expect_and_advance(&TokenType::LeftParenthesis)?;

        let mut arguments = vec![];
        loop {
            let current_token = self.current_token().unwrap();
            if [TokenType::RightParenthesis, TokenType::EndOfFile]
                .contains(&current_token.token_type)
            {
                break;
            }
            if !arguments.is_empty() {
                self.expect_and_advance(&TokenType::Comma)?;
            }
            arguments.push(self.expression(&ExpressionSettings::new())?);
            self.advance();
        }

        // println!("{arguments:#?}");

        self.expect(&TokenType::RightParenthesis)?;

        Some(Instruction {
            instruction_type: InstructionType::FunctionCall {
                identifier,
                arguments,
                index: FunctionInline::None(0),
                created_by_accessing: false,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // structure-creation:
    // |> identifier '{' [identifier ':' expression ',']* '}'
    fn structure_creation(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        let identifier = self.expect_identifier_and_advance()?;
        self.expect_and_advance(&TokenType::LeftCurly)?;

        let mut fields = vec![];
        loop {
            let current_token = self.current_token().unwrap();
            if [TokenType::RightCurly, TokenType::EndOfFile].contains(&current_token.token_type) {
                break;
            }
            if !fields.is_empty() {
                self.expect_and_advance(&TokenType::Comma)?;
                if [TokenType::RightCurly, TokenType::EndOfFile]
                    .contains(&self.current_token().unwrap().token_type)
                {
                    break;
                }
            }
            let identifier = self.expect_identifier_and_advance()?;
            self.expect_and_advance(&TokenType::Colon)?;
            let expression = self.expression(&ExpressionSettings::new())?;
            self.advance();
            fields.push((identifier, expression));
        }

        self.expect(&TokenType::RightCurly)?;

        Some(Instruction {
            instruction_type: InstructionType::CreateStruct {
                identifier,
                variables: fields,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // not-operation:
    // |> '!' comparison-expression
    fn not_operation(&mut self, expression_settings: &ExpressionSettings) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::ExclamationMark)?;
        Some(Instruction {
            instruction_type: InstructionType::UnaryOperation {
                data: Box::new(self.expression(expression_settings)?),
                operator: UnaryOperator::Not,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }

    // negation-operation:
    // |> '-' factor-expression
    fn negation_operation(&mut self) -> Option<Instruction> {
        let context = self.context_of_current_token()?;
        self.expect_and_advance(&TokenType::Minus)?;
        Some(Instruction {
            instruction_type: InstructionType::UnaryOperation {
                data: Box::new(self.factor_expression(&ExpressionSettings::new())?),
                operator: UnaryOperator::Minus,
            },
            start: context.0,
            end: self.context_of_current_token()?.1,
            line: context.2,
            pop_after: false,
        })
    }
}

impl Parser {
    fn error_premature_eof(&mut self) -> Option<()> {
        let context = self.context_of_current_token()?;
        self.errors.push(Error::new(
            vec![(context.0, context.1, Highlight::Red)],
            "premature end of file",
            "expected a token but the file ended".to_string(),
            &FATAL,
            self.current_file.clone(),
        ));
        Some(())
    }

    fn unexpected_token(&mut self, start: u32) -> Option<()> {
        let end = self.context_of_current_token()?.1;
        self.errors.push(Error::new(
            vec![(start, end, Highlight::Red)],
            "unexpected token",
            "".to_string(),
            &FATAL,
            self.current_file.clone(),
        ));
        Some(())
    }
}

struct ExpressionSettings {
    can_parse_struct: bool,
}

impl ExpressionSettings {
    fn new() -> Self {
        Self {
            can_parse_struct: true,
        }
    }

    fn remove_struct_parsing(mut self) -> Self {
        self.can_parse_struct = false;
        self
    }
}
