//! Evaluating expressions, branching, and calling.

use decay_ir::lower_with_environment;
use decay_semantic::{Environment, Type};

use crate::{DEFAULT_CALL_DEPTH_LIMIT, EmptyHost, Runtime, RuntimeError, Value};

use super::support::{CountingHost, TestHost, probing_environment};

#[test]
fn executes_arithmetic_and_return() {
    let lowered =
        decay_ir::lower(r"script Math { fn double(value: f32) -> f32 { return value * 2.0; } }");
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

/// `&&` and `||` decide from the left operand whether to evaluate the
/// right, which is only observable through what the right one does. The
/// guard this exists for is `held != null && World.exists(held)`: it reads
/// as protecting the call to its right, and before this it did not.
#[test]
fn boolean_operators_skip_the_right_operand_the_left_decides() {
    let lowered = lower_with_environment(
        r"script Guard {
            fn both(flag: bool) -> bool { return flag && Probe.ready(); }
            fn either(flag: bool) -> bool { return flag || Probe.ready(); }
        }",
        &probing_environment(),
    );
    let program = lowered.program.expect("valid program");

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "both", vec![Value::Bool(false)]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        runtime.into_host().probes,
        0,
        "a false left operand answers `&&` on its own"
    );

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "either", vec![Value::Bool(true)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        runtime.into_host().probes,
        0,
        "a true left operand answers `||` on its own"
    );
}

/// The other half: an operand that is *not* skipped is still evaluated,
/// and still decides the answer.
#[test]
fn boolean_operators_evaluate_the_right_operand_the_left_does_not_decide() {
    let lowered = lower_with_environment(
        r"script Guard {
            fn both(flag: bool) -> bool { return flag && Probe.ready(); }
            fn either(flag: bool) -> bool { return flag || Probe.ready(); }
        }",
        &probing_environment(),
    );
    let program = lowered.program.expect("valid program");

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "both", vec![Value::Bool(true)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(runtime.into_host().probes, 1);

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "either", vec![Value::Bool(false)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(runtime.into_host().probes, 1);
}

/// Chained and nested operators, which is where lowering that patches jump
/// sites tends to go wrong: an inner operator's tail must not be mistaken
/// for an outer one's.
#[test]
fn short_circuiting_survives_chaining_and_nesting() {
    let lowered = lower_with_environment(
        r"script Guard {
            fn chain(a: bool, b: bool) -> bool { return a && b && Probe.ready(); }
            fn mixed(a: bool, b: bool) -> bool { return a || b && Probe.ready(); }
            fn guarded(a: bool) -> f32 { if a && Probe.ready() { return 1.0; } return 0.0; }
        }",
        &probing_environment(),
    );
    let program = lowered.program.expect("valid program");

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call(
            "Guard",
            "chain",
            vec![Value::Bool(true), Value::Bool(false)]
        ),
        Ok(Value::Bool(false))
    );
    assert_eq!(runtime.into_host().probes, 0, "`b` decides the chain");

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "chain", vec![Value::Bool(true), Value::Bool(true)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(runtime.into_host().probes, 1);

    // `&&` binds tighter than `||`, so this is `a || (b && ready())`.
    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "mixed", vec![Value::Bool(true), Value::Bool(true)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(runtime.into_host().probes, 0, "`a` decides the whole of it");

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call(
            "Guard",
            "mixed",
            vec![Value::Bool(false), Value::Bool(false)]
        ),
        Ok(Value::Bool(false))
    );
    assert_eq!(runtime.into_host().probes, 0, "`b` decides the right half");

    // The value a short-circuit leaves behind is what `if` then tests, so
    // the two sets of jumps have to agree about where the answer is.
    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "guarded", vec![Value::Bool(false)]),
        Ok(Value::Number(0.0))
    );
    assert_eq!(runtime.into_host().probes, 0);

    let mut runtime = Runtime::new(&program, CountingHost::default());
    assert_eq!(
        runtime.call("Guard", "guarded", vec![Value::Bool(true)]),
        Ok(Value::Number(1.0))
    );
    assert_eq!(runtime.into_host().probes, 1);
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
