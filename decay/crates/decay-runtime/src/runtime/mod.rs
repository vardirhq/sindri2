//! The interpreter itself.
//!
//! `execute_instructions` is the whole evaluation loop: one arm per IR
//! instruction. A new instruction is an arm there and, if it reaches the
//! host or an instance, a function in `path`.

mod call;
mod path;

use std::collections::HashMap;

use decay_ir::{Instruction, IrContainer, IrProgram};

use crate::error::RuntimeError;
use crate::host::Host;
use crate::instance::Slot;
use crate::value::{Value, apply_binary, apply_unary};

/// How deep Decay calls may nest before [`RuntimeError::CallDepthExceeded`].
///
/// Far past anything gameplay needs, and far short of what overflows the host's
/// stack — the gap between those two is wide enough that the number does not
/// have to be exact.
pub const DEFAULT_CALL_DEPTH_LIMIT: usize = 64;

pub struct Runtime<'a, H: Host> {
    program: &'a IrProgram,
    host: H,
    call_depth_limit: usize,
    depth: usize,
}

pub(super) struct Frame {
    /// Innermost scope last. The base scope holds the parameters and the
    /// function body's own bindings, which is the arrangement the analyzer
    /// checks against: a parameter and a body binding of the same name are a
    /// duplicate, not a shadow.
    scopes: Vec<HashMap<String, Slot>>,
    stack: Vec<Value>,
}

impl Frame {
    pub(super) fn new(locals: HashMap<String, Slot>) -> Self {
        Self {
            scopes: vec![locals],
            stack: Vec::new(),
        }
    }

    pub(super) fn declare(&mut self, name: &str, slot: Slot) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), slot);
        }
    }

    pub(super) fn lookup(&self, name: &str) -> Option<&Slot> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub(super) fn lookup_mut(&mut self, name: &str) -> Option<&mut Slot> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }
}

impl<'a, H: Host> Runtime<'a, H> {
    #[must_use]
    pub fn new(program: &'a IrProgram, host: H) -> Self {
        Self {
            program,
            host,
            call_depth_limit: DEFAULT_CALL_DEPTH_LIMIT,
            depth: 0,
        }
    }

    /// Sets how deep Decay calls may nest.
    #[must_use]
    pub const fn with_call_depth_limit(mut self, limit: usize) -> Self {
        self.call_depth_limit = limit;
        self
    }

    #[must_use]
    pub const fn call_depth_limit(&self) -> usize {
        self.call_depth_limit
    }

    pub fn into_host(self) -> H {
        self.host
    }

    pub(super) fn execute_instructions(
        &mut self,
        container: &IrContainer,
        fields: &mut HashMap<String, Slot>,
        frame: &mut Frame,
        instructions: &[Instruction],
    ) -> Result<Value, RuntimeError> {
        let mut ip = 0usize;
        while ip < instructions.len() {
            match &instructions[ip] {
                Instruction::Push(constant) => frame.stack.push(Value::from(constant)),
                Instruction::Load(path) => {
                    let value = self.load_path(fields, frame, path)?;
                    frame.stack.push(value);
                }
                Instruction::Store(path) => {
                    let value = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                    self.store_path(fields, frame, path, value.clone())?;
                    frame.stack.push(value);
                }
                Instruction::Declare { name, mutable } => {
                    let value = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                    frame.declare(
                        name,
                        Slot {
                            value,
                            mutable: *mutable,
                        },
                    );
                }
                Instruction::Unary(op) => {
                    let value = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                    frame.stack.push(apply_unary(*op, value)?);
                }
                Instruction::Binary(op) => {
                    let right = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                    let left = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                    frame.stack.push(apply_binary(*op, left, right)?);
                }
                Instruction::Call {
                    callee,
                    argument_count,
                } => {
                    let mut args = Vec::with_capacity(*argument_count);
                    for _ in 0..*argument_count {
                        args.push(frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?);
                    }
                    args.reverse();
                    let result = if callee.0.len() == 1
                        && container
                            .functions
                            .iter()
                            .any(|function| function.name == callee.0[0])
                    {
                        self.call_in_container(container, fields, &callee.0[0], args)?
                    } else if let Some((subject, rest)) = Self::subject_path(fields, frame, callee)?
                    {
                        self.host
                            .call(Some(subject), &rest, &args)?
                            .ok_or_else(|| RuntimeError::FunctionNotFound(rest.dotted()))?
                    } else if let Some(value) = self.host.call(None, callee, &args)? {
                        value
                    } else {
                        return Err(RuntimeError::FunctionNotFound(callee.dotted()));
                    };
                    frame.stack.push(result);
                }
                Instruction::Pop => {
                    frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                }
                Instruction::Return => return Ok(frame.stack.pop().unwrap_or(Value::Unit)),
                Instruction::JumpIfFalse(target) => {
                    let condition = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                    match condition {
                        Value::Bool(false) => {
                            if *target > instructions.len() {
                                return Err(RuntimeError::InvalidJump(*target));
                            }
                            ip = *target;
                            continue;
                        }
                        Value::Bool(true) => {}
                        _ => return Err(RuntimeError::ExpectedBool),
                    }
                }
                Instruction::Jump(target) => {
                    if *target > instructions.len() {
                        return Err(RuntimeError::InvalidJump(*target));
                    }
                    ip = *target;
                    continue;
                }
                Instruction::ScopeEnter => frame.scopes.push(HashMap::new()),
                Instruction::ScopeExit => {
                    // The base scope holds the parameters, so it is never the
                    // one being closed.
                    if frame.scopes.len() > 1 {
                        frame.scopes.pop();
                    }
                }
            }
            ip += 1;
        }
        Ok(frame.stack.pop().unwrap_or(Value::Unit))
    }
}
