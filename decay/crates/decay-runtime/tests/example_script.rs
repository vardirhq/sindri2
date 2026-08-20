//! Does the example the documentation leads with actually run?
//!
//! `decay/examples/player.decay` is the script the README shows to say what
//! Decay is. Every `let` local in it failed at runtime for the whole of the
//! foundation branch, and nothing noticed, because no test ran the example and
//! no unit test bound a `let` local either. A documented example that does not
//! execute is worse than no example, so this executes it.

use std::collections::HashMap;

use decay_ir::{Path, lower_with_environment};
use decay_runtime::{Host, Runtime, RuntimeError, Value};
use decay_semantic::{Environment, FunctionType, Type};

const SOURCE: &str = include_str!("../../../examples/player.decay");

/// Enough of a host to answer the script, and a record of what it was asked —
/// the point is what crosses the boundary, not what is on the other side.
#[derive(Default)]
struct Recorder {
    values: HashMap<String, Value>,
    calls: Vec<(String, Vec<Value>)>,
}

impl Host for Recorder {
    fn load(&mut self, path: &Path) -> Result<Option<Value>, RuntimeError> {
        Ok(self.values.get(&path.dotted()).cloned())
    }

    fn store(&mut self, path: &Path, value: Value) -> Result<bool, RuntimeError> {
        self.values.insert(path.dotted(), value);
        Ok(true)
    }

    fn call(&mut self, path: &Path, args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let name = path.dotted();
        self.calls.push((name.clone(), args.to_vec()));
        Ok(Some(match name.as_str() {
            // Held right, so the axis reads +1.
            "Input.axis" => Value::Number(1.0),
            "Input.just_pressed" => Value::Bool(true),
            "vec3" => Value::Null,
            _ => Value::Unit,
        }))
    }
}

/// The host globals the example names. Decay knows none of these; they arrive
/// through the environment, which is the boundary the language is built around.
fn environment() -> Environment {
    let mut environment = Environment::new();
    environment.add_value("Input", Type::Named("Input".to_owned()));
    environment.add_function(
        "vec3",
        FunctionType {
            params: vec![Type::F32, Type::F32, Type::F32],
            return_type: Type::Named("Vec3".to_owned()),
        },
    );
    environment
}

#[test]
fn the_documented_example_analyzes_lowers_and_runs() {
    let lowered = lower_with_environment(SOURCE, &environment());
    assert!(
        lowered.analysis.diagnostics.is_empty(),
        "{:?}",
        lowered.analysis.diagnostics
    );
    let program = lowered.program.expect("the example lowers");

    let mut host = Recorder::default();
    // The script reads this before writing it, because `+=` is a read.
    host.values
        .insert("this.transform.position.x".to_owned(), Value::Number(0.0));

    let mut runtime = Runtime::new(&program, host);
    let result = runtime.call("PlayerController", "update", vec![Value::Number(0.5)]);
    assert_eq!(result, Ok(Value::Unit), "the example's update ran");

    let host = runtime.into_host();

    // One frame of half a second, moving right at the authored six units a
    // second. The number matters because it can only be right if the `let`
    // local, the exported field, and the host call all reached the arithmetic.
    assert_eq!(
        host.values.get("this.transform.position.x"),
        Some(&Value::Number(3.0))
    );

    let called: Vec<&str> = host.calls.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        called,
        [
            "Input.axis",
            "Input.just_pressed",
            "vec3",
            "this.rigidbody.add_impulse"
        ],
        "the jump branch is taken, and every engine concept leaves through the host"
    );

    // The exported field reached the call rather than a default.
    let (_, impulse_args) = &host.calls[2];
    assert_eq!(
        impulse_args,
        &[Value::Number(0.0), Value::Number(8.0), Value::Number(0.0)]
    );
}
