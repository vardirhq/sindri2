//! Containers, and the members they declare.
//!
//! A new kind of member is a branch in `parse_container` and a function
//! beside the others here.

use crate::{
    TokenKind,
    ast::{Attribute, ContainerDecl, FieldDecl, FunctionDecl, Item, Member, Param, TypeRef},
};

use super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_container(&mut self, script: bool) -> Option<Item> {
        let start = self.current().span;
        self.advance();
        let (name, _) = self.expect_identifier("expected a name after declaration keyword")?;
        self.expect_simple(&TokenKind::LeftBrace, "expected `{` after declaration name")?;

        let mut members = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            let attributes = self.parse_attributes();
            if self.at(&TokenKind::Fn) {
                if !attributes.is_empty() {
                    self.error_span(
                        attributes[0].span,
                        "attributes on functions are not supported yet",
                    );
                }
                if let Some(function) = self.parse_function() {
                    members.push(Member::Function(function));
                } else {
                    self.synchronize_member();
                }
            } else if self.at(&TokenKind::Let) || self.at(&TokenKind::Var) {
                if let Some(field) = self.parse_field(attributes) {
                    members.push(Member::Field(field));
                } else {
                    self.synchronize_member();
                }
            } else {
                self.error_here("expected a field or function in declaration body");
                self.synchronize_member();
            }
        }

        let end = self
            .expect_simple(
                &TokenKind::RightBrace,
                "expected `}` after declaration body",
            )
            .unwrap_or_else(|| self.current().span);
        let decl = ContainerDecl {
            name,
            members,
            span: start.join(end),
        };
        Some(if script {
            Item::Script(decl)
        } else {
            Item::Component(decl)
        })
    }

    pub(super) fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attributes = Vec::new();
        while self.at(&TokenKind::At) {
            let start = self.current().span;
            self.advance();
            if let Some((name, name_span)) =
                self.expect_identifier("expected attribute name after `@`")
            {
                attributes.push(Attribute {
                    name,
                    span: start.join(name_span),
                });
            } else {
                break;
            }
        }
        attributes
    }

    pub(super) fn parse_field(&mut self, attributes: Vec<Attribute>) -> Option<FieldDecl> {
        let start = self.current().span;
        let mutable = self.at(&TokenKind::Var);
        self.advance();
        let (name, _) = self.expect_identifier("expected field name")?;
        let ty = self.parse_optional_type();
        let initializer = if self.consume_simple(&TokenKind::Equal).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let end = self.expect_simple(&TokenKind::Semicolon, "expected `;` after field")?;
        Some(FieldDecl {
            attributes,
            mutable,
            name,
            ty,
            initializer,
            span: start.join(end),
        })
    }

    pub(super) fn parse_function(&mut self) -> Option<FunctionDecl> {
        let start = self.expect_simple(&TokenKind::Fn, "expected `fn`")?;
        let (name, _) = self.expect_identifier("expected function name")?;
        self.expect_simple(&TokenKind::LeftParen, "expected `(` after function name")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                let (name, name_span) = self.expect_identifier("expected parameter name")?;
                let ty = self.parse_optional_type();
                let end = ty.as_ref().map_or(name_span, |ty| ty.span);
                params.push(Param {
                    name,
                    ty,
                    span: name_span.join(end),
                });
                if self.consume_simple(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect_simple(&TokenKind::RightParen, "expected `)` after parameters")?;
        let return_type = if self.consume_simple(&TokenKind::Arrow).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = start.join(body.span);
        Some(FunctionDecl {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    pub(super) fn parse_optional_type(&mut self) -> Option<TypeRef> {
        if self.consume_simple(&TokenKind::Colon).is_some() {
            self.parse_type()
        } else {
            None
        }
    }

    pub(super) fn parse_type(&mut self) -> Option<TypeRef> {
        self.expect_identifier("expected type name")
            .map(|(name, span)| TypeRef { name, span })
    }
}
