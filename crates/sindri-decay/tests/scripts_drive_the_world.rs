//! A Decay script moving a real entity's transform.
//!
//! This is the binding's whole claim, so it is tested at the seam rather than
//! either side of it: a world with a `sindri.script` component, a source, and a
//! frame — then the entity's transform is somewhere new. Nothing here touches a
//! GPU, a window, or a file, which is the point of the crate doing no I/O.

use serde_json::{Value, json};
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFailure, ScriptSources, Scripts};

const SPIN: &str = r"
script Spin {
    @export
    let turns_per_second: f32 = 1.0;

    var elapsed: f32 = 0.0;

    fn update(dt: f32) {
        elapsed += dt;
        this.transform.rotation_z = elapsed * turns_per_second;
    }
}
";

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// One entity with a transform and a script on it.
///
/// The component is authored as the JSON a scene would hold rather than built
/// from the view type: `ScriptComponent` is `Deserialize` only, on the same
/// rule every built-in component follows, so that adding a field the engine
/// reads can never change what a scene writes back.
fn world_with(script: Value) -> (World, EntityId) {
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(ScriptComponent::TYPE_NAME.to_owned(), script)]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    (world, entity)
}

fn component(properties: &Value) -> Value {
    json!({
        "source": "scripts/spin.decay",
        "script": "Spin",
        "properties": properties.clone(),
    })
}

fn sources() -> ScriptSources {
    let mut sources = ScriptSources::new();
    sources.insert("scripts/spin.decay", SPIN);
    sources
}

fn rotation(world: &World, entity: EntityId) -> f32 {
    world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("the entity kept its transform")
        .rotation_z_radians()
}

/// The claim: a script writes to the world, and the world keeps it.
#[test]
fn a_script_moves_the_entity_it_is_attached_to() {
    let (mut world, entity) = world_with(component(&json!({})));
    let mut scripts = Scripts::new();

    let failures = scripts.advance(&mut world, &registry(), &sources(), 0.25);
    assert!(failures.is_empty(), "{failures:?}");
    assert!((rotation(&world, entity) - 0.25).abs() < 1.0e-5);

    // State survives between frames, which is what a script instance is for:
    // `elapsed` accumulates rather than restarting.
    scripts.advance(&mut world, &registry(), &sources(), 0.25);
    assert!((rotation(&world, entity) - 0.5).abs() < 1.0e-5);
}

/// The argument for a typed language over a dynamic one: the scene authors a
/// declared, exported field, and the script sees it before its first line runs.
#[test]
fn an_authored_property_reaches_the_script() {
    let (mut world, entity) = world_with(component(&json!({ "turns_per_second": 4.0 })));
    let mut scripts = Scripts::new();

    let failures = scripts.advance(&mut world, &registry(), &sources(), 0.25);
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        (rotation(&world, entity) - 1.0).abs() < 1.0e-5,
        "a quarter second at four turns a second, not the script's default of one"
    );
}

/// A property that goes nowhere is the bug this component exists to prevent, so
/// every way of writing one is refused rather than ignored.
#[test]
fn a_property_that_would_go_nowhere_is_refused() {
    for (properties, expected) in [
        (json!({ "turns_per_secnod": 4.0 }), "no such field"),
        (json!({ "elapsed": 4.0 }), "not @export"),
    ] {
        let (mut world, _) = world_with(component(&properties));
        let failures = Scripts::new().advance(&mut world, &registry(), &sources(), 0.25);
        let reported = format!("{failures:?}");
        assert!(
            reported.contains(expected),
            "{properties} should have been refused for `{expected}`, and said {reported}"
        );
    }
}

/// One broken script must not silence the others: in the editor that would mean
/// a typo in one object freezing every other, and the author looking for the
/// wrong bug entirely.
#[test]
fn a_failing_script_does_not_stop_the_rest() {
    let (mut world, working) = world_with(component(&json!({})));
    world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "scripts/missing.decay", "script": "Nope" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });

    let failures = Scripts::new().advance(&mut world, &registry(), &sources(), 0.25);

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(matches!(failures[0], ScriptFailure::MissingSource { .. }));
    assert!(
        (rotation(&world, working) - 0.25).abs() < 1.0e-5,
        "the working script still ran"
    );
}

/// A source that does not compile reports where, rather than failing silently
/// or panicking somewhere inside the language.
#[test]
fn a_broken_source_reports_its_diagnostics() {
    let (mut world, entity) = world_with(component(&json!({})));
    let mut sources = ScriptSources::new();
    sources.insert("scripts/spin.decay", "script Spin { fn update(dt: f32) { ");

    let failures = Scripts::new().advance(&mut world, &registry(), &sources, 0.25);

    match failures.as_slice() {
        [ScriptFailure::Compile { asset, diagnostics }] => {
            assert_eq!(asset, "scripts/spin.decay");
            assert!(!diagnostics.is_empty());
        }
        other => panic!("expected one compile failure, got {other:?}"),
    }
    assert!(
        rotation(&world, entity).abs() < f32::EPSILON,
        "and nothing moved on a script that never ran"
    );
}

/// Replacing the source runs the new program on the next frame. This is all hot
/// reload needs from this side.
#[test]
fn replacing_a_source_recompiles_it() {
    let (mut world, entity) = world_with(component(&json!({})));
    let mut scripts = Scripts::new();
    let mut sources = sources();

    scripts.advance(&mut world, &registry(), &sources, 1.0);
    assert!((rotation(&world, entity) - 1.0).abs() < 1.0e-5);

    sources.insert(
        "scripts/spin.decay",
        r"script Spin { fn update(dt: f32) { this.transform.rotation_z = 0.0; } }",
    );
    let failures = scripts.advance(&mut world, &registry(), &sources, 1.0);
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        rotation(&world, entity).abs() < f32::EPSILON,
        "the new program is running"
    );
}

/// A disabled script is still authored and still saved; it simply does not
/// tick, which is what an author wants while narrowing down a misbehaviour.
#[test]
fn a_disabled_script_does_not_run() {
    let mut disabled = component(&json!({}));
    disabled["enabled"] = json!(false);
    let (mut world, entity) = world_with(disabled);

    let failures = Scripts::new().advance(&mut world, &registry(), &sources(), 0.25);

    assert!(failures.is_empty(), "{failures:?}");
    assert!(rotation(&world, entity).abs() < f32::EPSILON);
}
