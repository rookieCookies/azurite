use std::str::Chars;

use azurite_errors::{CompilerError, ErrorBuilder, Error, CombineIntoError};
use common::{SymbolTable, SymbolIndex, SourceRange};

mod tests;


#[derive(Debug, PartialEq)]
pub struct Token {
    pub token_kind: TokenKind,
    pub source_range: SourceRange,
}


#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TokenKind {
    LeftParenthesis,
    RightParenthesis,

    LeftAngle,
    RightAngle,

    LeftBracket,
    RightBracket,

    LeftSquare,
    RightSquare,

    Percent,
    Slash,
    Plus,
    Minus,
    Star,
    Caret,
    Colon,
    DoubleColon,
    Comma,
    Dot,
    Bang,
    Equals,
    Underscore,

    Literal(Literal),
    Keyword(Keyword),
    Identifier(SymbolIndex),

    LesserEquals,
    GreaterEquals,
    EqualsTo,
    NotEqualsTo,
    LogicalOr,
    LogicalAnd,

    AddEquals,
    SubEquals,
    MulEquals,
    DivEquals,
    
    EndOfFile,
}


#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(SymbolIndex),
    Bool(bool),
}


#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Keyword {
    Fn,
    Struct,
    Impl,
    Namespace,
    Extern,
    Using,
    If,
    Else,
    While,
    For,
    Loop,
    Break,
    Continue,
    Var,
    Return,
    As,
    Const,
}


#[derive(Debug)]
struct Lexer<'a> {
    characters: Chars<'a>,
    index: usize,

    character_index: usize,
    current: Option<char>,
    stale: bool,

    string_storage: String,
    symbol_table: &'a mut SymbolTable,
    file: SymbolIndex,
}


