//! Syntax foundation for the Decay gameplay language.
//!
//! This crate deliberately has no dependency on Sindri. Decay syntax should be
//! testable and extractable before any engine binding exists.

pub mod ast;
mod parser;

pub use ast::*;
pub use parser::{Parsed, parse};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        Self::new(self.start, other.end)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Script,
    Component,
    Fn,
    Let,
    Var,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Return,
    True,
    False,
    Null,
    Identifier(String),
    Number(f64),
    String(String),
    At,
    Colon,
    Semicolon,
    Comma,
    Dot,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Arrow,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn lex(source: &str) -> Lexed {
    Lexer::new(source).lex()
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex(mut self) -> Lexed {
        while self.cursor < self.source.len() {
            self.skip_trivia();
            if self.cursor >= self.source.len() {
                break;
            }

            let start = self.cursor;
            let Some(ch) = self.bump() else {
                break;
            };

            match ch {
                '@' => self.push(TokenKind::At, start),
                ':' => self.push(TokenKind::Colon, start),
                ';' => self.push(TokenKind::Semicolon, start),
                ',' => self.push(TokenKind::Comma, start),
                '.' => self.push(TokenKind::Dot, start),
                '(' => self.push(TokenKind::LeftParen, start),
                ')' => self.push(TokenKind::RightParen, start),
                '{' => self.push(TokenKind::LeftBrace, start),
                '}' => self.push(TokenKind::RightBrace, start),
                '[' => self.push(TokenKind::LeftBracket, start),
                ']' => self.push(TokenKind::RightBracket, start),
                '+' => self.push_if_equal(TokenKind::PlusEqual, TokenKind::Plus, start),
                '-' => {
                    if self.consume('>') {
                        self.push(TokenKind::Arrow, start);
                    } else if self.consume('=') {
                        self.push(TokenKind::MinusEqual, start);
                    } else {
                        self.push(TokenKind::Minus, start);
                    }
                }
                '*' => self.push_if_equal(TokenKind::StarEqual, TokenKind::Star, start),
                '/' => self.push_if_equal(TokenKind::SlashEqual, TokenKind::Slash, start),
                '%' => self.push_if_equal(TokenKind::PercentEqual, TokenKind::Percent, start),
                '=' => self.push_if_equal(TokenKind::EqualEqual, TokenKind::Equal, start),
                '!' => self.push_if_equal(TokenKind::BangEqual, TokenKind::Bang, start),
                '<' => self.push_if_equal(TokenKind::LessEqual, TokenKind::Less, start),
                '>' => self.push_if_equal(TokenKind::GreaterEqual, TokenKind::Greater, start),
                '&' if self.consume('&') => self.push(TokenKind::AndAnd, start),
                '|' if self.consume('|') => self.push(TokenKind::OrOr, start),
                '"' => self.lex_string(start),
                c if c.is_ascii_digit() => self.lex_number(start),
                c if is_identifier_start(c) => self.lex_identifier(start),
                other => self.error(
                    start,
                    self.cursor,
                    format!("unexpected character `{other}`"),
                ),
            }
        }

        let end = self.source.len();
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(end, end),
        });

        Lexed {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }

            if self.peek() == Some('/') && self.peek_next() == Some('/') {
                while self.peek().is_some_and(|ch| ch != '\n') {
                    self.bump();
                }
                continue;
            }

            break;
        }
    }

    fn lex_identifier(&mut self, start: usize) {
        while self.peek().is_some_and(is_identifier_continue) {
            self.bump();
        }

        let text = &self.source[start..self.cursor];
        let kind = match text {
            "script" => TokenKind::Script,
            "component" => TokenKind::Component,
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "var" => TokenKind::Var,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        self.push(kind, start);
    }

    fn lex_number(&mut self, start: usize) {
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }

        if self.peek() == Some('.') && self.peek_next().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.bump();
            }
        }

        let text = &self.source[start..self.cursor];
        match text.parse::<f64>() {
            Ok(value) => self.push(TokenKind::Number(value), start),
            Err(_) => self.error(start, self.cursor, format!("invalid number `{text}`")),
        }
    }

    fn lex_string(&mut self, start: usize) {
        let mut value = String::new();
        let mut terminated = false;

        while let Some(ch) = self.bump() {
            match ch {
                '"' => {
                    terminated = true;
                    break;
                }
                '\\' => match self.bump() {
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    Some(other) => {
                        value.push(other);
                        self.error(
                            self.cursor.saturating_sub(other.len_utf8() + 1),
                            self.cursor,
                            format!("unknown escape sequence `\\{other}`"),
                        );
                    }
                    None => break,
                },
                other => value.push(other),
            }
        }

        if terminated {
            self.push(TokenKind::String(value), start);
        } else {
            self.error(start, self.cursor, "unterminated string literal".to_owned());
        }
    }

    fn push_if_equal(&mut self, with_equal: TokenKind, plain: TokenKind, start: usize) {
        if self.consume('=') {
            self.push(with_equal, start);
        } else {
            self.push(plain, start);
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.cursor),
        });
    }

    fn error(&mut self, start: usize, end: usize, message: String) {
        let (line, column) = line_column(self.source, start);
        self.diagnostics.push(Diagnostic {
            message,
            span: Span::new(start, end),
            line,
            column,
        });
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.cursor..].chars();
        chars.next()?;
        chars.next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.cursor += ch.len_utf8();
        Some(ch)
    }
}

const fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

const fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

pub(crate) fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len() + 1, |(_, tail)| tail.len() + 1);
    (line, column)
}
