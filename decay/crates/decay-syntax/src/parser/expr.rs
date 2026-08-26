//! Expressions, loosest binding first.
//!
//! Each function parses one precedence level and calls the next one
//! down, so a new operator is a level added in the chain rather than a
//! table anyone has to keep in step with the grammar.

use crate::{
    TokenKind,
    ast::{AssignOp, BinaryOp, Expr, ExprKind, UnaryOp},
};

use super::Parser;

pub(super) fn binary_operator(kind: &TokenKind) -> Option<(u8, BinaryOp)> {
    match kind {
        TokenKind::OrOr => Some((1, BinaryOp::Or)),
        TokenKind::AndAnd => Some((2, BinaryOp::And)),
        TokenKind::EqualEqual => Some((3, BinaryOp::Equal)),
        TokenKind::BangEqual => Some((3, BinaryOp::NotEqual)),
        TokenKind::Less => Some((4, BinaryOp::Less)),
        TokenKind::LessEqual => Some((4, BinaryOp::LessEqual)),
        TokenKind::Greater => Some((4, BinaryOp::Greater)),
        TokenKind::GreaterEqual => Some((4, BinaryOp::GreaterEqual)),
        TokenKind::Plus => Some((5, BinaryOp::Add)),
        TokenKind::Minus => Some((5, BinaryOp::Subtract)),
        TokenKind::Star => Some((6, BinaryOp::Multiply)),
        TokenKind::Slash => Some((6, BinaryOp::Divide)),
        TokenKind::Percent => Some((6, BinaryOp::Modulo)),
        _ => None,
    }
}

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> Option<Expr> {
        self.parse_assignment()
    }

    pub(super) fn parse_assignment(&mut self) -> Option<Expr> {
        let target = self.parse_binary(0)?;
        let op = match self.current().kind {
            TokenKind::Equal => AssignOp::Assign,
            TokenKind::PlusEqual => AssignOp::Add,
            TokenKind::MinusEqual => AssignOp::Subtract,
            TokenKind::StarEqual => AssignOp::Multiply,
            TokenKind::SlashEqual => AssignOp::Divide,
            TokenKind::PercentEqual => AssignOp::Modulo,
            _ => return Some(target),
        };
        self.advance();
        let value = self.parse_assignment()?;
        let span = target.span.join(value.span);
        Some(Expr {
            kind: ExprKind::Assign {
                target: Box::new(target),
                op,
                value: Box::new(value),
            },
            span,
        })
    }

    pub(super) fn parse_binary(&mut self, min_precedence: u8) -> Option<Expr> {
        let mut left = self.parse_unary()?;
        while let Some((precedence, op)) = binary_operator(&self.current().kind) {
            if precedence < min_precedence {
                break;
            }
            self.advance();
            let right = self.parse_binary(precedence + 1)?;
            let span = left.span.join(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            };
        }
        Some(left)
    }

    pub(super) fn parse_unary(&mut self) -> Option<Expr> {
        let op = match self.current().kind {
            TokenKind::Minus => Some(UnaryOp::Negate),
            TokenKind::Bang => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            let start = self.current().span;
            self.advance();
            let expr = self.parse_unary()?;
            let span = start.join(expr.span);
            return Some(Expr {
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
                span,
            });
        }
        self.parse_postfix()
    }

    pub(super) fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.consume_simple(&TokenKind::Dot).is_some() {
                let (field, field_span) =
                    self.expect_identifier("expected member name after `.`")?;
                let span = expr.span.join(field_span);
                expr = Expr {
                    kind: ExprKind::Member {
                        object: Box::new(expr),
                        field,
                    },
                    span,
                };
                continue;
            }
            if self.consume_simple(&TokenKind::LeftParen).is_some() {
                let mut args = Vec::new();
                if !self.at(&TokenKind::RightParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        if self.consume_simple(&TokenKind::Comma).is_none() {
                            break;
                        }
                    }
                }
                let end =
                    self.expect_simple(&TokenKind::RightParen, "expected `)` after arguments")?;
                let span = expr.span.join(end);
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                };
                continue;
            }
            break;
        }
        Some(expr)
    }

    pub(super) fn parse_primary(&mut self) -> Option<Expr> {
        let token = self.current().clone();
        let kind = match token.kind {
            TokenKind::Identifier(name) => {
                self.advance();
                ExprKind::Identifier(name)
            }
            TokenKind::Number(value) => {
                self.advance();
                ExprKind::Number(value)
            }
            TokenKind::String(value) => {
                self.advance();
                ExprKind::String(value)
            }
            TokenKind::True => {
                self.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Null => {
                self.advance();
                ExprKind::Null
            }
            TokenKind::LeftParen => {
                self.advance();
                let inner = self.parse_expression()?;
                let end =
                    self.expect_simple(&TokenKind::RightParen, "expected `)` after expression")?;
                return Some(Expr {
                    span: token.span.join(end),
                    kind: ExprKind::Group(Box::new(inner)),
                });
            }
            _ => {
                self.error_here("expected expression");
                return None;
            }
        };
        Some(Expr {
            kind,
            span: token.span,
        })
    }
}
