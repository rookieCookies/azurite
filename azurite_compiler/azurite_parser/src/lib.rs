pub mod ast;

use std::{iter::Peekable, vec::IntoIter};

use ast::{Instruction, BinaryOperator, InstructionKind, Expression, Statement, Declaration, ExternFunctionAST};
use azurite_lexer::{Token, TokenKind, Keyword, Literal};
use azurite_errors::{Error, SourceRange, CompilerError, ErrorBuilder, CombineIntoError};
use common::{DataType, Data, SymbolTable, SourcedDataType, SourcedData, SymbolIndex};

type ParseResult = Result<Instruction, Error>;

struct Parser<'a> {
    tokens: Peekable<IntoIter<Token>>,

    current: Option<Token>,

    symbol_table: &'a mut SymbolTable,
}

pub fn parse(tokens: IntoIter<Token>, symbol_table: &mut SymbolTable) -> Result<Vec<Instruction>, Error> {
    let mut parser = Parser {
        tokens: tokens.peekable(),
        current: None,
        symbol_table,
    };

    parser.advance();
    parser.parse_till(&TokenKind::EndOfFile)
}


impl Parser<'_> {
    fn parse_till(&mut self, token_kind: &TokenKind) -> Result<Vec<Instruction>, Error> {
        let mut instructions = vec![];
        let mut errors = vec![];
        
        while let Some(token) = self.current_token() {
            if &token.token_kind == token_kind || token.token_kind == TokenKind::EndOfFile {
                break
            }

            match self.statement() {
                Ok(v) => instructions.push(v),
                Err(e) => {
                    if let Some(err) = errors.last() {
                        if err == &e {
                            self.advance();
                            continue
                        }
                    }
                    errors.push(e);
                    continue
                },
            }

            self.advance();
            
        }

        if let Err(err) = self.expect(token_kind) {
            errors.push(err);
        }
        
        if errors.is_empty() {
            Ok(instructions)
        } else {
            Err(errors.combine_into_error())
       }
    }
  
}

impl Parser<'_> {    
    fn advance(&mut self) -> Option<&Token> {
        self.current = self.tokens.next();

        self.current_token()
    }

    fn peek(&mut self) -> Option<&Token> {
        self.tokens.peek()
    }

    fn current_token(&self) -> Option<&Token> {
        self.current.as_ref()
    }

    fn expect(&self, token_kind: &TokenKind) -> Result<&Token, Error> {
        let token = match self.current_token() {
            Some(value) => value,
            None => panic!("unreachable {token_kind:?}"),
        };

        if &token.token_kind != token_kind {
            return Err(CompilerError::new(102, "unexpected token")
                .highlight(token.source_range)
                    .note(format!("expected {token_kind:?}"))
                .build())
        }

        Ok(token)
    }


     fn expect_identifier(&self) -> Result<SymbolIndex, Error> {
        let token = match self.current_token() {
            Some(value) => value,
            None => panic!("unreachable"),
        };

        if let TokenKind::Identifier(v) = token.token_kind {
            return Ok(v)
        }

        return Err(CompilerError::new(102, "unexpected token")
            .highlight(token.source_range)
                .note("expected identifier".to_string())
            .build())
    }

    
    fn parse_type(&mut self) -> Result<SourcedDataType, Error> {
        let current_token = self.current_token().unwrap();
        let source = current_token.source_range;
        
        // let mut string = match current_token.token_kind {
        //     TokenKind::Identifier(v) => v,

        //     _ => return {
        //         let source_range = current_token.source_range;
        //         self.advance();
        //         Err(CompilerError::new(104, "expected data type")
        //             .highlight(source_range)
        //             .build())
        //     }
        // };

        // while let Some(TokenKind::DoubleColon) = self.peek().map(|x| x.token_kind) {
        //     self.advance();
        //     self.advance();

        //     let identifier = self.expect_identifier()?;
        //     string = self.symbol_table.add_combo(string, identifier);
        // }


        // PERF: Obviously, cache this vec somewhere so it doesn't constantly realloc
        let mut string = vec![];
        loop {
            string.push(self.expect_identifier()?);

            if let Some(TokenKind::DoubleColon) = self.peek().map(|x| x.token_kind) {
                self.advance(); // identifier
                self.advance(); // double colon
                // self.advance(); // next identifier
                // loop
            } else { break }
        }

        let mut built_string = None;

        for i in string.into_iter().rev() {
            match built_string {
                Some(v) => built_string = Some(self.symbol_table.add_combo(i, v)),
                None => built_string = Some(i),
            }
        }

        let built_string = built_string.unwrap();        

        let data_type = match self.symbol_table.get(built_string).as_str() {
            "int" => DataType::Integer,
            "float" => DataType::Float,
            "bool" => DataType::Bool,
            "str" => DataType::String,
            
            _ => DataType::Struct(built_string)
        };

        Ok(SourcedDataType::new(SourceRange::new(source.start, self.current_token().unwrap().source_range.end), data_type))
    }
}

