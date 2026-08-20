//! Does the language reference describe the language?
//!
//! `decay/LANGUAGE.md` makes specific, checkable claims — that `else if` is a
//! parse error, that there is no truthiness, that every number is a float.
//! Documentation of a language that is still moving goes stale silently and is
//! then believed, which is worse than having none. So the claims are assertions.
//!
//! When one of these fails, the language changed and the reference is now
//! wrong: fix the document in the same commit as the change.

use decay_ir::lower_with_environment;
use decay_semantic::{Environment, FunctionType, HostType, Type};

/// A host offering the one function the reference's example calls.
fn environment() -> Environment {
    let mut environment = Environment::new();
    environment.add_function(
        "sin",
        FunctionType {
            params: vec![Type::F32],
            return_type: Type::F32,
        },
    );
    environment
}

fn compiles(source: &str) -> bool {
    lower_with_environment(source, &environment())
        .analysis
        .diagnostics
        .is_empty()
}

#[track_caller]
fn accepted(what: &str, source: &str) {
    assert!(compiles(source), "LANGUAGE.md says {what} is accepted");
}

#[track_caller]
fn rejected(what: &str, source: &str) {
    assert!(!compiles(source), "LANGUAGE.md says {what} is rejected");
}

/// "What does not exist". An absence nothing checks is how a document starts
/// lying.
#[test]
fn nothing_the_reference_calls_absent_compiles() {
    rejected("`while`", "script T { fn f() { while true { } } }");
    rejected("`for`", "script T { fn f() { for i in 0..3 { } } }");
    rejected("`loop`", "script T { fn f() { loop { } } }");
    rejected("`match`", "script T { fn f(a: f32) { match a { } } }");
    rejected(
        "a chained `else if`",
        "script T { fn f(a: bool) { if a { } else if a { } } }",
    );
    rejected(
        "array literals",
        "script T { fn f() { let a: f32 = [1.0]; } }",
    );
    rejected(
        "indexing",
        "script T { fn f(a: f32) { let b: f32 = a[0]; } }",
    );
    rejected("block comments", "script T { /* nope */ }");
    rejected(
        "exponent literals",
        "script T { fn f() { let a: f32 = 1e6; } }",
    );
    rejected("hex literals", "script T { fn f() { let a: f32 = 0xff; } }");
    rejected(
        "digit separators",
        "script T { fn f() { let a: f32 = 1_000; } }",
    );
    rejected("`import`", "import other; script T { }");
    rejected("attributes on functions", "script T { @export fn f() {} }");
}

/// "Surprising behaviour": the claims a reader is most likely to guess wrong,
/// which makes them the ones most worth holding still.
#[test]
fn the_surprises_are_still_surprising() {
    rejected(
        "`if` without a bool condition, because there is no truthiness",
        "script T { fn f() { if 1.0 { } } }",
    );
    rejected(
        "`+` on strings, because it does not concatenate",
        r#"script T { fn f() { let s: String = "a" + "b"; } }"#,
    );
    rejected(
        "a body binding reusing a parameter name, because they share one scope",
        "script T { fn f(x: f32) { let x: f32 = 1.0; } }",
    );
    rejected(
        "assignment to a `let` binding",
        "script T { fn f() { let x: f32 = 1.0; x = 2.0; } }",
    );
    accepted(
        "`this.field` and the bare name naming the same field",
        "script T { var a: f32 = 1.0; fn f() { this.a = 2.0; a = 3.0; } }",
    );
    accepted(
        "a field reading a field declared above it",
        "script T { let a: f32 = 2.0; let b: f32 = a; }",
    );
    accepted(
        "a field reading one declared below it, which fails only at runtime",
        "script T { let a: f32 = b; let b: f32 = 2.0; }",
    );
    accepted(
        "any path into a type the host has not described",
        "script T { fn f() { this.transfrom.position.x = 1.0; } }",
    );
    rejected(
        "reaching for a method on a container, because there are none",
        "script T { fn helper() -> f32 { return 1.0; } fn f() { this.helper(); } }",
    );
}

/// The other half of the member rule: once a host describes a type, its members
/// are checked. Both halves matter — checking is the point, and staying
/// permissive about the undescribed is what makes describing a host gradual.
#[test]
fn a_described_type_is_checked_and_an_undescribed_one_is_not() {
    let mut described = Environment::new();
    described.add_type(
        "Vec3",
        HostType::new()
            .with_value("x", Type::F32)
            .with_value("y", Type::F32),
    );
    described.add_this_value("position", Type::Named("Vec3".to_owned()));
    // Named but never described, which is the permissive case.
    described.add_this_value("body", Type::Named("RigidBody".to_owned()));

    let check = |source: &str| {
        lower_with_environment(source, &described)
            .analysis
            .diagnostics
            .is_empty()
    };

    assert!(
        check("script T { fn f() { this.position.x = 1.0; } }"),
        "a member the host described is accepted"
    );
    assert!(
        !check("script T { fn f() { this.position.z = 1.0; } }"),
        "a member it did not describe on a type it did is refused"
    );
    assert!(
        !check("script T { fn f() { this.anything = 1.0; } }"),
        "and once `this` is described, a member it does not have is refused too"
    );
    assert!(
        check("script T { fn f() { this.body.anything.at.all = 1.0; } }"),
        "but a named type it never described stays permissive"
    );
}

/// Parentheses around a condition are allowed and do nothing, which is pinned
/// because the reference used to imply they were an error.
#[test]
fn parentheses_around_a_condition_are_allowed_and_pointless() {
    accepted(
        "a parenthesised condition",
        "script T { fn f(a: bool) { if (a) { } } }",
    );
    accepted(
        "an unparenthesised condition",
        "script T { fn f(a: bool) { if a { } } }",
    );
}

/// The reference's complete example is its claim about what a real script looks
/// like, so it has to be one.
#[test]
fn the_documented_example_compiles() {
    accepted(
        "the reference's complete example",
        r#"
        script PlayerController {
            @export
            let speed: f32 = 6.0;

            @export
            let label: String = "player";

            var elapsed: f32 = 0.0;
            var airborne: bool = false;

            fn start() {
                elapsed = 0.0;
            }

            fn update(dt: f32) {
                elapsed += dt;
                let offset: f32 = wave(elapsed) * speed;
                this.transform.position.x = offset;
                if offset > 0.0 {
                    airborne = true;
                } else {
                    airborne = false;
                }
            }

            fn wave(seconds: f32) -> f32 {
                return sin(seconds);
            }
        }
        "#,
    );
}
