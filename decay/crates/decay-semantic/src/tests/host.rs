//! What a host puts in scope, and how strictly it is then checked.

use crate::{Environment, FunctionType, HostType, Type, analyze, analyze_with_environment};

#[test]
fn host_globals_are_injected_without_engine_dependencies() {
    let mut environment = Environment::new();
    environment.add_function(
        "delta",
        FunctionType {
            params: vec![],
            return_type: Type::F32,
        },
    );
    environment.add_value("Input", Type::Named("Input".to_owned()));

    let analysis = analyze_with_environment(
        r#"
        script Player {
            fn update() {
                let dt: f32 = delta();
                Input.axis("left", "right");
            }
        }
        "#,
        &environment,
    );

    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
}

/// A host that has described a type gets its members checked, which is the
/// whole point: a misspelled component field is a compile error rather than
/// a runtime failure at frame one.
#[test]
fn a_described_type_checks_its_members() {
    let mut environment = Environment::new();
    environment.add_type(
        "Vec3",
        HostType::new()
            .with_value("x", Type::F32)
            .with_value("y", Type::F32)
            .with_value("z", Type::F32),
    );
    environment.add_type(
        "Transform",
        HostType::new().with_value("position", Type::Named("Vec3".to_owned())),
    );
    environment.add_this_value("transform", Type::Named("Transform".to_owned()));

    let good = analyze_with_environment(
        r"script Player { fn update(dt: f32) { this.transform.position.x += dt; } }",
        &environment,
    );
    assert!(good.diagnostics.is_empty(), "{:?}", good.diagnostics);

    let typo = analyze_with_environment(
        r"script Player { fn update(dt: f32) { this.transfrom.position.x += dt; } }",
        &environment,
    );
    assert!(
        typo.diagnostics
            .iter()
            .any(|d| d.message.contains("has no member `transfrom`")),
        "{:?}",
        typo.diagnostics
    );

    let deep_typo = analyze_with_environment(
        r"script Player { fn update(dt: f32) { this.transform.position.w += dt; } }",
        &environment,
    );
    assert!(
        deep_typo
            .diagnostics
            .iter()
            .any(|d| d.message.contains("`Vec3` has no member `w`")),
        "{:?}",
        deep_typo.diagnostics
    );
}

/// A member's type is a real type, so what is done with it is checked too.
#[test]
fn a_members_type_is_enforced_like_any_other() {
    let mut environment = Environment::new();
    environment.add_type("Sprite", HostType::new().with_value("visible", Type::Bool));
    environment.add_this_value("sprite", Type::Named("Sprite".to_owned()));

    let analysis = analyze_with_environment(
        r"script Player { fn update(dt: f32) { this.sprite.visible = dt; } }",
        &environment,
    );
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|d| d.message.contains("cannot assign `f32` to `bool`")),
        "{:?}",
        analysis.diagnostics
    );
}

/// Describing the host is gradual. A named type nobody has described keeps
/// behaving exactly as everything did before types existed, so a host
/// part-way through describing itself does not reject working scripts.
#[test]
fn an_undescribed_type_stays_permissive() {
    let mut environment = Environment::new();
    environment.add_this_value("rigidbody", Type::Named("RigidBody".to_owned()));

    let analysis = analyze_with_environment(
        r"script Player { fn update(dt: f32) { this.rigidbody.anything.at.all = dt; } }",
        &environment,
    );
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
}

/// A container's own field wins over anything the host attached, so the
/// engine growing a name cannot shadow a script's state.
#[test]
fn a_container_field_wins_over_a_host_member() {
    let mut environment = Environment::new();
    environment.add_this_value("speed", Type::Named("Opaque".to_owned()));

    let analysis = analyze_with_environment(
        r"script Player { var speed: f32 = 1.0; fn update(dt: f32) { this.speed += dt; } }",
        &environment,
    );
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
}

/// Methods on host types are checked against a real signature rather than
/// waved through because the callee was unknown.
#[test]
fn a_host_method_checks_its_arguments() {
    let mut environment = Environment::new();
    environment.add_type(
        "RigidBody",
        HostType::new().with_function(
            "add_impulse",
            FunctionType {
                params: vec![Type::F32, Type::F32],
                return_type: Type::Unit,
            },
        ),
    );
    environment.add_this_value("rigidbody", Type::Named("RigidBody".to_owned()));

    let good = analyze_with_environment(
        r"script Player { fn update() { this.rigidbody.add_impulse(0.0, 1.0); } }",
        &environment,
    );
    assert!(good.diagnostics.is_empty(), "{:?}", good.diagnostics);

    let wrong = analyze_with_environment(
        r"script Player { fn update() { this.rigidbody.add_impulse(0.0); } }",
        &environment,
    );
    assert!(
        wrong
            .diagnostics
            .iter()
            .any(|d| d.message.contains("expected 2 argument(s), found 1")),
        "{:?}",
        wrong.diagnostics
    );
}

/// Decay has no methods. `this.helper()` lowered to a host path call that no
/// host implements, so it failed at runtime looking like the engine's fault;
/// now it says what to write instead.
#[test]
fn reaching_for_a_method_says_what_to_write_instead() {
    let analysis = analyze(
        r"script Player { fn helper() -> f32 { return 1.0; } fn update() { this.helper(); } }",
    );
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|d| d.message.contains("call it as `helper(...)`")),
        "{:?}",
        analysis.diagnostics
    );

    let bare =
        analyze(r"script Player { fn helper() -> f32 { return 1.0; } fn update() { helper(); } }");
    assert!(bare.diagnostics.is_empty(), "{:?}", bare.diagnostics);
}
