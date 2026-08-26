//! What the language itself accepts and refuses, with no host involved.

use crate::{DiagnosticPhase, analyze};

#[test]
fn accepts_typed_gameplay_code() {
    let analysis = analyze(
        r"
        script Player {
            let speed: f32 = 6.0;

            fn update(dt: f32) {
                var movement: f32 = 1.0;
                movement += speed * dt;

                if movement > 0.0 {
                    movement = movement - 1.0;
                }
            }
        }
        ",
    );

    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
}

#[test]
fn reports_unknown_names_and_type_mismatches() {
    let analysis = analyze(
        r"
        script Broken {
            fn update(dt: f32) {
                var alive: bool = true;
                alive = 1.0;
                missing = dt;
            }
        }
        ",
    );

    assert!(analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.phase == DiagnosticPhase::Semantic
            && diagnostic.message.contains("cannot assign `f32` to `bool`")
    }));
    assert!(analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.phase == DiagnosticPhase::Semantic
            && diagnostic.message.contains("unknown name `missing`")
    }));
}

#[test]
fn rejects_assignment_to_immutable_bindings() {
    let analysis = analyze(
        r"
        script Player {
            fn update() {
                let speed: f32 = 6.0;
                speed = 8.0;
            }
        }
        ",
    );

    assert!(analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot assign to immutable `speed`")
    }));
}

#[test]
fn catches_duplicate_members_and_locals() {
    let analysis = analyze(
        r"
        script Player {
            let speed: f32 = 1.0;
            let speed: f32 = 2.0;

            fn update() {
                let value: f32 = 1.0;
                let value: f32 = 2.0;
            }
        }
        ",
    );

    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("duplicate member `speed`") })
    );
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("duplicate local `value`") })
    );
}
