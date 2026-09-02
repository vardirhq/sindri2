//! One function per expression form, and the types they produce.

use decay_syntax::{AssignOp, BinaryOp, Expr, ExprKind, UnaryOp};

use crate::types::Type;

use super::Analyzer;

impl Analyzer<'_, '_> {
    pub(super) fn expr_type(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Identifier(name) => self.resolve_identifier(name, expr.span),
            ExprKind::Number(_) => Type::F32,
            ExprKind::String(_) => Type::String,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Null => Type::Null,
            ExprKind::Group(inner) => self.expr_type(inner),
            ExprKind::Unary { op, expr: inner } => {
                let inner_type = self.expr_type(inner);
                match op {
                    UnaryOp::Negate => {
                        self.require_type(&inner_type, &Type::F32, inner.span);
                        Type::F32
                    }
                    UnaryOp::Not => {
                        self.require_type(&inner_type, &Type::Bool, inner.span);
                        Type::Bool
                    }
                }
            }
            ExprKind::Binary { left, op, right } => self.binary_type(left, *op, right),
            ExprKind::Assign { target, op, value } => self.assignment_type(target, *op, value),
            ExprKind::Member { object, field } => self.member_type(object, field, expr.span),
            ExprKind::Call { callee, args } => self.call_type(callee, args, expr.span),
            ExprKind::Index { object, index } => self.index_type(object, index),
        }
    }

    /// The type of `items[index]`.
    ///
    /// Indexing something that is not a collection is an error naming what it
    /// actually is, rather than a runtime failure with a number in it. The
    /// index is the language's one numeric type: there is no integer type, and
    /// `docs/decay-direction.md` records why introducing one for this alone
    /// would be the wrong trade. A fractional or out-of-range index is refused
    /// when it runs, where the value is known.
    pub(super) fn index_type(&mut self, object: &Expr, index: &Expr) -> Type {
        let object_type = self.expr_type(object);
        let index_type = self.expr_type(index);
        self.require_type(&index_type, &Type::F32, index.span);
        match &object_type {
            Type::Array(element) => (**element).clone(),
            Type::Unknown => Type::Unknown,
            other => {
                self.error(
                    object.span,
                    format!("`{}` cannot be indexed", other.display_name()),
                );
                Type::Unknown
            }
        }
    }

    pub(super) fn binary_type(&mut self, left: &Expr, op: BinaryOp, right: &Expr) -> Type {
        let left_type = self.expr_type(left);
        let right_type = self.expr_type(right);

        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulo => {
                self.require_type(&left_type, &Type::F32, left.span);
                self.require_type(&right_type, &Type::F32, right.span);
                Type::F32
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.require_type(&left_type, &Type::F32, left.span);
                self.require_type(&right_type, &Type::F32, right.span);
                Type::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                self.require_type(&left_type, &Type::Bool, left.span);
                self.require_type(&right_type, &Type::Bool, right.span);
                Type::Bool
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if !Self::compatible(&left_type, &right_type) {
                    self.error(
                        right.span,
                        format!(
                            "cannot compare `{}` with `{}`",
                            left_type.display_name(),
                            right_type.display_name()
                        ),
                    );
                }
                Type::Bool
            }
        }
    }

    pub(super) fn assignment_type(&mut self, target: &Expr, op: AssignOp, value: &Expr) -> Type {
        let target_type = self.assignment_target_type(target);
        let value_type = self.expr_type(value);

        if matches!(op, AssignOp::Assign) {
            self.check_assignable(&target_type, &value_type, value.span);
        } else {
            self.require_type(&target_type, &Type::F32, target.span);
            self.require_type(&value_type, &Type::F32, value.span);
        }

        target_type
    }

    pub(super) fn assignment_target_type(&mut self, target: &Expr) -> Type {
        match &target.kind {
            ExprKind::Identifier(name) => {
                if let Some(symbol) = self.lookup(name).cloned() {
                    if symbol.function.is_some() {
                        self.error(target.span, format!("cannot assign to function `{name}`"));
                    } else if !symbol.mutable {
                        self.error(target.span, format!("cannot assign to immutable `{name}`"));
                    }
                    symbol.ty
                } else {
                    self.error(target.span, format!("unknown name `{name}`"));
                    Type::Unknown
                }
            }
            ExprKind::Member { object, field } => self.member_type(object, field, target.span),
            _ => {
                self.error(target.span, "invalid assignment target".to_owned());
                Type::Unknown
            }
        }
    }
}
