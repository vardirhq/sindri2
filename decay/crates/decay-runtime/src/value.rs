//! What a Decay expression evaluates to, and the arithmetic over it.

use decay_ir::Constant;
use decay_syntax::{BinaryOp, UnaryOp};

use crate::error::RuntimeError;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    /// Something the host owns, which a script may hold, compare, pass and
    /// store, but cannot construct, read into, or do arithmetic on.
    ///
    /// The number inside is the host's, and Decay attaches no meaning to it.
    /// That is the whole design: a script needs to be able to *name* another
    /// thing in the world in order to say anything about it, and it can do
    /// that without the language knowing what a world is. The engine packs an
    /// entity's slot and generation into it; a different host could pack
    /// something else, and nothing here would change.
    ///
    /// An absent reference is [`Value::Null`], not a reserved number, for the
    /// same reason an empty tile is null: every number is a real reference.
    Reference(u64),
    Null,
    Unit,
}

impl From<&Constant> for Value {
    fn from(value: &Constant) -> Self {
        match value {
            Constant::Number(value) => Self::Number(*value),
            Constant::String(value) => Self::String(value.clone()),
            Constant::Bool(value) => Self::Bool(*value),
            Constant::Null => Self::Null,
        }
    }
}

pub(crate) fn apply_unary(op: UnaryOp, value: Value) -> Result<Value, RuntimeError> {
    match (op, value) {
        (UnaryOp::Negate, Value::Number(value)) => Ok(Value::Number(-value)),
        (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
        _ => Err(RuntimeError::InvalidUnary),
    }
}

pub(crate) fn apply_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    match op {
        BinaryOp::Add => numbers(left, right, |a, b| a + b),
        BinaryOp::Subtract => numbers(left, right, |a, b| a - b),
        BinaryOp::Multiply => numbers(left, right, |a, b| a * b),
        BinaryOp::Divide => numbers(left, right, |a, b| a / b),
        BinaryOp::Less => compare(left, right, |a, b| a < b),
        BinaryOp::LessEqual => compare(left, right, |a, b| a <= b),
        BinaryOp::Greater => compare(left, right, |a, b| a > b),
        BinaryOp::GreaterEqual => compare(left, right, |a, b| a >= b),
        // Lowering does not emit these any more: `&&` and `||` became branches
        // so that a left operand which already decides the answer skips the
        // right one. They stay because `Instruction` is public and an IR built
        // by hand may still ask for the operation over two values it has
        // already evaluated, which is the one thing this can still mean.
        BinaryOp::And => booleans(left, right, |a, b| a && b),
        BinaryOp::Or => booleans(left, right, |a, b| a || b),
        BinaryOp::Equal => Ok(Value::Bool(left == right)),
        BinaryOp::NotEqual => Ok(Value::Bool(left != right)),
    }
}

pub(crate) fn numbers(
    left: Value,
    right: Value,
    op: impl FnOnce(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(op(a, b))),
        _ => Err(RuntimeError::InvalidBinary),
    }
}

pub(crate) fn compare(
    left: Value,
    right: Value,
    op: impl FnOnce(f64, f64) -> bool,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err(RuntimeError::InvalidBinary),
    }
}

pub(crate) fn booleans(
    left: Value,
    right: Value,
    op: impl FnOnce(bool, bool) -> bool,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err(RuntimeError::InvalidBinary),
    }
}
