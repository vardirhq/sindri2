//! One function per statement form.
//!
//! A new statement is a branch in `parse_statement` and a function
//! beside the others here.

use crate::{
    TokenKind,
    ast::{Block, Stmt},
};

use super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_block(&mut self) -> Option<Block> {
        let start = self.expect_simple(&TokenKind::LeftBrace, "expected `{`")?;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            } else {
                self.synchronize_statement();
            }
        }
        let end = self.expect_simple(&TokenKind::RightBrace, "expected `}` after block")?;
        Some(Block {
            statements,
            span: start.join(end),
        })
    }

    pub(super) fn parse_statement(&mut self) -> Option<Stmt> {
        if self.at(&TokenKind::Let) || self.at(&TokenKind::Var) {
            return self.parse_binding_statement();
        }
        if self.at(&TokenKind::Return) {
            return self.parse_return_statement();
        }
        if self.at(&TokenKind::If) {
            return self.parse_if_statement();
        }
        if self.at(&TokenKind::LeftBrace) {
            return self.parse_block().map(Stmt::Block);
        }

        let expr = self.parse_expression()?;
        let end = self.expect_simple(&TokenKind::Semicolon, "expected `;` after expression")?;
        let span = expr.span.join(end);
        Some(Stmt::Expr { expr, span })
    }

    pub(super) fn parse_binding_statement(&mut self) -> Option<Stmt> {
        let start = self.current().span;
        let mutable = self.at(&TokenKind::Var);
        self.advance();
        let (name, _) = self.expect_identifier("expected variable name")?;
        let ty = self.parse_optional_type();
        let initializer = if self.consume_simple(&TokenKind::Equal).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let end = self.expect_simple(&TokenKind::Semicolon, "expected `;` after binding")?;
        Some(Stmt::Binding {
            mutable,
            name,
            ty,
            initializer,
            span: start.join(end),
        })
    }

    pub(super) fn parse_return_statement(&mut self) -> Option<Stmt> {
        let start = self.expect_simple(&TokenKind::Return, "expected `return`")?;
        let value = if self.at(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        let end = self.expect_simple(&TokenKind::Semicolon, "expected `;` after return")?;
        Some(Stmt::Return {
            value,
            span: start.join(end),
        })
    }

    pub(super) fn parse_if_statement(&mut self) -> Option<Stmt> {
        let start = self.expect_simple(&TokenKind::If, "expected `if`")?;
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let mut end = then_branch.span;
        let else_branch = if self.consume_simple(&TokenKind::Else).is_some() {
            let block = self.parse_block()?;
            end = block.span;
            Some(block)
        } else {
            None
        };
        Some(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: start.join(end),
        })
    }
}
