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
    Host(String),
}

pub trait Host {
    fn load(&mut self, path: &Path) -> Result<Option<Value>, RuntimeError>;
    fn store(&mut self, path: &Path, value: Value) -> Result<bool, RuntimeError>;
    fn call(&mut self, path: &Path, args: &[Value]) -> Result<Option<Value>, RuntimeError>;
}

#[derive(Debug, Default)]
pub struct EmptyHost;

impl Host for EmptyHost {
    fn load(&mut self, _path: &Path) -> Result<Option<Value>, RuntimeError> {
        Ok(None)
    }
    fn store(&mut self, _path: &Path, _value: Value) -> Result<bool, RuntimeError> {
        Ok(false)
    }
    fn call(&mut self, _path: &Path, _args: &[Value]) -> Result<Option<Value>, RuntimeError> {
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
}

pub struct Runtime<'a, H: Host> {
    program: &'a IrProgram,
    host: H,
}

impl<'a, H: Host> Runtime<'a, H> {
    #[must_use]
    pub fn new(program: &'a IrProgram, host: H) -> Self {
        Self { program, host }
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
        self.execute_function(container, fields, function, &mut frame)
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
                    frame.locals.insert(
                        name.clone(),
                        Slot {
                            value: Value::Null,
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
                    } else if let Some(value) = self.host.call(callee, &args)? {
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
            }
            ip += 1;
        }
        Ok(frame.stack.pop().unwrap_or(Value::Unit))
    }

    fn load_path(
        &mut self,
        fields: &HashMap<String, Slot>,
        frame: &Frame,
        path: &Path,
    ) -> Result<Value, RuntimeError> {
        if path.0.len() == 1 {
            let name = &path.0[0];
            if let Some(slot) = frame.locals.get(name).or_else(|| fields.get(name)) {
                return Ok(slot.value.clone());
            }
        }
        if path.0.len() == 2
            && path.0[0] == "this"
            && let Some(slot) = fields.get(&path.0[1])
        {
            return Ok(slot.value.clone());
        }
        self.host
            .load(path)?
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
            if let Some(slot) = frame.locals.get_mut(name) {
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
        if self.host.store(path, value)? {
            Ok(())
        } else {
            Err(RuntimeError::UnknownPath(path.dotted()))
        }
    }
}

struct Frame {
    locals: HashMap<String, Slot>,
    stack: Vec<Value>,
}
impl Frame {
    fn new(locals: HashMap<String, Slot>) -> Self {
        Self {
            locals,
            stack: Vec::new(),
        }
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
    use super::{EmptyHost, Host, Path, Runtime, RuntimeError, Value};
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

    #[derive(Default)]
    struct TestHost {
        values: HashMap<String, Value>,
    }
    impl Host for TestHost {
        fn load(&mut self, path: &Path) -> Result<Option<Value>, RuntimeError> {
            Ok(self.values.get(&path.dotted()).cloned())
        }
        fn store(&mut self, path: &Path, value: Value) -> Result<bool, RuntimeError> {
            self.values.insert(path.dotted(), value);
            Ok(true)
        }
        fn call(&mut self, path: &Path, args: &[Value]) -> Result<Option<Value>, RuntimeError> {
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
