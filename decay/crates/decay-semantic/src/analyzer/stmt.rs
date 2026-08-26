//! One function per statement form.
//!
//! A new statement is an arm in `analyze_stmt` and a function beside
//! the others here.

use std::collections::HashMap;

use decay_syntax::{Block, Span, Stmt};

use crate::types::Type;

use super::{Analyzer, Symbol};

impl Analyzer<'_, '_> {
    pub(super) fn analyze_block(&mut self, block: &Block, create_scope: bool) {
        if create_scope {
            self.scopes.push(HashMap::new());
        }

        for statement in &block.statements {
            self.analyze_stmt(statement);
        }

        if create_scope {
            self.scopes.pop();
        }
    }

    pub(super) fn analyze_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Binding {
                mutable,
                name,
                ty,
                initializer,
                span,
            } => {
                let initializer_type = initializer
                    .as_ref()
                    .map_or(Type::Unknown, |expr| self.expr_type(expr));
                let declared_type = ty.as_ref().map(Type::from_ref);

                if declared_type.is_none() && initializer.is_none() {
                    self.error(
                        *span,
                        format!("binding `{name}` needs a type or initializer"),
                    );
                }

                if let Some(expected) = &declared_type {
                    self.check_assignable(expected, &initializer_type, *span);
                }

                self.define_local(
                    name,
                    Symbol {
                        ty: declared_type.unwrap_or(initializer_type),
                        mutable: *mutable,
                        function: None,
                    },
                    *span,
                );
            }
            Stmt::Expr { expr, .. } => {
                self.expr_type(expr);
            }
            Stmt::Return { value, span } => {
                let actual = value
                    .as_ref()
                    .map_or(Type::Unit, |expr| self.expr_type(expr));
                let expected = self.current_return.clone();
                self.check_assignable(&expected, &actual, *span);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_type = self.expr_type(condition);
                if !matches!(condition_type, Type::Bool | Type::Unknown) {
                    self.error(
                        condition.span,
                        format!(
                            "if condition must be `bool`, found `{}`",
                            condition_type.display_name()
                        ),
                    );
                }
                self.analyze_block(then_branch, true);
                if let Some(else_branch) = else_branch {
                    self.analyze_block(else_branch, true);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let condition_type = self.expr_type(condition);
                if !matches!(condition_type, Type::Bool | Type::Unknown) {
                    self.error(
                        condition.span,
                        format!(
                            "while condition must be `bool`, found `{}`",
                            condition_type.display_name()
                        ),
                    );
                }
                self.loop_depth += 1;
                self.analyze_block(body, true);
                self.loop_depth -= 1;
            }
            Stmt::Break { span } => {
                if self.loop_depth == 0 {
                    self.error(*span, "`break` outside of a loop".to_owned());
                }
            }
            Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    self.error(*span, "`continue` outside of a loop".to_owned());
                }
            }
            Stmt::Block(block) => self.analyze_block(block, true),
        }
    }

    pub(super) fn define_local(&mut self, name: &str, symbol: Symbol, span: Span) {
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        if scope.insert(name.to_owned(), symbol).is_some() {
            self.error(span, format!("duplicate local `{name}`"));
        }
    }
}
