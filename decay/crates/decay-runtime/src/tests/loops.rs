//! `while`, the statements that leave it, and the budget that bounds it.

use decay_ir::lower;

use crate::{EmptyHost, Runtime, RuntimeError, Value};

fn run(source: &str, function: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let lowered = lower(source);
    let program = lowered.program.expect("valid program");
    let mut runtime = Runtime::new(&program, EmptyHost);
    runtime.call("Loops", function, args)
}

#[test]
fn a_while_loop_runs_until_its_condition_is_false() {
    assert_eq!(
        run(
            r"script Loops {
                fn sum(limit: f32) -> f32 {
                    var total: f32 = 0.0;
                    var i: f32 = 0.0;
                    while i < limit {
                        total += i;
                        i += 1.0;
                    }
                    return total;
                }
            }",
            "sum",
            vec![Value::Number(5.0)],
        ),
        Ok(Value::Number(10.0))
    );
}

#[test]
fn a_condition_that_is_false_to_begin_with_runs_nothing() {
    assert_eq!(
        run(
            r"script Loops {
                fn never() -> f32 {
                    var touched: f32 = 0.0;
                    while false {
                        touched = 1.0;
                    }
                    return touched;
                }
            }",
            "never",
            vec![],
        ),
        Ok(Value::Number(0.0))
    );
}

#[test]
fn break_leaves_the_loop_and_continue_returns_to_its_condition() {
    // Counts to ten, skipping the fifth turn and stopping on the eighth, so a
    // `continue` that skipped the increment or a `break` that fell through
    // would both produce a different number.
    assert_eq!(
        run(
            r"script Loops {
                fn counted() -> f32 {
                    var i: f32 = 0.0;
                    var seen: f32 = 0.0;
                    while i < 10.0 {
                        i += 1.0;
                        if i == 5.0 {
                            continue;
                        }
                        if i == 8.0 {
                            break;
                        }
                        seen += 1.0;
                    }
                    return seen;
                }
            }",
            "counted",
            vec![],
        ),
        Ok(Value::Number(6.0))
    );
}

/// A `break` or `continue` jumps over the `ScopeExit` instructions between it
/// and the loop. Unlike a `return`, which takes the frame with it, the frame
/// keeps running — so the scopes it skipped have to be closed before it goes,
/// or a binding inside the loop outlives the turn that declared it and the next
/// turn's declaration lands in the wrong scope.
#[test]
fn leaving_a_nested_block_closes_the_scopes_it_leaves() {
    assert_eq!(
        run(
            r"script Loops {
                fn shadowed() -> f32 {
                    var i: f32 = 0.0;
                    var total: f32 = 0.0;
                    while i < 3.0 {
                        i += 1.0;
                        {
                            let inner: f32 = 10.0;
                            total += inner;
                            if i == 1.0 {
                                continue;
                            }
                        }
                        total += 1.0;
                    }
                    return total;
                }
            }",
            "shadowed",
            vec![],
        ),
        Ok(Value::Number(32.0))
    );
}

#[test]
fn a_loop_that_never_ends_is_stopped_rather_than_hanging() {
    let lowered = lower(
        r"script Loops {
            fn forever() -> f32 {
                var i: f32 = 0.0;
                while true {
                    i += 1.0;
                }
                return i;
            }
        }",
    );
    let program = lowered.program.expect("valid program");
    let mut runtime = Runtime::new(&program, EmptyHost).with_operation_budget(10_000);
    assert_eq!(
        runtime.call("Loops", "forever", vec![]),
        Err(RuntimeError::OperationBudgetExceeded { limit: 10_000 })
    );
}

/// The budget is per outermost call, so an ordinary script is not charged for
/// what the last frame did.
#[test]
fn each_call_starts_with_its_whole_budget() {
    let lowered = lower(
        r"script Loops {
            fn spin() -> f32 {
                var i: f32 = 0.0;
                while i < 50.0 {
                    i += 1.0;
                }
                return i;
            }
        }",
    );
    let program = lowered.program.expect("valid program");
    let mut runtime = Runtime::new(&program, EmptyHost).with_operation_budget(1_000);
    let mut instance = runtime.instantiate("Loops").expect("instance");
    for _ in 0..5 {
        assert_eq!(
            runtime.call_instance(&mut instance, "spin", vec![]),
            Ok(Value::Number(50.0))
        );
    }
}

#[test]
fn modulo_takes_its_sign_from_the_left_operand() {
    assert_eq!(
        run(
            r"script Loops { fn wrap(a: f32, b: f32) -> f32 { return a % b; } }",
            "wrap",
            vec![Value::Number(7.0), Value::Number(3.0)],
        ),
        Ok(Value::Number(1.0))
    );
    assert_eq!(
        run(
            r"script Loops { fn wrap(a: f32, b: f32) -> f32 { return a % b; } }",
            "wrap",
            vec![Value::Number(-7.0), Value::Number(3.0)],
        ),
        Ok(Value::Number(-1.0))
    );
    assert_eq!(
        run(
            r"script Loops {
                fn drain(elapsed: f32, step: f32) -> f32 {
                    var left: f32 = elapsed;
                    left %= step;
                    return left;
                }
            }",
            "drain",
            vec![Value::Number(0.7), Value::Number(0.32)],
        ),
        Ok(Value::Number(0.7_f64 % 0.32_f64))
    );
}

/// `else if` is a desugar, so the thing worth testing is that a chain picks the
/// right arm rather than that it parses.
#[test]
fn an_else_if_chain_picks_one_arm() {
    let source = r"script Loops {
        fn band(n: f32) -> f32 {
            if n < 1.0 {
                return 1.0;
            } else if n < 2.0 {
                return 2.0;
            } else if n < 3.0 {
                return 3.0;
            } else {
                return 4.0;
            }
        }
    }";
    for (input, expected) in [(0.5, 1.0), (1.5, 2.0), (2.5, 3.0), (9.0, 4.0)] {
        assert_eq!(
            run(source, "band", vec![Value::Number(input)]),
            Ok(Value::Number(expected))
        );
    }
}