///
/// Lexer
///
/// # Panics: 
///   If `data` includes any `\t` or `\r` characters this function will panic  
///   `\t` should be converted to spaces while any mention of `\r` should be
///   stripped out
///
/// # Arguments:
///   - `data`: The file source code
///   - `file`:
///     - The file name (without extension) the errors will be created with
///     - This can be obtained by using the `SymbolTable::add` method
///   - `symbol_table`:
///     - **MUTABILITY**: Appends newly encountered strings and identifier
///     - Check the `SymbolTable` docs to see how to create it
///
/// # Return Value:
///   - The return value either returns a vector with the tokens or
///     the lexing errors that occurred while lexing.
///
pub fn lex(
    data: &str,
    file: SymbolIndex,
    symbol_table: &mut SymbolTable
) -> Result<Vec<Token>, Error> {
    
    let mut lexer = Lexer {
        characters: data.chars(),
        index: 0,
        current: None,
        stale: false,
        string_storage: String::with_capacity(128),
        character_index: 0,
        symbol_table,
        file,
    };

    let mut tokens = vec![];
    let mut errors = vec![];

    while let Some(value) = lexer.advance() {
        let start = lexer.character_index;

        let token_kind = match value {
            '0'..='9' => {
                let parsed_number = lexer.number();
                lexer.stale = true;
                match parsed_number {
                    Ok(value) => TokenKind::Literal(value),
                    Err(error) => {
                        errors.push(error);
                        continue;

                    },
                }
            }


            '\n' | ' ' => continue,

            '"' => match lexer.string() {
                Ok(value) => TokenKind::Literal(value),
                Err(mut error) => {
                    errors.append(&mut error);
                    continue;
                }
            },

            '/' => match lexer.peek() {
                Some('/') => {
                    while let Some(value) = lexer.current_character() {
                        if value == '\n' {
                            break;
                        }
                        lexer.advance();
                    }
                    continue;
                }
                Some('=') => {
                    lexer.advance();
                    TokenKind::DivEquals
                }
                _ => TokenKind::Slash,
            },

            'a'..='z' | 'A'..='Z' => lexer.identifier(),

            '(' => TokenKind::LeftParenthesis,
            ')' => TokenKind::RightParenthesis,
            '<' => lexer.next_matches('=', TokenKind::LesserEquals, TokenKind::LeftAngle),
            '>' => lexer.next_matches('=', TokenKind::GreaterEquals, TokenKind::RightAngle),
            '&' if lexer.peek() == Some('&') => {
                lexer.advance();
                TokenKind::LogicalAnd
            },
            '|' if lexer.peek() == Some('|') => {
                lexer.advance();
                TokenKind::LogicalOr
            },
            '{' => TokenKind::LeftBracket,
            '}' => TokenKind::RightBracket,
            '[' => TokenKind::LeftSquare,
            ']' => TokenKind::RightSquare,
            '%' => TokenKind::Percent,
            '+' => lexer.next_matches('=', TokenKind::AddEquals, TokenKind::Plus),
            '-' => lexer.next_matches('=', TokenKind::SubEquals, TokenKind::Minus),
            '*' => lexer.next_matches('=', TokenKind::MulEquals, TokenKind::Star),
            '^' => TokenKind::Caret,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            ':' => lexer.next_matches(':', TokenKind::DoubleColon, TokenKind::Colon),
            '=' => lexer.next_matches('=', TokenKind::EqualsTo, TokenKind::Equals),
            '!' => lexer.next_matches('=', TokenKind::NotEqualsTo, TokenKind::Bang),

            
            '_' => {
                if let Some('a'..='z' | 'A'..='Z' | '_' | '0'..='9') = lexer.peek() {
                    lexer.identifier()
                } else {
                    TokenKind::Underscore
                }
            },


            '\t' => panic!("compiler error! tab character wasn't converted"),
            '\r' => panic!("compiler error! carriage return character wasn't converted"),
            
            
            _ => {
                errors.push(CompilerError::new(lexer.file, 1, "invalid character")
                    .highlight(SourceRange::new(start, start))
                        .note(format!("{value:?}"))
                    .build());
                continue;
            }
        };

        let end = lexer.character_index - lexer.stale as usize;

        let token = Token {
            token_kind,
            source_range: SourceRange { start, end },
        };

        tokens.push(token);
    }

    let end = lexer.character_index.saturating_sub(1);

    tokens.push(Token {
        token_kind: TokenKind::EndOfFile,
        source_range: SourceRange {
            start: end,
            end,
        },
    });

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors.combine_into_error())
    }
}


// utility methods
impl Lexer<'_> {
    pub(crate) fn advance(&mut self) -> Option<char> {
        if self.stale {
            self.stale = false;
            return self.current;
        }
        
        self.index += 1;

        if let Some(v) = self.current {
            self.character_index += v.len_utf8();
        }
        
        self.current = self.characters.next();
        self.current
    }


    fn current_character(&self) -> Option<char> {
        self.current
    }


    pub(crate) fn peek(&mut self) -> Option<char> {
        self.characters.clone().next()
    }


    // # Safety:
    //   - It is the responsibility of the caller to
    //     properly call `Lexer::return_string_storage`
    //     on all code-paths and not use this multiple
    //     times without returning.
    fn borrow_string_storage(&mut self) -> String {
        self.string_storage.clear();
        std::mem::take(&mut self.string_storage)
    }


    fn return_string_storage(&mut self, string: String) {
        self.string_storage = string;
    }

    
    fn next_matches(&mut self, matches: char, yes: TokenKind, no: TokenKind) -> TokenKind {
        if self.peek() == Some(matches) {
            self.advance();
            return yes
        }
        no
    }
}