impl Parser<'_> {
    fn statement(&mut self) -> ParseResult {
        let current_token = match self.current_token() {
            Some(value) => value,
            None => panic!("how did we even get here?"),
        };

        match &current_token.token_kind {
            TokenKind::Keyword(keyword) => match keyword {
                Keyword::Var => self.var_declaration(),
                Keyword::Loop => self.loop_statement(),
                Keyword::While => self.while_statement(),

                Keyword::Namespace => self.namespace_declaration(),
                Keyword::Fn => self.function_declaration(),
                Keyword::Struct => self.struct_declaration(),

                Keyword::Using => self.using_declaration(),
                Keyword::Extern => self.extern_block(),

                Keyword::Return => {
                    let start = current_token.source_range.start;
                    self.advance();

                    let expression = self.expression()?;
                    
                    Ok(Instruction {
                        source_range: SourceRange::new(start, expression.source_range.end),
                        instruction_kind: InstructionKind::Statement(Statement::Return(Box::new(expression))),
                    })
                },

                Keyword::Break => Ok(Instruction {
                    instruction_kind: InstructionKind::Statement(Statement::Break),
                    source_range: self.current_token().unwrap().source_range
                }),

                Keyword::Continue => Ok(Instruction {
                    instruction_kind: InstructionKind::Statement(Statement::Continue),
                    source_range: self.current_token().unwrap().source_range
                }),


                
                _ => self.expression(),
            },

            _ => self.var_update(),
        }
    }


    fn struct_declaration(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::Struct))?;
        let start = self.current_token().unwrap().source_range.start;

        self.advance();
        
        let identifier = self.expect_identifier()?;

        self.advance();
        self.expect(&TokenKind::LeftBracket)?;
        
        self.advance();

        let mut fields = vec![];
        loop {
            if self.expect(&TokenKind::RightBracket).is_ok() {
                break
            }
            
            if !fields.is_empty() {
                self.expect(&TokenKind::Comma)?;
                self.advance();
            }

            if self.expect(&TokenKind::RightBracket).is_ok() {
                break
            }
            
            let name = match self.expect_identifier() {
                Ok(v) => v,
                Err(_) => break,
            };

            self.advance();
            self.expect(&TokenKind::Colon)?;

            self.advance();
            let data_type = self.parse_type()?;

            self.advance();

            fields.push((name, data_type));
        }

        
        self.expect(&TokenKind::RightBracket)?;

        Ok(Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::StructDeclaration { name: identifier, fields }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
        
    }
    

    fn function_declaration(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::Fn))?;
        let start = self.current_token().unwrap().source_range.start;

        self.advance();

        let identifier = self.expect_identifier()?;

        self.advance();
        self.expect(&TokenKind::LeftParenthesis)?;
        self.advance();

        let mut arguments = vec![];
        loop {
            if self.expect(&TokenKind::RightParenthesis).is_ok() {
                break
            }
            
            if !arguments.is_empty() {
                self.expect(&TokenKind::Comma)?;
                self.advance();
            }

            if self.expect(&TokenKind::RightParenthesis).is_ok() {
                break
            }
            
            
            let identifier = match self.expect_identifier() {
                Ok(v) => v,
                Err(_) => break,
            };

            self.advance();
            self.expect(&TokenKind::Colon)?;

            self.advance();
            let data_type = self.parse_type()?;

            self.advance();

            arguments.push((identifier, data_type));
        }


        self.expect(&TokenKind::RightParenthesis)?;
        
        self.advance();

        let return_type = if self.expect(&TokenKind::Colon).is_ok() {
            self.advance();
            let return_type = self.parse_type()?;
            
            self.advance();
            return_type
        } else {
            SourcedDataType::new(SourceRange::new(start, self.current_token().unwrap().source_range.end), DataType::Empty)
        };

        let declaration_end = self.current_token().unwrap().source_range.end;

        self.expect(&TokenKind::LeftBracket)?;
        self.advance();
        
        let body = self.parse_till(&TokenKind::RightBracket)?;
        
        Ok(Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::FunctionDeclaration {
                name: identifier,
                arguments,
                return_type,
                body,
                source_range_declaration: SourceRange::new(start, declaration_end),
            }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
    }


    fn var_declaration(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::Var))?;
        let start = self.current_token().unwrap().source_range.start;
        
        self.advance();

        let identifier = self.expect_identifier()?;
        
        self.advance();
        let type_hint = if self.expect(&TokenKind::Colon).is_ok() {
            self.advance();
            
            let datatype = self.parse_type()?;
            
            self.advance();
            Some(datatype)
        } else {
            None
        };
        self.expect(&TokenKind::Equals)?;

        self.advance();
        let expression = self.expression()?;
        
        Ok(Instruction {
            source_range: SourceRange::new(start, expression.source_range.end),
            instruction_kind: InstructionKind::Statement(Statement::DeclareVar { identifier, type_hint, data: Box::new(expression) }),
        })
    }

    
    fn loop_statement(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::Loop))?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();
        
        self.expect(&TokenKind::LeftBracket)?;
        self.advance();

        let body = self.parse_till(&TokenKind::RightBracket)?;

        Ok(Instruction {
            instruction_kind: InstructionKind::Statement(Statement::Loop { body }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end),
        })
    }


    fn while_statement(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::While))?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();

        let condition = self.expression()?;
        self.advance();

        self.expect(&TokenKind::LeftBracket)?;
        self.advance();

        let body = self.parse_till(&TokenKind::RightBracket)?;

        let source_range = SourceRange::new(start, self.current_token().unwrap().source_range.end);

        
        // This converts the usual while statement into a loop
        // i.e.
        // 
        // while x > 15 {
        //    do_stuff()
        // }
        //
        // into:
        //
        // loop {
        //     if x > 15 {
        //        do_stuff()
        //     } else {
        //        break
        //     }
        // }
        
        let if_statement = Instruction {
            instruction_kind: InstructionKind::Expression(Expression::IfExpression {
                body,
                condition: Box::new(condition),
                else_part: Some(Box::new(Instruction {
                    instruction_kind: InstructionKind::Expression(Expression::Block {
                        body: vec![Instruction {
                            instruction_kind: InstructionKind::Statement(Statement::Break),
                            source_range
                        }]
                    }),
                    source_range 
                })),
            }),
            source_range
        };
        
        Ok(Instruction {
            instruction_kind: InstructionKind::Statement(Statement::Loop { body: vec![if_statement] }),
            source_range,
        })
    }
    

    fn var_update(&mut self) -> ParseResult {
        let left = self.expression()?;

        if self.peek().is_none() || self.peek().unwrap().token_kind != TokenKind::Equals {
            return Ok(left)
        }

        self.advance(); // =
        self.advance();

        let right = self.expression()?;

        match left.instruction_kind {
            InstructionKind::Expression(Expression::Identifier(_)) => {
                Ok(Instruction {
                    source_range: SourceRange::new(left.source_range.start, right.source_range.end), 
                    instruction_kind: InstructionKind::Statement(Statement::VariableUpdate { 
                        left: Box::new(left), 
                        right: Box::new(right)
                    })
                })                
            }


            InstructionKind::Expression(Expression::AccessStructureData { structure, identifier, index_to }) => {
                Ok(Instruction {
                    source_range: SourceRange::new(left.source_range.start, right.source_range.end), 
                    instruction_kind: InstructionKind::Statement(Statement::FieldUpdate {
                        structure,
                        right: Box::new(right),
                        identifier,
                        index_to,
                    })
                })
            }

            
            _ => Err(CompilerError::new(103, "invalid assignment value")
                    .highlight(left.source_range)
                        .note("this is not one of the following: identifier, field access".to_string())
                    .build()
            )
        }

    }


    fn namespace_declaration(&mut self) -> ParseResult {
        fn namespace_rename(symbol_table: &mut SymbolTable, namespace: SymbolIndex, i: &mut Instruction) {
            match &mut i.instruction_kind {
                InstructionKind::Declaration(Declaration::FunctionDeclaration { name, .. } | Declaration::StructDeclaration { name, .. }) => {
                    print!("{:?} -> ", name);
                    *name = symbol_table.add_combo(namespace, *name);
                    println!("{name:?}");
                    // *name = symbol_table.add_combo(namespace, *name)
                }
                
                InstructionKind::Declaration(Declaration::Namespace { body, identifier }) => {
                    print!("{:?} -> ", identifier);
                    *identifier = symbol_table.add_combo(namespace, *identifier);
                    println!("{identifier:?}");
                    
                    body.iter_mut().for_each(|x| namespace_rename(symbol_table, namespace, x));
                    // body.iter_mut().for_each(|x| namespace_rename(symbol_table, *identifier, x));
                },

                InstructionKind::Declaration(Declaration::Extern { functions, .. }) => {
                    for f in functions {
                        f.identifier = symbol_table.add_combo(namespace, f.identifier);
                    }
                }

                _ => todo!()
            }
            
        }
        
        self.expect(&TokenKind::Keyword(Keyword::Namespace))?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();

        let identifier = self.expect_identifier()?;
        self.advance();

        self.expect(&TokenKind::LeftBracket)?;
        self.advance();

        let mut body = vec![];
        let mut errors = vec![];
        loop {
            if self.current_token().is_none() {
                break
            }
            
            if self.expect(&TokenKind::RightBracket).is_ok() {
                break
            }

            let token = self.current_token().unwrap();

            let v = match token.token_kind {
                TokenKind::Keyword(Keyword::Namespace) => self.namespace_declaration(),
                TokenKind::Keyword(Keyword::Fn) => self.function_declaration(),
                TokenKind::Keyword(Keyword::Struct) => self.struct_declaration(),
                TokenKind::Keyword(Keyword::Extern) => self.extern_block(),

                
                _ => Err(CompilerError::new(105, "invalid statement in namespace")
                    .highlight(token.source_range)
                        .note("only the following are allowed: function declarations, namespaces, structure declarations".to_string())
                    .build())
            };

            match v {
                Ok(v) => body.push(v),
                Err(e) => errors.push(e),
            };
            self.advance();
        }

        if !errors.is_empty() {
            return Err(errors.combine_into_error())
        }

        self.expect(&TokenKind::RightBracket)?;

        for i in body.iter_mut() {
            namespace_rename(self.symbol_table, identifier, i)
        }

        dbg!(&self.symbol_table);
        

        Ok(Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::Namespace { body, identifier }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
    }


    fn extern_block(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::Extern))?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();

        let path = match self.current_token().map(|x| x.token_kind).unwrap() {
            TokenKind::Literal(Literal::String(v)) => v,
            _ => return Err(CompilerError::new(107, "expected a constant string")
                    .highlight(self.current_token().unwrap().source_range)
                        .note("..because of the `extern` keyword before".to_string())
                    .build())
        };
        self.advance();

        self.expect(&TokenKind::LeftBracket)?;
        self.advance();

        let mut functions = vec![];
        loop {
            if self.expect(&TokenKind::RightBracket).is_ok() {
                break
            }

            self.expect(&TokenKind::Keyword(Keyword::Fn))?;
            self.advance();
            
            let name = self.expect_identifier()?;
            self.advance();

            self.expect(&TokenKind::LeftParenthesis)?;
            self.advance();

            
            let mut arguments = vec![];
            loop {
                if self.expect(&TokenKind::RightParenthesis).is_ok() {
                    break
                }
            
                if !arguments.is_empty() {
                    self.expect(&TokenKind::Comma)?;
                    self.advance();
                }

                if self.expect(&TokenKind::RightParenthesis).is_ok() {
                    break
                }
            
                let data_type = self.parse_type()?;

                self.advance();

                arguments.push(data_type);
            }
            
            self.expect(&TokenKind::RightParenthesis)?;

            
            let return_type = if let Some(TokenKind::Colon) = self.peek().map(|x| x.token_kind) {
                self.advance();
                self.advance();
                self.parse_type()?
            } else { SourcedDataType::new(SourceRange::new(start, self.current_token().unwrap().source_range.end), DataType::Empty) };

            self.advance();

            functions.push(ExternFunctionAST {
                raw_name: name,
                identifier: name,
                return_type,
                arguments,
            });
        }

        self.expect(&TokenKind::RightBracket)?;

        Ok(Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::Extern { file: path, functions }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
    }


    fn using_declaration(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::Using))?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();

        let string = self.expect_identifier()?;

        Ok(Instruction {
            instruction_kind: InstructionKind::Declaration(Declaration::UseFile { file_name: string }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
    }
}

impl Parser<'_> {
    fn expression(&mut self) -> ParseResult {
        self.comparison_expression()
    }

    fn comparison_expression(&mut self) -> ParseResult {
        self.binary_operation(
            Parser::arithmetic_expression,
            Parser::arithmetic_expression,
            &[
                TokenKind::LeftAngle,
                TokenKind::RightAngle,
                TokenKind::GreaterEquals,
                TokenKind::LesserEquals,
                TokenKind::EqualsTo,
                TokenKind::NotEqualsTo,
            ]
        )
    }

    fn arithmetic_expression(&mut self) -> ParseResult {
        self.binary_operation(
            Parser::product_expression, 
            Parser::product_expression,
            &[
                TokenKind::Plus,
                TokenKind::Minus,
            ],
        )
    }

    fn product_expression(&mut self) -> ParseResult {
         self.binary_operation(
            Parser::accessor, 
            Parser::accessor,
            &[
                TokenKind::Star,
                TokenKind::Slash,
            ],
        )       
    }

    
    fn accessor(&mut self) -> ParseResult {
        let mut atom = self.atom()?;

        while let Some(TokenKind::Dot) = self.peek().map(|x| x.token_kind) {
            self.advance();
            self.advance();
            
            let identifier = self.expect_identifier()?;

            atom = Instruction {
                source_range: SourceRange::combine(atom.source_range, self.current_token().unwrap().source_range),
                instruction_kind: InstructionKind::Expression(Expression::AccessStructureData { structure: Box::new(atom), identifier, index_to: usize::MAX }),
            }
        }
        
        Ok(atom)
    }


    fn atom(&mut self) -> ParseResult {
        let token = match self.current_token() {
            Some(token) => token,
            None => panic!("uh oh")
        };

        match &token.token_kind {
            TokenKind::Literal(_) => {
                let literal = match token.token_kind {
                    TokenKind::Literal(literal) => literal,
                    _ => unreachable!()
                };
                
                let data = match literal {
                    Literal::Integer(i) => Data::Int(i),
                    Literal::Float(f) => Data::Float(f),
                    Literal::String(s) => Data::String(s),
                    Literal::Bool(b) => Data::Bool(b),
                };

                Ok(Instruction {
                    instruction_kind: InstructionKind::Expression(Expression::Data(SourcedData::new(token.source_range, data))),
                    source_range: token.source_range,
                })
            }
            
            
            TokenKind::Keyword(Keyword::If) => self.if_expression(),
            
            
            TokenKind::Identifier(_) => {
                let token = self.current_token().unwrap();

                let v = match token.token_kind {
                    TokenKind::Identifier(identifier) => identifier,
                    _ => unreachable!()
                };

                
                if let Some(v) = self.peek().map(|x| x.token_kind) {
                    if v == TokenKind::LeftParenthesis  {
                        return self.function_call()
                    }

                    if v == TokenKind::LeftBracket {
                        return self.structure_creation()
                    }

                    if v == TokenKind::DoubleColon {
                        return self.do_within_namespace()
                    }
                    
                    
                }

                
                Ok(Instruction { instruction_kind: InstructionKind::Expression(Expression::Identifier(v)), source_range: self.current_token().unwrap().source_range })
            },


            TokenKind::LeftParenthesis => {
                let start = token.source_range.start;
                self.advance();

                if self.expect(&TokenKind::RightParenthesis).is_ok() {
                    let source_range = SourceRange::new(start, self.current_token().unwrap().source_range.end);
                    return Ok(Instruction {
                        instruction_kind: InstructionKind::Expression(Expression::Data(SourcedData::new(source_range, Data::Empty))),
                        source_range,
                    })
                }

                let expr = self.expression()?;
                self.advance();
                
                self.expect(&TokenKind::RightParenthesis)?;

                Ok(expr)
            }

            
            TokenKind::LeftBracket => self.block_expression(),

            TokenKind::Underscore => Ok(Instruction {
                instruction_kind: InstructionKind::Expression(Expression::Data(SourcedData::new(token.source_range, Data::Empty))),
                source_range: token.source_range,
            }),
            

            _ => {
                let return_val = Err(
                    CompilerError::new(101, "expected an expression")
                        .highlight(token.source_range)
                        .build()
                );
                
                return_val
            },
        }
    }
}

