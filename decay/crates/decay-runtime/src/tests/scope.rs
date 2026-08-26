//! What a name means, and how long it lasts.

use crate::{EmptyHost, Runtime, Value};

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
