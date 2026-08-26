//! Instance fields, and what a host may do to them.

use crate::{EmptyHost, Runtime, RuntimeError, Value};

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