impl<'a> Parser<'a> {
    fn binary_operation(
        &mut self,
        left_func : fn(&mut Parser<'a>) -> ParseResult,
        right_func: fn(&mut Parser<'a>) -> ParseResult,
        operators : &[TokenKind],
    ) -> ParseResult {
        let mut base = left_func(self)?;

        loop {
            if self.peek().is_none() || !operators.contains(&self.peek().unwrap().token_kind) {
                break
            }
            
            let token = self.advance().unwrap();

            let operator = BinaryOperator::from_token(&token.token_kind).expect("invalid function call input (parser)");

            self.advance();

            let right = right_func(self)?;

            base = Instruction {
                source_range: SourceRange::new(base.source_range.start, right.source_range.end),
                instruction_kind: InstructionKind::Expression(Expression::BinaryOp {
                    operator,
                    left: Box::new(base),
                    right: Box::new(right),
                }),
            }
        }

        Ok(base)
    }
}

impl Parser<'_> {
    fn block_expression(&mut self) -> ParseResult {
        self.expect(&TokenKind::LeftBracket)?;
        let start = self.current_token().unwrap().source_range.start;
        
        self.advance();

        let body = self.parse_till(&TokenKind::RightBracket)?;

        Ok(Instruction {
            instruction_kind: InstructionKind::Expression(Expression::Block { body }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
    }
    

    fn if_expression(&mut self) -> ParseResult {
        self.expect(&TokenKind::Keyword(Keyword::If))?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();
        
        let condition = self.expression()?;
        self.advance();

        self.expect(&TokenKind::LeftBracket)?;
        self.advance();
        
        let block = self.parse_till(&TokenKind::RightBracket)?;

        let if_end = self.current_token().unwrap().source_range.end;

        if self.peek().is_some() && self.peek().unwrap().token_kind == TokenKind::Keyword(Keyword::Else) {
            self.advance();
            self.advance();
            
            let else_part = if self.expect(&TokenKind::Keyword(Keyword::If)).is_ok() {
                self.if_expression()?
            } else {
                self.block_expression()?
            };

            return Ok(Instruction {
                source_range: SourceRange::new(start, if_end),
                instruction_kind: InstructionKind::Expression(Expression::IfExpression { body: block, condition: Box::new(condition), else_part: Some(Box::new(else_part)) }),
            })
        }

        Ok(Instruction {
            instruction_kind: InstructionKind::Expression(Expression::IfExpression { body: block, condition: Box::new(condition), else_part: None }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end)
        })
        
    }


    fn function_call(&mut self) -> ParseResult {
        let identifier = self.expect_identifier()?;
        let start = self.current_token().unwrap().source_range.start;

        self.advance();
        self.expect(&TokenKind::LeftParenthesis)?;
        self.advance();

        let mut arguments = vec![];
        loop {
            if self.expect(&TokenKind::RightParenthesis).is_ok() {
                break
            }
            
            if !arguments.is_empty() {
                self.expect(&TokenKind::Comma)?;
                self.advance();
            }

            if self.expect(&TokenKind::RightParenthesis).is_ok() {
                break
            }
            
            let expression = self.expression()?;

            self.advance();
            
            arguments.push(expression);
        }
        
        self.expect(&TokenKind::RightParenthesis)?;

        Ok(Instruction {
            instruction_kind: InstructionKind::Expression(Expression::FunctionCall {
                identifier,
                arguments,
            }),
            source_range: SourceRange::new(start, self.current_token().unwrap().source_range.end),
        })
    }


    fn structure_creation(&mut self) -> ParseResult {
        let identifier = self.expect_identifier()?;
        let start = self.current_token().unwrap().source_range.start;
        let identifier_range = self.current_token().unwrap().source_range;
        self.advance();
        
        self.expect(&TokenKind::LeftBracket)?;
        self.advance();

        let mut fields = vec![];
        loop {
            if self.expect(&TokenKind::RightBracket).is_ok() {
                break
            }
            
            if !fields.is_empty() {
                self.expect(&TokenKind::Comma)?;
                self.advance();
            }

            if self.expect(&TokenKind::RightBracket).is_ok() {
                break
            }

            let identifier = self.expect_identifier()?;
            
            self.advance();
            self.expect(&TokenKind::Colon)?;
            self.advance();
            
            let expression = self.expression()?;

            self.advance();
            
            fields.push((identifier, expression));
        }

        self.expect(&TokenKind::RightBracket)?;
        
        Ok(Instruction {
            instruction_kind: InstructionKind::Expression(Expression::StructureCreation { identifier, fields, identifier_range }),
            source_range: SourceRange { start, end: self.current_token().unwrap().source_range.end }
        })
    }


    fn do_within_namespace(&mut self) -> ParseResult {
        let namespace = self.expect_identifier()?;
        let start = self.current_token().unwrap().source_range.start;
        self.advance();

        self.expect(&TokenKind::DoubleColon)?;
        self.advance();

        let mut expression = self.expression()?;

        expression.source_range.start = start;
        match &mut expression.instruction_kind {
            InstructionKind::Expression(v) => match v {
                | Expression::StructureCreation { identifier, .. }
                | Expression::FunctionCall { identifier, .. } => {
                    *identifier = self.symbol_table.add_combo(namespace, *identifier)
                },

                // Expression::AccessStructureData { structure, .. } => {
                //     match &mut structure.instruction_kind {
                //         InstructionKind::Expression(Expression::StructureCreation { identifier, identifier_range, .. }) => {
                //             *identifier = self.symbol_table.add_combo(namespace, *identifier);
                //             identifier_range.start = start;
                        
                //         }

                //         _ => todo!()
                //     }
                // }


                _ => return Err(CompilerError::new(105, "invalid expression in namespace")
                    .highlight(expression.source_range)
                        .note("only function calls are allowed".to_string())
                    .build())
            },
            _ => unreachable!()
        }

        Ok(expression)
    }
    
}