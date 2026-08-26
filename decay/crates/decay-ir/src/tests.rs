//! What lowering produces, and what it refuses to lower.

use decay_semantic::{Environment, Type};

use crate::{Constant, Instruction, Path, lower, lower_with_environment};

#[test]
fn lowers_member_assignment_to_symbolic_path() {
    let lowered = lower(
        r"
        script Player {
            fn update(dt: f32) {
                this.transform.position.x += 6.0 * dt;
            }
        }
        ",
    );
    let program = lowered.program.expect("program should lower");
    let instructions = &program.containers[0].functions[0].instructions;

    assert!(instructions.contains(&Instruction::Load(Path(vec![
        "this".into(),
        "transform".into(),
        "position".into(),
        "x".into(),
    ]))));
    assert!(instructions.contains(&Instruction::Push(Constant::Number(6.0))));
    assert!(instructions.contains(&Instruction::Store(Path(vec![
        "this".into(),
        "transform".into(),
        "position".into(),
        "x".into(),
    ]))));
}

#[test]
fn lowers_host_call_without_knowing_host_semantics() {
    let mut environment = Environment::new();
    environment.add_value("Input", Type::Named("Input".to_owned()));
    let lowered = lower_with_environment(
        r#"
        script Player {
            fn update() {
                Input.axis("left", "right");
            }
        }
        "#,
        &environment,
    );
    let program = lowered.program.expect("program should lower");
    let instructions = &program.containers[0].functions[0].instructions;

    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::Call { callee, argument_count: 2 }
            if callee == &Path(vec!["Input".into(), "axis".into()])
    )));
}

#[test]
fn patches_if_else_jump_targets() {
    let lowered = lower(
        r"
        script Test {
            fn run(flag: bool) {
                if flag {
                    var x: f32 = 1.0;
                } else {
                    var x: f32 = 2.0;
                }
            }
        }
        ",
    );
    let program = lowered.program.expect("program should lower");
    let instructions = &program.containers[0].functions[0].instructions;

    for instruction in instructions {
        match instruction {
            Instruction::JumpIfFalse(target) | Instruction::Jump(target) => {
                assert!(*target <= instructions.len());
                assert_ne!(*target, usize::MAX);
            }
            _ => {}
        }
    }
}

/// A binding evaluates its initializer and then `Declare` takes the value.
/// The shape matters: the moment initialization becomes a `Store`, it is
/// subject to the mutability rule and every `let` binding fails.
#[test]
fn a_binding_declares_its_value_rather_than_storing_it() {
    let lowered = lower(r"script T { fn run() { let speed: f32 = 6.0; } }");
    let program = lowered.program.expect("program should lower");
    let instructions = &program.containers[0].functions[0].instructions;

    assert_eq!(
        instructions[..2],
        [
            Instruction::Push(Constant::Number(6.0)),
            Instruction::Declare {
                name: "speed".into(),
                mutable: false,
            },
        ]
    );
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Store(_))),
        "a binding is not a store"
    );
}

/// Blocks carry their scope into the IR, and the enter/exit stay paired
/// whichever way the branch jumps.
#[test]
fn blocks_open_and_close_a_scope() {
    let lowered = lower(
        r"script T { fn run(flag: bool) { if flag { var x: f32 = 1.0; } else { var x: f32 = 2.0; } } }",
    );
    let program = lowered.program.expect("program should lower");
    let instructions = &program.containers[0].functions[0].instructions;

    let enters = instructions
        .iter()
        .filter(|instruction| **instruction == Instruction::ScopeEnter)
        .count();
    let exits = instructions
        .iter()
        .filter(|instruction| **instruction == Instruction::ScopeExit)
        .count();
    assert_eq!((enters, exits), (2, 2), "one scope per branch");

    // Whichever branch runs, it enters exactly one scope and leaves it.
    let mut depth = 0i32;
    let mut lowest = 0i32;
    for instruction in instructions {
        match instruction {
            Instruction::ScopeEnter => depth += 1,
            Instruction::ScopeExit => depth -= 1,
            _ => {}
        }
        lowest = lowest.min(depth);
    }
    assert_eq!(lowest, 0, "no exit precedes its enter");
}

#[test]
fn refuses_to_lower_semantically_invalid_source() {
    let lowered = lower(
        r"
        script Broken {
            fn run() {
                let value: bool = 1.0;
            }
        }
        ",
    );

    assert!(lowered.program.is_none());
    assert!(!lowered.analysis.diagnostics.is_empty());
}
