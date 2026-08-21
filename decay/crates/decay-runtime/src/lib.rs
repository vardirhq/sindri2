//! Small interpreter for Decay IR.
//!
//! The runtime deliberately knows nothing about Sindri. Host integrations
//! provide path loads/stores and callable functions through [`Host`].

use std::collections::HashMap;

use decay_ir::{Constant, Instruction, IrContainer, IrFunction, IrProgram, Path};
use decay_syntax::{BinaryOp, UnaryOp};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// A path was rooted at something the script holds that is not a reference,
    /// so there is nothing for the rest of the path to be about.
    NotAReference(String),
    /// A path was rooted at a reference that is empty. Reaching through nothing
    /// is a mistake worth naming, rather than silently doing nothing.
    NullReference(String),
    ContainerNotFound(String),
    FunctionNotFound(String),
    Arity {
        function: String,
        expected: usize,
        found: usize,
    },
    UnknownPath(String),
    Immutable(String),
    StackUnderflow,
    InvalidUnary,
    InvalidBinary,
    ExpectedBool,
    InvalidJump(usize),
    /// A script called deeper than [`Runtime::call_depth_limit`] allows.
    ///
    /// Without it, unbounded recursion overflowed the host's own stack and
    /// aborted the process — which, for a runtime meant to execute author
    /// scripts inside the editor, takes the editor and any unsaved work with
    /// it. A limit turns that into a value a caller can report.
    CallDepthExceeded {
        function: String,
        limit: usize,
    },
    Host(String),
}

/// Everything outside the language, across three methods.
///
/// Each takes a `subject`: `None` for a path the script wrote from a root the
/// host owns (`this.transform.position.x`, `Input.axis(...)`), and `Some(id)`
/// for one rooted at a value a script is holding — `target.transform.position.x`
/// where `target` came from the host earlier. The path passed alongside a
/// subject is the part *after* the root, so a host answers
/// `transform.position.x` for whichever thing the subject names.
///
/// Three methods and not six because the subject is an argument rather than a
/// mode: a host that ignores it simply refuses every subject, which is what
/// [`EmptyHost`] does and what a host without references should do.
pub trait Host {
    fn load(&mut self, subject: Option<u64>, path: &Path) -> Result<Option<Value>, RuntimeError>;
    fn store(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        value: Value,
    ) -> Result<bool, RuntimeError>;
    fn call(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError>;
}

#[derive(Debug, Default)]
pub struct EmptyHost;

impl Host for EmptyHost {
    fn load(&mut self, _subject: Option<u64>, _path: &Path) -> Result<Option<Value>, RuntimeError> {
        Ok(None)
    }
    fn store(
        &mut self,
        _subject: Option<u64>,
        _path: &Path,
        _value: Value,
    ) -> Result<bool, RuntimeError> {
        Ok(false)
    }
    fn call(
        &mut self,
        _subject: Option<u64>,
        _path: &Path,
        _args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
struct Slot {
    value: Value,
    mutable: bool,
}

#[derive(Debug, Clone)]
pub struct ScriptInstance {
    container_name: String,
    fields: HashMap<String, Slot>,
}

impl ScriptInstance {
    #[must_use]
    pub fn container_name(&self) -> &str {
        &self.container_name
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name).map(|slot| &slot.value)
    }

    /// Sets a field from outside the script, as an authoring tool does.
    ///
    /// This deliberately ignores the field's mutability. `@export let speed`
    /// means *the author sets this and the script does not*, so the host
    /// writing it is the point rather than a violation — the immutability the
    /// analyzer enforces is immutability to the script's own code. Callers that
    /// want to honour `@export` should check
    /// [`decay_ir::IrField::exported`] before calling; the runtime does not
    /// know what a property panel is.
    ///
    /// Fails for a name the container does not declare, rather than adding one:
    /// a typo in an authored property is otherwise a value that silently goes
    /// nowhere.
    pub fn set_field(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        match self.fields.get_mut(name) {
            Some(slot) => {
                slot.value = value;
                Ok(())
            }
            None => Err(RuntimeError::UnknownPath(format!(
                "{}.{name}",
                self.container_name
            ))),
        }
    }

    /// Every field this instance holds, in declaration-independent order.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.fields
            .iter()
            .map(|(name, slot)| (name.as_str(), &slot.value))
    }
}

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

