//! References: a script naming an entity other than the one it runs on.
//!
//! Before this, a script could reach its own transform and leave numbers on a
//! shared board, and that was all. Two scripts could cooperate only by agreeing
//! on a name and passing floats through it — the companion game's collectibles
//! compared against a position the player published, because no collectible
//! could ask the player anything.
//!
//! What makes it work is that Decay holds a value it cannot look inside.
//! `Value::Reference` is an opaque number; the engine packs a slot and a
//! generation into it and nothing in the language knows that.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{PrefabSources, ScriptComponent, ScriptFailure, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// A world holding a named target and a scripted seeker.
fn world(script: &str) -> (World, EntityId, EntityId, ScriptSources) {
    let mut world = World::default();
    let target = world.spawn(EntityData {
        name: Some("Target".to_owned()),
        transform_3d: Some(Transform3D {
            position: [5.0, 6.0, 7.0],
            ..Transform3D::default()
        }),
        ..EntityData::default()
    });
    let seeker = world.spawn(EntityData {
        name: Some("Seeker".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "s.decay", "script": "S" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("s.decay", script);
    (world, target, seeker, sources)
}

fn run(world: &mut World, sources: &ScriptSources) -> Vec<ScriptFailure> {
    let mut scripts = Scripts::new();
    let report = scripts.advance(
        world,
        &registry(),
        sources,
        &PrefabSources::new(),
        &InputState::default(),
        1.0 / 60.0,
    );
    report.failures
}

fn position(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("the entity kept its transform")
        .position
}

/// The whole point: one script reads another entity's position.
#[test]
fn a_script_reads_another_entitys_transform() {
    let (mut world, _target, seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let other = World.find("Target");
                this.transform.position.x = other.transform.position.x;
                this.transform.position.y = other.transform.position.y;
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    let moved = position(&world, seeker);
    assert!(
        (moved[0] - 5.0).abs() < f32::EPSILON && (moved[1] - 6.0).abs() < f32::EPSILON,
        "the seeker should have copied the target's position, and is at {moved:?}"
    );
}

/// And writes to it, which is the half that makes a reference more than a
/// read-only lookup.
#[test]
fn a_script_writes_another_entitys_transform() {
    let (mut world, target, _seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let other = World.find("Target");
                other.transform.position.z = 42.0;
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    assert!(
        (position(&world, target)[2] - 42.0).abs() < f32::EPSILON,
        "the target should have been moved by a script it does not run"
    );
}

/// A script can name itself, and reaching through that reference is the same as
/// reaching directly.
#[test]
fn reaching_through_this_entity_is_reaching_directly() {
    let (mut world, _target, seeker, sources) = world(
        r"
        script S {
            fn update(dt: f32) {
                let me = this.entity;
                me.transform.position.x = 3.0;
                this.entity.transform.position.y = 4.0;
            }
        }
    ",
    );
    assert!(run(&mut world, &sources).is_empty());
    let moved = position(&world, seeker);
    assert!(
        (moved[0] - 3.0).abs() < f32::EPSILON && (moved[1] - 4.0).abs() < f32::EPSILON,
        "a script should reach itself through its own reference, and is at {moved:?}"
    );
}

/// References compare, which is how a script tells one entity from another.
#[test]
fn references_to_the_same_entity_are_equal() {
    let (mut world, _target, seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let a = World.find("Target");
                let b = World.find("Target");
                if a == b {
                    this.transform.position.x = 1.0;
                }
                if a == this.entity {
                    this.transform.position.y = 1.0;
                }
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    let moved = position(&world, seeker);
    assert!(
        (moved[0] - 1.0).abs() < f32::EPSILON,
        "two lookups of one entity should compare equal"
    );
    assert!(
        moved[1].abs() < f32::EPSILON,
        "two different entities should not compare equal"
    );
}

/// A name nothing answers to is `null`, and a script can say so.
#[test]
fn finding_nothing_is_null() {
    let (mut world, _target, seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let missing = World.find("Nobody");
                if missing == null {
                    this.transform.position.x = 9.0;
                }
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    assert!(
        (position(&world, seeker)[0] - 9.0).abs() < f32::EPSILON,
        "a lookup that matched nothing should be null"
    );
}

/// A script can remove another entity, and the world loses it and its subtree.
#[test]
fn a_script_despawns_another_entity() {
    let (mut world, target, _seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                World.despawn(World.find("Target"));
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    assert!(
        world.get(target).is_none(),
        "the target should be gone from the world"
    );
}

/// Despawning nothing is a no-op, because `despawn(find(...))` is a reasonable
/// thing to write and the lookup may find nothing.
#[test]
fn despawning_null_is_not_an_error() {
    let (mut world, _target, _seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                World.despawn(World.find("Nobody"));
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
}

/// The property generation checking exists for: a reference outlives what it
/// names, and asking is how a script finds out.
#[test]
fn a_reference_to_a_removed_entity_stops_existing() {
    let (mut world, _target, seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let doomed = World.find("Target");
                if World.exists(doomed) {
                    this.transform.position.x = 1.0;
                }
                World.despawn(doomed);
                if World.exists(doomed) {
                    this.transform.position.y = 1.0;
                }
            }
        }
    "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    let moved = position(&world, seeker);
    assert!(
        (moved[0] - 1.0).abs() < f32::EPSILON,
        "the reference should exist before the despawn"
    );
    assert!(moved[1].abs() < f32::EPSILON, "and not after it");
}

/// Reaching through a reference to something gone is an error naming the path,
/// not a silent no-op: a script holding a dead handle is a bug in the script.
#[test]
fn reaching_through_a_stale_reference_is_reported() {
    let (mut world, _target, _seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let doomed = World.find("Target");
                World.despawn(doomed);
                doomed.transform.position.x = 1.0;
            }
        }
    "#,
    );
    let failures = run(&mut world, &sources);
    assert!(
        failures
            .iter()
            .any(|failure| failure.to_string().contains("no longer exists")),
        "a stale reference should be reported, got {failures:?}"
    );
}

/// A misspelled member on a reference is a compile error with a line number,
/// the same as one on `this` — which is the whole argument for a typed host.
#[test]
fn a_misspelled_member_on_a_reference_does_not_compile() {
    let (mut world, _target, _seeker, sources) = world(
        r#"
        script S {
            fn update(dt: f32) {
                let other = World.find("Target");
                other.transfrom.position.x = 1.0;
            }
        }
    "#,
    );
    let failures = run(&mut world, &sources);
    assert!(
        !failures.is_empty(),
        "reaching a member that does not exist should not compile"
    );
}