impl Lexer<'_> {
    fn identifier(&mut self) -> TokenKind {
        let mut string = self.borrow_string_storage();

        string.push(self.current_character().unwrap());

        while let Some(value) = self.advance() {
            match value {
                'a'..='z' | 'A'..='Z' | '_' | '0'..='9' => string.push(value),
                _ => break,
            }
        }
        self.stale = true;

        let token = match string.as_str() {
            "true" => TokenKind::Literal(Literal::Bool(true)),
            "false" => TokenKind::Literal(Literal::Bool(false)),

            "fn" => TokenKind::Keyword(Keyword::Fn),
            "struct" => TokenKind::Keyword(Keyword::Struct),
            "impl" => TokenKind::Keyword(Keyword::Impl),
            // "namespace" => TokenKind::Keyword(Keyword::Namespace),
            "using" => TokenKind::Keyword(Keyword::Using),
            "extern" => TokenKind::Keyword(Keyword::Extern),
            "if" => TokenKind::Keyword(Keyword::If),
            "else" => TokenKind::Keyword(Keyword::Else),
            "while" => TokenKind::Keyword(Keyword::While),
            "for" => TokenKind::Keyword(Keyword::For),
            "loop" => TokenKind::Keyword(Keyword::Loop),
            "continue" => TokenKind::Keyword(Keyword::Continue),
            "break" => TokenKind::Keyword(Keyword::Break),
            "return" => TokenKind::Keyword(Keyword::Return),
            "var" => TokenKind::Keyword(Keyword::Var),
            "as" => TokenKind::Keyword(Keyword::As),
            "const" => TokenKind::Keyword(Keyword::Const),

            _ => {
                let index = self.symbol_table.add(String::from(&string));
                
                TokenKind::Identifier(index)
            },
        };

        self.return_string_storage(string);

        token
    }

    
    fn string(&mut self) -> Result<Literal, Vec<Error>> {
        let mut string = String::new();
        let start = self.character_index;

        let mut errors = vec![];

        let mut is_in_escape = false;
        while let Some(value) = self.advance() {
            if is_in_escape {
                match value {
                    'n' => string.push('\n'),
                    'r' => string.push('\r'),
                    't' => string.push('\t'),
                    '\\' => string.push('\\'),
                    '0' => string.push('\0'),
                    '"' => string.push('"'),

                    'u' => match self.unicode_escape_character() {
                        Ok(val) => string.push(val),
                        Err(err) => {
                            errors.push(err);
                        },
                    },

                    _ => string.push(value),
                }

                is_in_escape = false;

                continue;
            }

            match value {
                '\\' => is_in_escape = true,
                '"' => break,
                _ => string.push(value),
            }
        }

        if self.current_character() != Some('"') {
            errors.push(CompilerError::new(self.file, 2, "unterminated string")
                .highlight(SourceRange::new(start, self.character_index))
                    .note("consider adding a quotation mark here".to_string())

                .build()
            );
        }

        if errors.is_empty() {
            let index = self.symbol_table.add(string);
            return Ok(Literal::String(index));
        }

        Err(errors)
    }


    fn unicode_escape_character(&mut self) -> Result<char, Error> {
        if self.advance() != Some('{') {
            self.stale = true;
            return Err(CompilerError::new(self.file, 3, "corrupt unicode escape")
                .highlight(SourceRange::new(self.character_index, self.character_index))
                    .note("unicode escapes are formatted like \\u{..}".to_string())

                .build()
            );
        }

        let start = self.character_index;
        
        let mut unicode = self.borrow_string_storage();

        while let Some(value) = self.advance() {
            match value {
                '}' => break,

                '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | 'A' | 'B' | 'C'
                | 'D' | 'E' | 'F' => unicode.push(value),

                _ => return Err(CompilerError::new(self.file, 4, "invalid unicode value")
                    .highlight(SourceRange::new(self.character_index, self.character_index))
                        .note("unicode escape values must be written in base-16 (0-1-2-3-4-5-6-7-8-9-A-B-C-D-E-F)".to_string())
                    
                    .build()
                ),
            }
        }

        let number = self.base_n_number_conversion(16, &unicode)?;

        self.return_string_storage(unicode);

        match char::from_u32(number as u32) {
            Some(value) => Ok(value),
            None => Err(CompilerError::new(self.file, 7, "isn't a valid unicode character")
                    .highlight(SourceRange::new(start, self.character_index))
                    .build()
                ),
        }
    }


    fn number(&mut self) -> Result<Literal, Error> {
        if self.current_character() == Some('0') {
            match self.peek() {
                Some('b') => {
                    self.advance();
                    self.advance();
                    self.base_n_number(2)
                }

                Some('o') => {
                    self.advance();
                    self.advance();
                    self.base_n_number(8)
                }

                Some('x') => {
                    self.advance();
                    self.advance();
                    self.base_n_number(16)
                }

                _ => self.base_n_number(10),
            }
        } else {
            self.base_n_number(10)
        }
    }


    fn base_n_number(&mut self, base: u32) -> Result<Literal, Error> {
        if base > 16 {
            panic!("invalid base number provided by the compiler")
        }

        let mut number_string = self.borrow_string_storage();
        let mut dot_count = 0;
        let start = self.character_index;

        while let Some(value) = self.current_character() {
            match map_to_hex(value) {
                Some(n) if base < n as u32 + 1 => 
                    return Err(CompilerError::new(self.file, 6, "invalid number for base")
                        .highlight(SourceRange::new(self.character_index, self.character_index))
                            .note(format!("the value {value} is too big for a base-{base} number"))

                        .build()),

                Some(_) => (),
                _ => match value {
                    '.' => dot_count += 1,
                    '_' => {
                        self.advance();
                        continue;
                    }
                    _ => break,
                }
            }

            number_string.push(value);
            self.advance();
        }

        if dot_count > 1 {
            self.return_string_storage(number_string);

            return Err(CompilerError::new(self.file, 8, "too many dots")
                .highlight(SourceRange::new(start, self.character_index-1))
                .build()
            );
        }

        let (full_number, decimals) = number_string
            .split_once('.')
            .unwrap_or((&number_string, ""));

        let number = self.base_n_number_conversion(base, full_number)?;

        if !decimals.is_empty() {
            let mut decimal = 0.0;
            for (index, value) in decimals.chars().enumerate() {
                let digit = value.to_digit(base).expect("unreachable") as f64;
                let power = -(index as i32) - 1;

                decimal += (base as f64).powi(power) * digit;
            }

            self.return_string_storage(number_string);
            return Ok(Literal::Float(number as f64 + decimal));
        }

        self.return_string_storage(number_string);
        Ok(Literal::Integer(number))
    }
}