    pub fn instantiate(&mut self, container_name: &str) -> Result<ScriptInstance, RuntimeError> {
        let container = self.find_container(container_name)?.clone();
        let fields = self.initialize_fields(&container)?;
        Ok(ScriptInstance {
            container_name: container_name.to_owned(),
            fields,
        })
    }

    pub fn call(
        &mut self,
        container_name: &str,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let mut instance = self.instantiate(container_name)?;
        self.call_instance(&mut instance, function_name, args)
    }

    pub fn call_instance(
        &mut self,
        instance: &mut ScriptInstance,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let container = self.find_container(&instance.container_name)?.clone();
        self.call_in_container(&container, &mut instance.fields, function_name, args)
    }

    fn find_container(&self, name: &str) -> Result<&IrContainer, RuntimeError> {
        self.program
            .containers
            .iter()
            .find(|container| container.name == name)
            .ok_or_else(|| RuntimeError::ContainerNotFound(name.to_owned()))
    }

    fn initialize_fields(
        &mut self,
        container: &IrContainer,
    ) -> Result<HashMap<String, Slot>, RuntimeError> {
        let mut fields = HashMap::new();
        for field in &container.fields {
            let value = if let Some(initializer) = &field.initializer {
                let mut frame = Frame::new(HashMap::new());
                self.execute_instructions(container, &mut fields, &mut frame, initializer)?
            } else {
                Value::Null
            };
            fields.insert(
                field.name.clone(),
                Slot {
                    value,
                    mutable: field.mutable,
                },
            );
        }
        Ok(fields)
    }

    fn call_in_container(
        &mut self,
        container: &IrContainer,
        fields: &mut HashMap<String, Slot>,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let function = container
            .functions
            .iter()
            .find(|function| function.name == function_name)
            .ok_or_else(|| RuntimeError::FunctionNotFound(function_name.to_owned()))?;
        if function.params.len() != args.len() {
            return Err(RuntimeError::Arity {
                function: function_name.to_owned(),
                expected: function.params.len(),
                found: args.len(),
            });
        }
        let locals = function
            .params
            .iter()
            .cloned()
            .zip(args)
            .map(|(name, value)| {
                (
                    name,
                    Slot {
                        value,
                        mutable: false,
                    },
                )
            })
            .collect();
        let mut frame = Frame::new(locals);

        // Counted here rather than around `execute_instructions`, because a
        // field initializer and a function body both run instructions and only
        // one of them is a call.
        if self.depth >= self.call_depth_limit {
            return Err(RuntimeError::CallDepthExceeded {
                function: function_name.to_owned(),
                limit: self.call_depth_limit,
            });
        }
        self.depth += 1;
        let result = self.execute_function(container, fields, function, &mut frame);
        self.depth -= 1;
        result
    }

    fn execute_function(
        &mut self,
        container: &IrContainer,
        fields: &mut HashMap<String, Slot>,
        function: &IrFunction,
        frame: &mut Frame,
    ) -> Result<Value, RuntimeError> {
        self.execute_instructions(container, fields, frame, &function.instructions)
    }

