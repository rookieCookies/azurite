use std::fmt::Display;

use crate::error::{Error, Highlight, FATAL};

#[derive(Debug, PartialEq)]
pub enum TokenType {
    Slash,
    Plus,
    Comma,
    Minus,
    Star,
    Carrot,
    Colon,
    DoubleColon,
    Equals,
    ExclamationMark,
    LeftParenthesis,
    RightParenthesis,
    LeftCurly,
    RightCurly,
    Arrow,
    Dot,

    EqualsEquals,
    NotEquals,
    RightAngle,
    LeftAngle,
    GreaterEquals,
    LesserEquals,

    AddAssign,
    SubtractAssign,
    MultiplyAssign,
    DivideAssign,
    PowerAssign,

    Integer(i64),
    Float(f64),
    String(String),
    Identifier(String),

    True,
    False,
    
    Var,
    If,
    Else,
    While,
    Fn,
    Return,
    Struct,
    Impl,
    Raw,
    Using,
    Inline,
    Bytecode,

    EndOfFile,
}

impl Display for TokenType {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[derive(Debug)]
pub struct Token {
    pub token_type: TokenType,
    pub start: u32,
    pub end: u32,
    pub line: u32,
}

enum CommentMode {
    SingleLine,
    MultiLine,
    None,
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
    line: usize,
    current_file: String,
}

macro_rules! unwrap (
    ($s:expr, $e:expr) => {
        match $s {
            Ok(v) => v,
            Err(mut v) => {
                $e.append(&mut v);
                continue
            }
        }
    }
);

#[allow(clippy::too_many_lines)]
#[allow(clippy::cast_possible_truncation)]
pub fn lex(data: Vec<char>, current_file: String) -> Result<Vec<Token>, Vec<Error>> {
    let mut lexer = Lexer {
        chars: data,
        index: 0,
        line: 0,
        current_file,
    };
    let mut errors = Vec::new();

    let mut tokens: Vec<Token> = Vec::with_capacity(lexer.chars.len());
    let mut comment_mode = CommentMode::None;
    while let Some(chr) = lexer.current_char() {
        let start = lexer.index;

        // Handle comments
        match comment_mode {
            CommentMode::SingleLine => {
                if chr == &'\n' {
                    comment_mode = CommentMode::None;
                    lexer.line += 1;
                }
                lexer.advance();
                continue;
            }
            CommentMode::MultiLine => {
                if chr == &'*' {
                    if let Some('/') = lexer.peek() {
                        lexer.advance();
                        comment_mode = CommentMode::None;
                    }
                }
                lexer.advance();
                continue;
            }
            CommentMode::None => (),
        }

        let token = match chr {
            '/' => match lexer.handle_comment_activation(&mut comment_mode) {
                Some(v) => v,
                None => continue,
            },
            '+' => lexer.check('=', TokenType::AddAssign, TokenType::Plus),
            '-' => {
                let value = lexer.check('=', TokenType::SubtractAssign, TokenType::Minus);
                if value == TokenType::SubtractAssign {
                    value
                } else {
                    lexer.check('>', TokenType::Arrow, TokenType::Minus)
                }
            }
            '*' => lexer.check('=', TokenType::MultiplyAssign, TokenType::Star),
            '^' => lexer.check('=', TokenType::PowerAssign, TokenType::Carrot),
            '(' => TokenType::LeftParenthesis,
            ')' => TokenType::RightParenthesis,
            '{' => TokenType::LeftCurly,
            '}' => TokenType::RightCurly,
            ',' => TokenType::Comma,
            '.' => TokenType::Dot,
            '=' => lexer.check('=', TokenType::EqualsEquals, TokenType::Equals),
            '!' => lexer.check('=', TokenType::NotEquals, TokenType::ExclamationMark),
            '>' => lexer.check('=', TokenType::GreaterEquals, TokenType::RightAngle),
            '<' => lexer.check('=', TokenType::LesserEquals, TokenType::LeftAngle),
            ':' => lexer.check(':', TokenType::DoubleColon, TokenType::Colon),
            '0'..='9' => unwrap!(lexer.number(), &mut errors),
            'a'..='z' | 'A'..='Z' => lexer.identifier(),
            '"' => unwrap!(lexer.string(), &mut errors),
            ' ' | '\r' => {
                lexer.advance();
                continue;
            }
            '\n' => {
                lexer.advance();
                lexer.line += 1;
                continue;
            }
            _ => {
                errors.push(Error::new(
                    vec![(start as u32, lexer.index as u32, Highlight::Red)],
                    "unknown character",
                    format!(
                        "this character ({chr:?}) is not a valid character, please check the docs"
                    ),
                    &FATAL,
                    lexer.current_file.clone(),
                ));
                lexer.advance();
                continue;
            }
        };
        tokens.push(Token {
            token_type: token,
            start: start as u32,
            end: lexer.index as u32,
            line: lexer.line as u32,
        });
        lexer.advance();
    }

    tokens.push(Token {
        token_type: TokenType::EndOfFile,
        start: lexer.index.max(1) as u32 - 1,
        end: lexer.index.max(1) as u32 - 1,
        line: lexer.line as u32,
    });

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

impl Lexer {
    fn advance(&mut self) -> Option<&char> {
        self.index += 1;
        self.current_char()
    }

    fn retreat(&mut self) -> Option<&char> {
        self.index -= 1;
        self.current_char()
    }

    fn current_char(&self) -> Option<&char> {
        self.chars.get(self.index)
    }

    fn peek(&self) -> Option<&char> {
        self.chars.get(self.index + 1)
    }

    fn check(&mut self, expect: char, yes: TokenType, no: TokenType) -> TokenType {
        if self.peek() == Some(&expect) {
            self.advance();
            yes
        } else {
            no
        }
    }

    fn handle_comment_activation(&mut self, comment_mode: &mut CommentMode) -> Option<TokenType> {
        if let Some(peek) = self.peek() {
            if peek == &'/' {
                *comment_mode = CommentMode::SingleLine;
                return None;
            } else if peek == &'*' {
                *comment_mode = CommentMode::MultiLine;
                return None;
            }
        }
        Some(self.check('=', TokenType::DivideAssign, TokenType::Slash))
    }

    #[allow(clippy::cast_possible_truncation)]
    fn number(&mut self) -> Result<TokenType, Vec<Error>> {
        let start = self.index;
        let mut number_str = String::with_capacity(8);
        let mut dot_count = 0;
        while let Some(chr) = self.current_char() {
            match chr {
                '.' => {
                    if let Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_') = self.peek() {
                        break
                    }
                    dot_count += 1;
                },
                '0'..='9' => (),
                '_' => {
                    self.advance();
                    continue;
                }
                _ => break,
            }
            number_str.push(*chr);
            self.advance();
        }
        self.retreat();
        match dot_count {
            0 => Ok(TokenType::Integer(match number_str.parse() {
                Ok(v) => v,
                Err(_) => {
                    return Err(vec![Error::new(
                        vec![(start as u32, self.index as u32, Highlight::Red)],
                        "failed to parse integer",
                        "is the number is too large".to_string(),
                        &FATAL,
                        self.current_file.clone(),
                    )])
                }
            })),
            1 => Ok(TokenType::Float(match number_str.parse() {
                Ok(v) => v,
                Err(_) => {
                    return Err(vec![Error::new(
                        vec![(start as u32, self.index as u32, Highlight::Red)],
                        "failed to parse float",
                        "is the number is too large".to_string(),
                        &FATAL,
                        self.current_file.clone(),
                    )])
                }
            })),
            _ => Err(vec![Error::new(
                vec![(start as u32, self.index as u32, Highlight::Red)],
                "failed to parse number",
                "the number has too many dots".to_string(),
                &FATAL,
                self.current_file.clone(),
            )]),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn string(&mut self) -> Result<TokenType, Vec<Error>> {
        let start = self.index;
        let mut string = String::with_capacity(8);
        let mut escape_char = false;
        let mut errors = vec![];

        // As this function is only called by the lexer we can assume
        // the first character is '"'
        self.advance();

        let mut line_increase = 0;

        while let Some(chr) = self.current_char() {
            if escape_char {
                escape_char = false;
                match chr {
                    '\\' => string.push('\\'),
                    '"' => string.push('"'),
                    'n' => string.push('\n'),
                    't' => string.push('\t'),
                    _ => errors.push(Error::new(
                        vec![(start as u32, self.index as u32, Highlight::Red)],
                        "invalid escape sequence",
                        "".to_string(),
                        &FATAL,
                        self.current_file.clone(),
                    )),
                }
            } else {
                match chr {
                    '\\' => escape_char = true,
                    '"' => break,
                    '\n' => {
                        line_increase += 1;
                        string.push(*chr);
                    }
                    _ => string.push(*chr),
                }
            }
            self.advance();
        }

        self.line += line_increase;

        if self.current_char() != Some(&'"') {
            errors.push(Error::new(
                vec![
                    (start as u32, start as u32 + 1, Highlight::Red),
                    (
                        start as u32 + string.trim_end().len() as u32 + 1,
                        start as u32 + string.trim_end().len() as u32 + 1,
                        Highlight::Red,
                    ),
                ],
                "unterminated string",
                "add a closing quotation mark at the end".to_string(),
                &FATAL,
                self.current_file.clone(),
            ));
        }

        if errors.is_empty() {
            Ok(TokenType::String(string))
        } else {
            Err(errors)
        }
    }

    fn identifier(&mut self) -> TokenType {
        let mut string = String::with_capacity(8);

        while let Some(chr) = self.current_char() {
            match chr {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => string.push(*chr),
                _ => break,
            }
            self.advance();
        }
        self.retreat();

        match string.as_str() {
            "true" => TokenType::True,
            "false" => TokenType::False,
            "var" => TokenType::Var,
            "if" => TokenType::If,
            "else" => TokenType::Else,
            "while" => TokenType::While,
            "fn" => TokenType::Fn,
            "return" => TokenType::Return,
            "struct" => TokenType::Struct,
            "impl" => TokenType::Impl,
            "raw" => TokenType::Raw,
            "inline" => TokenType::Inline,
            "using" => TokenType::Using,
            "bytecode" => TokenType::Bytecode,
            _ => TokenType::Identifier(string),
        }
    }
}
