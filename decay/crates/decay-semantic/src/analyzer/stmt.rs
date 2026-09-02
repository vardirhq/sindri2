//! One function per statement form.
//!
//! A new statement is an arm in `analyze_stmt` and a function beside
//! the others here.

use std::collections::HashMap;

use decay_syntax::{Block, Expr, Span, Stmt};

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
                let declared_type = ty.as_ref().map(|ty| self.resolve_type(ty));

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
            Stmt::For {
                name,
                name_span,
                iterable,
                body,
                ..
            } => self.analyze_for(name, *name_span, iterable, body),
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

    /// `for name in items { ... }`.
    fn analyze_for(&mut self, name: &str, name_span: Span, iterable: &Expr, body: &Block) {
        let iterable_type = self.expr_type(iterable);
        let element = match &iterable_type {
            Type::Array(element) => (**element).clone(),
            Type::Unknown => Type::Unknown,
            other => {
                self.error(
                    iterable.span,
                    format!(
                        "`for` needs something to walk, and `{}` holds one value",
                        other.display_name()
                    ),
                );
                Type::Unknown
            }
        };
        // The binding lives in a scope of its own, so the body may
        // declare a local of the same name in a nested block without
        // colliding with it, and the name is gone after the loop.
        self.scopes.push(HashMap::new());
        self.define_local(
            name,
            Symbol {
                ty: element,
                // Immutable: an element is what the collection holds at
                // that position, not a place to put something. There is
                // no way to write back through one, so a `var` here
                // would promise something the language cannot do.
                mutable: false,
                function: None,
            },
            name_span,
        );
        self.loop_depth += 1;
        self.analyze_block(body, true);
        self.loop_depth -= 1;
        self.scopes.pop();
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