    fn execute_instructions(
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

    /// Splits a path rooted at a value the script is holding into that value's
    /// reference and the rest of the path.
    ///
    /// `target.transform.position.x` where `target` holds a reference becomes
    /// `(target's id, transform.position.x)`, which is what lets one script say
    /// anything about another entity. A path rooted at anything else — a host
    /// global like `Input`, or `this` — is not a subject path and goes to the
    /// host whole.
    ///
    /// A root that names a local holding something *other* than a reference is
    /// an error rather than a fall-through to the host: `speed.transform` where
    /// `speed` is a number should say so, not report an unknown host path that
    /// mentions a local the host has never heard of.
    fn subject_path(
        fields: &HashMap<String, Slot>,
        frame: &Frame,
        path: &Path,
    ) -> Result<Option<(u64, Path)>, RuntimeError> {
        if path.0.len() < 2 {
            return Ok(None);
        }
        let root = &path.0[0];
        let Some(slot) = frame.lookup(root).or_else(|| fields.get(root)) else {
            return Ok(None);
        };
        match slot.value {
            Value::Reference(id) => Ok(Some((id, Path(path.0[1..].to_vec())))),
            Value::Null => Err(RuntimeError::NullReference(path.dotted())),
            _ => Err(RuntimeError::NotAReference(root.clone())),
        }
    }

    fn load_path(
        &mut self,
        fields: &HashMap<String, Slot>,
        frame: &Frame,
        path: &Path,
    ) -> Result<Value, RuntimeError> {
        if path.0.len() == 1 {
            let name = &path.0[0];
            if let Some(slot) = frame.lookup(name).or_else(|| fields.get(name)) {
                return Ok(slot.value.clone());
            }
        }
        if path.0.len() == 2
            && path.0[0] == "this"
            && let Some(slot) = fields.get(&path.0[1])
        {
            return Ok(slot.value.clone());
        }
        if let Some((subject, rest)) = Self::subject_path(fields, frame, path)? {
            return self
                .host
                .load(Some(subject), &rest)?
                .ok_or_else(|| RuntimeError::UnknownPath(rest.dotted()));
        }
        self.host
            .load(None, path)?
            .ok_or_else(|| RuntimeError::UnknownPath(path.dotted()))
    }

    fn store_path(
        &mut self,
        fields: &mut HashMap<String, Slot>,
        frame: &mut Frame,
        path: &Path,
        value: Value,
    ) -> Result<(), RuntimeError> {
        if path.0.len() == 1 {
            let name = &path.0[0];
            if let Some(slot) = frame.lookup_mut(name) {
                if !slot.mutable {
                    return Err(RuntimeError::Immutable(name.clone()));
                }
                slot.value = value;
                return Ok(());
            }
            if let Some(slot) = fields.get_mut(name) {
                if !slot.mutable {
                    return Err(RuntimeError::Immutable(name.clone()));
                }
                slot.value = value;
                return Ok(());
            }
        }
        if path.0.len() == 2
            && path.0[0] == "this"
            && let Some(slot) = fields.get_mut(&path.0[1])
        {
            if !slot.mutable {
                return Err(RuntimeError::Immutable(path.dotted()));
            }
            slot.value = value;
            return Ok(());
        }
        if let Some((subject, rest)) = Self::subject_path(fields, frame, path)? {
            return if self.host.store(Some(subject), &rest, value)? {
                Ok(())
            } else {
                Err(RuntimeError::UnknownPath(rest.dotted()))
            };
        }
        if self.host.store(None, path, value)? {
            Ok(())
        } else {
            Err(RuntimeError::UnknownPath(path.dotted()))
        }
    }
}

struct Frame {
    /// Innermost scope last. The base scope holds the parameters and the
    /// function body's own bindings, which is the arrangement the analyzer
    /// checks against: a parameter and a body binding of the same name are a
    /// duplicate, not a shadow.
    scopes: Vec<HashMap<String, Slot>>,
    stack: Vec<Value>,
}

impl Frame {
    fn new(locals: HashMap<String, Slot>) -> Self {
        Self {
            scopes: vec![locals],
            stack: Vec::new(),
        }
    }

    fn declare(&mut self, name: &str, slot: Slot) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), slot);
        }
    }

    fn lookup(&self, name: &str) -> Option<&Slot> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Slot> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }
}

fn apply_unary(op: UnaryOp, value: Value) -> Result<Value, RuntimeError> {
    match (op, value) {
        (UnaryOp::Negate, Value::Number(value)) => Ok(Value::Number(-value)),
        (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
        _ => Err(RuntimeError::InvalidUnary),
    }
}

fn apply_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    match op {
        BinaryOp::Add => numbers(left, right, |a, b| a + b),
        BinaryOp::Subtract => numbers(left, right, |a, b| a - b),
        BinaryOp::Multiply => numbers(left, right, |a, b| a * b),
        BinaryOp::Divide => numbers(left, right, |a, b| a / b),
        BinaryOp::Less => compare(left, right, |a, b| a < b),
        BinaryOp::LessEqual => compare(left, right, |a, b| a <= b),
        BinaryOp::Greater => compare(left, right, |a, b| a > b),
        BinaryOp::GreaterEqual => compare(left, right, |a, b| a >= b),
        BinaryOp::And => booleans(left, right, |a, b| a && b),
        BinaryOp::Or => booleans(left, right, |a, b| a || b),
        BinaryOp::Equal => Ok(Value::Bool(left == right)),
        BinaryOp::NotEqual => Ok(Value::Bool(left != right)),
    }
}