impl Lexer<'_> {
    fn base_n_number_conversion(&self, base: u32, text: &str) -> Result<i64, Error> {
        let mut number : i64 = 0;
        let start = self.index - text.len() - 1;

        
        for (index, value) in text.chars().rev().enumerate() {
            let digit = value.to_digit(base).expect("unreachable") as i64;
            let power = index as u32;

            let power = match (base as i64).checked_pow(power) {
                Some(value) => value,
                None => return Err(CompilerError::new(self.file, 5, "number is too large")
                    .highlight(SourceRange::new(start, self.character_index-1))
                    .build()
                ),
            };

            let result : i64 = match power.checked_mul(digit) {
                Some(value) => value,
                None => return Err(CompilerError::new(self.file, 5, "number is too large")
                    .highlight(SourceRange::new(start, self.character_index-1))
                    .build()),
            };

            number = match number.checked_add(result) {
                Some(value) => value,
                None => return Err(CompilerError::new(self.file, 5, "number is too large")
                    .highlight(SourceRange::new(start, self.character_index-1))
                    .build()),
            };
        }

        Ok(number)
    }

    
}


fn map_to_hex(character: char) -> Option<u8> {
    match character {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'A' => Some(10),
        'B' => Some(11),
        'C' => Some(12),
        'D' => Some(13),
        'E' => Some(14),
        'F' => Some(15),
        _ => None
    }       
}