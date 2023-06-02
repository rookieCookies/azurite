#![allow(unused)]
use std::fmt::Debug;

use azurite_errors::SourceRange;
use common::SymbolTable;

use crate::{lex, Token, TokenKind, Literal};


#[test]
fn empty() {
    let mut symbol_table = SymbolTable::new();
    let file = symbol_table.add(String::from("test"));

    let data = "";
    let tokens = lex(data, file, &mut symbol_table).unwrap();

    assert_eq!(tokens, vec![
        Token {
            token_kind: TokenKind::EndOfFile,
            source_range: SourceRange::new(0, 0),
        }
    ])
}


#[test]
fn tokens() {
    let mut symbol_table = SymbolTable::new();
    let file = symbol_table.add(String::from("test"));

    let data = "<>{}";
    let tokens = lex(data, file, &mut symbol_table).unwrap();

    compare_individually(&tokens, &vec![
        Token {
            token_kind: TokenKind::LeftAngle,
            source_range: SourceRange::new(0, 0),
        },
        Token {
            token_kind: TokenKind::RightAngle,
            source_range: SourceRange::new(1, 1),
        },
        Token {
            token_kind: TokenKind::LeftBracket,
            source_range: SourceRange::new(2, 2),
        },
        Token {
            token_kind: TokenKind::RightBracket,
            source_range: SourceRange::new(3, 3),
        },
        Token {
            token_kind: TokenKind::EndOfFile,
            source_range: SourceRange::new(3, 3),
        }
    ])
}


#[test]
fn numbers() {
    let mut symbol_table = SymbolTable::new();
    let file = symbol_table.add(String::from("test"));

    let data = "123456789";
    let tokens = lex(data, file, &mut symbol_table).unwrap();

    compare_individually(&tokens, &vec![
        Token {
            token_kind: TokenKind::Literal(Literal::Integer(123456789)),
            source_range: SourceRange::new(0, 8),
        },
        Token {
            token_kind: TokenKind::EndOfFile,
            source_range: SourceRange::new(8, 8),
        }
    ]);
}


#[test]
fn identifiers() {
    let mut symbol_table = SymbolTable::new();
    let file = symbol_table.add(String::from("test"));

    let data = "hello there";
    let tokens = lex(data, file, &mut symbol_table).unwrap();

    compare_individually(&tokens, &vec![
        Token {
            token_kind: TokenKind::Identifier(symbol_table.add(String::from("hello"))),
            source_range: SourceRange::new(0, 4),
        },
        Token {
            token_kind: TokenKind::Identifier(symbol_table.add(String::from("there"))),
            source_range: SourceRange::new(6, 10),
        },
        Token {
            token_kind: TokenKind::EndOfFile,
            source_range: SourceRange::new(10, 10),
        },
    ])
}


#[test]
fn string() {
    let mut symbol_table = SymbolTable::new();
    let file = symbol_table.add(String::from("test"));

    let data = "\"hello there\"";
    let tokens = lex(data, file, &mut symbol_table).unwrap();

    compare_individually(&tokens, &vec![
        Token {
            token_kind: TokenKind::Literal(Literal::String(symbol_table.add(String::from("hello there")))),
            source_range: SourceRange::new(0, 12),
        },
        Token {
            token_kind: TokenKind::EndOfFile,
            source_range: SourceRange::new(12, 12),
        },
    ])
}


fn compare_individually<T: PartialEq + Debug>(list1: &Vec<T>, list2: &Vec<T>) {
    assert_eq!(list1.len(), list2.len());
    for (index, (v1, v2)) in list1.iter().zip(list2.iter()).enumerate() {
        assert_eq!(v1, v2, "{index}");
    }
}