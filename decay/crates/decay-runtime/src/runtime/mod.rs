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

/// What a value is, for an error that has to say which one was wrong.
///
/// Its kind rather than its contents: an error naming "a number" is what an
/// author needs, and printing the number would put a script's data in a
/// diagnostic for no gain.
fn describe(value: &Value) -> String {
    match value {
        Value::Number(_) => "a number",
        Value::String(_) => "text",
        Value::Bool(_) => "a truth",
        Value::Reference(_) => "an entity",
        Value::Array(_) => "a collection",
        Value::Null => "null",
        Value::Unit => "nothing",
    }
    .to_owned()
}

/// How deep Decay calls may nest before [`RuntimeError::CallDepthExceeded`].
///
/// Far past anything gameplay needs, and far short of what overflows the host's
/// stack — the gap between those two is wide enough that the number does not
/// have to be exact.
pub const DEFAULT_CALL_DEPTH_LIMIT: usize = 64;

/// How many instructions one call may execute before
/// [`RuntimeError::OperationBudgetExceeded`].
///
/// Chosen the same way as the depth limit: far past what a frame of gameplay
/// needs, and far short of a wait anyone would sit through. A script doing real
/// work in an `update` runs hundreds of instructions, not a million; a script
/// that reaches this has stopped making progress, and the point is that the
/// editor says so instead of stopping with it.
pub const DEFAULT_OPERATION_BUDGET: usize = 1_000_000;

pub struct Runtime<'a, H: Host> {
    program: &'a IrProgram,
    host: H,
    call_depth_limit: usize,
    depth: usize,
    operation_budget: usize,
    /// Instructions executed since the outermost call began. Reset there rather
    /// than per frame, so that a script cannot buy itself more by recursing.
    operations: usize,
}

pub(super) struct Frame {
    /// Innermost scope last. The base scope holds the parameters and the
    /// function body's own bindings, which is the arrangement the analyzer
    /// checks against: a parameter and a body binding of the same name are a
    /// duplicate, not a shadow.
    scopes: Vec<HashMap<String, Slot>>,
    stack: Vec<Value>,
    /// The collections a `for` is part way through, innermost last.
    ///
    /// Beside the value stack rather than on it, so no value a script could
    /// hold ever represents "part way through a loop" — there is no cursor to
    /// bind to a name, print, or pass to a host.
    walks: Vec<Walk>,
}

/// One `for` in progress.
struct Walk {
    over: std::rc::Rc<Vec<Value>>,
    next: usize,
}