fn numbers(
    left: Value,
    right: Value,
    op: impl FnOnce(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(op(a, b))),
        _ => Err(RuntimeError::InvalidBinary),
    }
}
fn compare(
    left: Value,
    right: Value,
    op: impl FnOnce(f64, f64) -> bool,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err(RuntimeError::InvalidBinary),
    }
}
fn booleans(
    left: Value,
    right: Value,
    op: impl FnOnce(bool, bool) -> bool,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err(RuntimeError::InvalidBinary),
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_CALL_DEPTH_LIMIT, EmptyHost, Host, Path, Runtime, RuntimeError, Value};
    use decay_ir::lower_with_environment;
    use decay_semantic::{Environment, Type};
    use std::collections::HashMap;

    #[test]
    fn executes_arithmetic_and_return() {
        let lowered = decay_ir::lower(
            r"script Math { fn double(value: f32) -> f32 { return value * 2.0; } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(
            runtime.call("Math", "double", vec![Value::Number(6.0)]),
            Ok(Value::Number(12.0))
        );
    }

    #[test]
    fn executes_if_else() {
        let lowered = decay_ir::lower(
            r"script Choice { fn pick(flag: bool) -> f32 { if flag { return 1.0; } else { return 2.0; } } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(
            runtime.call("Choice", "pick", vec![Value::Bool(false)]),
            Ok(Value::Number(2.0))
        );
    }

    #[test]
    fn calls_other_decay_functions() {
        let lowered = decay_ir::lower(
            r"script Math { fn double(value: f32) -> f32 { return value * 2.0; } fn eight() -> f32 { return double(4.0); } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(
            runtime.call("Math", "eight", vec![]),
            Ok(Value::Number(8.0))
        );
    }

    #[test]
    fn instance_fields_persist_between_calls() {
        let lowered = decay_ir::lower(
            r"script Counter { var count: f32 = 0.0; fn tick() -> f32 { count += 1.0; return count; } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        let mut instance = runtime.instantiate("Counter").expect("instance");
        assert_eq!(
            runtime.call_instance(&mut instance, "tick", vec![]),
            Ok(Value::Number(1.0))
        );
        assert_eq!(
            runtime.call_instance(&mut instance, "tick", vec![]),
            Ok(Value::Number(2.0))
        );
        assert_eq!(instance.field("count"), Some(&Value::Number(2.0)));
    }

    /// Every `let` local used to fail here, because the binding's own
    /// initialization went through the same store as an assignment and was
    /// refused for not being mutable. No test executed one, so nothing noticed
    /// — including the example in the README.
    #[test]
    fn a_let_local_can_be_bound_and_read() {
        let lowered = decay_ir::lower(
            r"script T { fn run() -> f32 { let speed: f32 = 6.0; return speed * 2.0; } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(runtime.call("T", "run", vec![]), Ok(Value::Number(12.0)));
    }

    /// The analyzer has always scoped blocks. The IR did not, so a shadowing
    /// declaration overwrote the name it shadowed and kept it overwritten —
    /// type-checking cleanly and then returning the wrong number.
    #[test]
    fn a_binding_inside_a_block_does_not_outlive_it() {
        let lowered = decay_ir::lower(
            r"script T { fn run(flag: bool) -> f32 { var x: f32 = 1.0; if flag { var x: f32 = 2.0; } return x; } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(
            runtime.call("T", "run", vec![Value::Bool(true)]),
            Ok(Value::Number(1.0)),
            "the inner binding shadows the outer one rather than replacing it"
        );
    }

    /// A block still writes through to what it did not declare, which is the
    /// half of scoping that shadowing must not break.
    #[test]
    fn a_block_still_assigns_to_a_name_it_did_not_declare() {
        let lowered = decay_ir::lower(
            r"script T { fn run(flag: bool) -> f32 { var x: f32 = 1.0; if flag { x = 2.0; } return x; } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(
            runtime.call("T", "run", vec![Value::Bool(true)]),
            Ok(Value::Number(2.0))
        );
    }

    /// Unbounded recursion overflowed the host's stack and aborted the process.
    /// A runtime that executes author scripts inside the editor has to hand the
    /// problem back instead of taking the editor with it.
    #[test]
    fn runaway_recursion_is_an_error_rather_than_a_crash() {
        let lowered = decay_ir::lower(r"script T { fn boom() -> f32 { return boom(); } }");
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        assert_eq!(
            runtime.call("T", "boom", vec![]),
            Err(RuntimeError::CallDepthExceeded {
                function: "boom".to_owned(),
                limit: DEFAULT_CALL_DEPTH_LIMIT,
            })
        );
    }

    /// The limit has to leave room for the nesting real code does, so it is
    /// checked from both sides rather than only from the runaway one.
    #[test]
    fn nesting_below_the_limit_still_runs() {
        let lowered = decay_ir::lower(
            r"script T { fn down(n: f32) -> f32 { if n <= 0.0 { return 0.0; } return 1.0 + down(n - 1.0); } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost).with_call_depth_limit(8);
        assert_eq!(
            runtime.call("T", "down", vec![Value::Number(5.0)]),
            Ok(Value::Number(5.0))
        );
    }

    /// The host sets an exported field before the script runs, which is what
    /// `@export` is for. It ignores mutability on purpose: `let` means the
    /// script does not reassign it, not that the author cannot author it.
    #[test]
    fn a_host_can_set_an_exported_field_the_script_treats_as_immutable() {
        let lowered = decay_ir::lower(
            r"script Player { let speed: f32 = 1.0; fn run() -> f32 { return speed; } }",
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        let mut instance = runtime.instantiate("Player").expect("instance");

        instance
            .set_field("speed", Value::Number(6.0))
            .expect("an authored property is applied");
        assert_eq!(
            runtime.call_instance(&mut instance, "run", vec![]),
            Ok(Value::Number(6.0))
        );
    }

    /// A property naming a field that does not exist is a mistake worth
    /// hearing about, not a value that quietly goes nowhere.
    #[test]
    fn setting_a_field_the_script_does_not_declare_is_refused() {
        let lowered = decay_ir::lower(r"script Player { let speed: f32 = 1.0; }");
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, EmptyHost);
        let mut instance = runtime.instantiate("Player").expect("instance");

        assert_eq!(
            instance.set_field("sped", Value::Number(6.0)),
            Err(RuntimeError::UnknownPath("Player.sped".to_owned()))
        );
    }

    #[derive(Default)]
    struct TestHost {
        values: HashMap<String, Value>,
    }
    impl Host for TestHost {
        fn load(
            &mut self,
            _subject: Option<u64>,
            path: &Path,
        ) -> Result<Option<Value>, RuntimeError> {
            Ok(self.values.get(&path.dotted()).cloned())
        }
        fn store(
            &mut self,
            _subject: Option<u64>,
            path: &Path,
            value: Value,
        ) -> Result<bool, RuntimeError> {
            self.values.insert(path.dotted(), value);
            Ok(true)
        }
        fn call(
            &mut self,
            _subject: Option<u64>,
            path: &Path,
            args: &[Value],
        ) -> Result<Option<Value>, RuntimeError> {
            if path.dotted() == "Input.axis" {
                assert_eq!(args.len(), 2);
                return Ok(Some(Value::Number(0.5)));
            }
            Ok(None)
        }
    }

    #[test]
    fn host_calls_cross_only_the_host_boundary() {
        let mut environment = Environment::new();
        environment.add_value("Input", Type::Named("Input".to_owned()));
        let lowered = lower_with_environment(
            r#"script Player { fn movement() -> f32 { return Input.axis("left", "right"); } }"#,
            &environment,
        );
        let program = lowered.program.expect("valid program");
        let mut runtime = Runtime::new(&program, TestHost::default());
        assert_eq!(
            runtime.call("Player", "movement", vec![]),
            Ok(Value::Number(0.5))
        );
    }
}