impl Frame {
    pub(super) fn new(locals: HashMap<String, Slot>) -> Self {
        Self {
            scopes: vec![locals],
            stack: Vec::new(),
            walks: Vec::new(),
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
            operation_budget: DEFAULT_OPERATION_BUDGET,
            operations: 0,
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

    /// Sets how many instructions one call may execute.
    #[must_use]
    pub const fn with_operation_budget(mut self, budget: usize) -> Self {
        self.operation_budget = budget;
        self
    }

    #[must_use]
    pub const fn operation_budget(&self) -> usize {
        self.operation_budget
    }

    /// Starts a fresh budget, unless this is a call made from inside another.
    pub(super) const fn begin_budget(&mut self) {
        if self.depth == 0 {
            self.operations = 0;
        }
    }

    /// Charges one instruction against the budget, before it runs rather than
    /// after, so the budget bounds what executes rather than what has executed.
    fn charge(&mut self) -> Result<(), RuntimeError> {
        self.operations += 1;
        if self.operations > self.operation_budget {
            return Err(RuntimeError::OperationBudgetExceeded {
                limit: self.operation_budget,
            });
        }
        Ok(())
    }

    pub fn into_host(self) -> H {
        self.host
    }

    /// The element an index names, refusing every way an index can be wrong.
    ///
    /// There is no integer type, so "a whole number" is a runtime property of
    /// the value rather than a static property of its type. Each failure is
    /// named separately because they are different mistakes: `items[1.5]` is a
    /// calculation that should have rounded, and `items[9]` on a collection of
    /// three is a loop bound that is wrong.
    fn element_at(object: &Value, index: &Value) -> Result<Value, RuntimeError> {
        let values = object
            .elements()
            .ok_or_else(|| RuntimeError::NotACollection(describe(object)))?;
        let Value::Number(number) = index else {
            return Err(RuntimeError::IndexNotANumber(describe(index)));
        };
        if !number.is_finite() || number.fract() != 0.0 || *number < 0.0 {
            return Err(RuntimeError::IndexNotWhole(*number));
        }
        // Guarded above: finite, non-negative, and whole.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let position = *number as usize;
        values
            .get(position)
            .cloned()
            .ok_or(RuntimeError::IndexOutOfRange {
                index: position,
                length: values.len(),
            })
    }

    /// One call: the container's own function, a call through a reference, or
    /// the host's.
    fn perform_call(
        &mut self,
        container: &IrContainer,
        fields: &mut HashMap<String, Slot>,
        frame: &mut Frame,
        callee: &decay_ir::Path,
        argument_count: usize,
    ) -> Result<Value, RuntimeError> {
        let mut args = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            args.push(frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?);
        }
        args.reverse();
        if callee.0.len() == 1
            && container
                .functions
                .iter()
                .any(|function| function.name == callee.0[0])
        {
            return self.call_in_container(container, fields, &callee.0[0], args);
        }
        if let Some((subject, rest)) = Self::subject_path(fields, frame, callee)? {
            return self
                .host
                .call(Some(subject), &rest, &args)?
                .ok_or_else(|| RuntimeError::FunctionNotFound(rest.dotted()));
        }
        self.host
            .call(None, callee, &args)?
            .ok_or_else(|| RuntimeError::FunctionNotFound(callee.dotted()))
    }

    /// The instructions that work on a collection.
    ///
    /// Their own function because they are the one group that shares a failure
    /// vocabulary, and because the evaluation loop is easier to read as a list
    /// of what an instruction *is* than as a list of what each one does.
    ///
    /// Answers with a jump target when the instruction takes one, which only
    /// the exhaustion of a walk does.
    fn step_collection(
        frame: &mut Frame,
        instruction: &Instruction,
        length: usize,
    ) -> Result<Option<usize>, RuntimeError> {
        match instruction {
            Instruction::Index => {
                let index = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                let object = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                frame.stack.push(Self::element_at(&object, &index)?);
            }
            Instruction::Length => {
                let object = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                let values = object
                    .elements()
                    .ok_or_else(|| RuntimeError::NotACollection(describe(&object)))?;
                // `usize` to `f64` is exact for every length a collection can
                // reach here, and a host bounds those far below the point
                // where it would not be.
                #[allow(clippy::cast_precision_loss)]
                frame.stack.push(Value::Number(values.len() as f64));
            }
            Instruction::IterBegin => {
                let object = frame.stack.pop().ok_or(RuntimeError::StackUnderflow)?;
                let over = object
                    .elements()
                    .ok_or_else(|| RuntimeError::NotACollection(describe(&object)))?
                    .clone();
                frame.walks.push(Walk { over, next: 0 });
            }
            Instruction::IterNext(target) => {
                let walk = frame.walks.last_mut().ok_or(RuntimeError::StackUnderflow)?;
                let Some(value) = walk.over.get(walk.next).cloned() else {
                    frame.walks.pop();
                    if *target > length {
                        return Err(RuntimeError::InvalidJump(*target));
                    }
                    return Ok(Some(*target));
                };
                walk.next += 1;
                frame.stack.push(value);
            }
            Instruction::IterEnd => {
                frame.walks.pop().ok_or(RuntimeError::StackUnderflow)?;
            }
            _ => unreachable!("only the collection instructions reach here"),
        }
        Ok(None)
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
            self.charge()?;

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
                    let result =
                        self.perform_call(container, fields, frame, callee, *argument_count)?;
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
                Instruction::Index
                | Instruction::Length
                | Instruction::IterBegin
                | Instruction::IterNext(_)
                | Instruction::IterEnd => {
                    if let Some(target) =
                        Self::step_collection(frame, &instructions[ip], instructions.len())?
                    {
                        ip = target;
                        continue;
                    }
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
