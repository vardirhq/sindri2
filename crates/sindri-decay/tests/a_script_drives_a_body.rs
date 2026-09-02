//! Physics: a script moving a body and reacting to what it touched.
//!
//! The audit's finding was that Rapier ran and no game could reach it — masks,
//! shapes and events all existed, with no Decay access and nothing exercising
//! them. These are the two halves of closing that: a script drives a body, and
//! a script is told what that body reached.
//!
//! The physics world here is driven by hand rather than by `ScenePhysics2d`,
//! because this crate must not depend on the scene-side driver to give a script
//! physics — a host with its own driver gets the same surface.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{Physics2d, ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_physics::{
    Collider2d, PhysicsEvent2d, PhysicsEventKind, PhysicsWorld2d, RigidBody2d, RigidBodyKind,
};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn scripted(world: &mut World, name: &str, script: &str) -> EntityId {
    world.spawn(EntityData {
        name: Some(name.to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "body.decay", "script": script }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

/// Runs one pass with the given physics world and events.
fn advance(
    scripts: &mut Scripts,
    world: &mut World,
    sources: &ScriptSources,
    physics: &mut PhysicsWorld2d,
    events: &[PhysicsEvent2d],
) -> ScriptReport {
    let input = InputState::default();
    scripts.advance(
        world,
        &registry(),
        ScriptFrame::new(sources, &input, 1.0 / 60.0).with_physics(Physics2d {
            world: physics,
            events,
        }),
    )
}

fn sources(script: &str) -> ScriptSources {
    let mut sources = ScriptSources::new();
    sources.insert("body.decay", script);
    sources
}

#[test]
fn a_script_sets_a_bodys_velocity() {
    let script = r"
    script Bullet {
        fn start() {
            Physics.set_velocity(this.entity, 400.0, -50.0);
        }
    }
    ";
    let mut world = World::default();
    let entity = scripted(&mut world, "Bullet", "Bullet");
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");
    physics
        .insert_body(entity, RigidBody2d::default(), Collider2d::circle(1.0))
        .expect("a body");

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &[],
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let velocity = physics.linear_velocity(entity).expect("a body");
    assert!((velocity[0] - 400.0).abs() < 1.0e-3, "{velocity:?}");
    assert!((velocity[1] + 50.0).abs() < 1.0e-3, "{velocity:?}");
}

#[test]
fn a_script_reads_a_bodys_velocity_back() {
    let script = r"
    script Bullet {
        fn update(dt: f32) {
            this.transform.position.x = Physics.velocity_x(this.entity);
        }
    }
    ";
    let mut world = World::default();
    let entity = scripted(&mut world, "Bullet", "Bullet");
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");
    physics
        .insert_body(entity, RigidBody2d::default(), Collider2d::circle(1.0))
        .expect("a body");
    physics
        .set_linear_velocity(entity, [12.0, 0.0])
        .expect("a velocity");

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &[],
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let at = world
        .get(entity)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position;
    assert!((at[0] - 12.0).abs() < 1.0e-3, "{at:?}");
}

#[test]
fn an_impulse_moves_a_body_that_was_still() {
    let script = r"
    script Bullet {
        fn start() { Physics.apply_impulse(this.entity, 5.0, 0.0); }
    }
    ";
    let mut world = World::default();
    let entity = scripted(&mut world, "Bullet", "Bullet");
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");
    physics
        .insert_body(entity, RigidBody2d::default(), Collider2d::circle(1.0))
        .expect("a body");

    advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &[],
    );
    assert!(
        physics.linear_velocity(entity).expect("a body")[0] > 0.0,
        "an impulse did nothing"
    );
}

/// An event is about a pair, and the pair a script cares about is the one it is
/// half of — so the answer names the other half, whichever side of the event
/// this entity was on.
#[test]
fn a_script_is_told_what_it_touched_from_either_side_of_the_event() {
    let script = r"
    script Bullet {
        fn update(dt: f32) {
            for hit in Physics.sensor_entered() {
                hit.transform.position.z = 1.0;
            }
        }
    }
    ";
    for swap in [false, true] {
        let mut world = World::default();
        let bullet = scripted(&mut world, "Bullet", "Bullet");
        let target = world.spawn(EntityData {
            name: Some("Target".to_owned()),
            transform_3d: Some(Transform3D::default()),
            ..EntityData::default()
        });
        let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");
        let event = PhysicsEvent2d {
            first: if swap { target } else { bullet },
            second: if swap { bullet } else { target },
            kind: PhysicsEventKind::SensorEntered,
        };

        let report = advance(
            &mut Scripts::new(),
            &mut world,
            &sources(script),
            &mut physics,
            &[event],
        );
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        let marked = world
            .get(target)
            .expect("there")
            .transform_3d
            .expect("a transform")
            .position[2];
        assert!((marked - 1.0).abs() < 1.0e-5, "swap={swap}");
    }
}

/// Four kinds, and a script asking for one must not be handed another. A sensor
/// registers a touch and does not push back, which is a different thing from a
/// collision and is why they are separate questions.
#[test]
fn each_kind_of_event_answers_only_its_own() {
    let script = r"
    script Bullet {
        fn update(dt: f32) {
            this.transform.position.x = Physics.collision_started().len;
            this.transform.position.y = Physics.sensor_entered().len;
            this.transform.position.z = Physics.collision_stopped().len;
        }
    }
    ";
    let mut world = World::default();
    let bullet = scripted(&mut world, "Bullet", "Bullet");
    let a = world.spawn(EntityData::default());
    let b = world.spawn(EntityData::default());
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");
    let events = [
        PhysicsEvent2d {
            first: bullet,
            second: a,
            kind: PhysicsEventKind::SensorEntered,
        },
        PhysicsEvent2d {
            first: bullet,
            second: b,
            kind: PhysicsEventKind::SensorEntered,
        },
        PhysicsEvent2d {
            first: bullet,
            second: a,
            kind: PhysicsEventKind::CollisionStarted,
        },
    ];

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &events,
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let counts = world
        .get(bullet)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position;
    assert!((counts[0] - 1.0).abs() < 1.0e-5, "collisions: {counts:?}");
    assert!((counts[1] - 2.0).abs() < 1.0e-5, "sensors: {counts:?}");
    assert!(counts[2].abs() < 1.0e-5, "stopped: {counts:?}");
}

/// An event about two other entities is not this script's business.
#[test]
fn an_event_this_entity_is_not_part_of_is_not_reported_to_it() {
    let script = r"
    script Bullet {
        fn update(dt: f32) {
            this.transform.position.x = Physics.sensor_entered().len;
        }
    }
    ";
    let mut world = World::default();
    let bullet = scripted(&mut world, "Bullet", "Bullet");
    let a = world.spawn(EntityData::default());
    let b = world.spawn(EntityData::default());
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");

    advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &[PhysicsEvent2d {
            first: a,
            second: b,
            kind: PhysicsEventKind::SensorEntered,
        }],
    );
    let at = world
        .get(bullet)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position;
    assert!(at[0].abs() < 1.0e-5, "{at:?}");
}

/// Removing what was hit, and the thing that hit it, from inside the answer.
/// This is what "safe destruction from an event" means in practice.
#[test]
fn a_script_can_despawn_both_halves_from_inside_an_event() {
    let script = r"
    script Bullet {
        fn update(dt: f32) {
            for hit in Physics.sensor_entered() {
                World.despawn(hit);
                World.despawn(this.entity);
            }
        }
    }
    ";
    let mut world = World::default();
    let bullet = scripted(&mut world, "Bullet", "Bullet");
    let target = world.spawn(EntityData::default());
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &[PhysicsEvent2d {
            first: bullet,
            second: target,
            kind: PhysicsEventKind::SensorEntered,
        }],
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(world.get(target).is_none(), "the target survived");
    assert!(world.get(bullet).is_none(), "the bullet survived");
}

/// A game whose bullets never move because nothing is stepping should hear
/// about it on the first frame.
#[test]
fn a_host_with_no_physics_says_so_rather_than_answering_zero() {
    let script = r"
    script Bullet {
        fn start() { Physics.set_velocity(this.entity, 1.0, 0.0); }
    }
    ";
    let mut world = World::default();
    scripted(&mut world, "Bullet", "Bullet");
    let input = InputState::default();

    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources(script), &input, 1.0 / 60.0),
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("not running any"), "{message}");
}

/// A script holding a reference to something with a sprite and no collider.
#[test]
fn an_entity_with_no_body_is_named_rather_than_silently_ignored() {
    let script = r"
    script Bullet {
        fn start() { Physics.set_velocity(this.entity, 1.0, 0.0); }
    }
    ";
    let mut world = World::default();
    scripted(&mut world, "Bullet", "Bullet");
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut physics,
        &[],
    );
    assert!(!report.failures.is_empty(), "a body that is not there");
}

/// Every script in a pass sees the same frame's events and drives the same
/// world. One taking physics away from the rest would make which script ran
/// first decide what the others could do.
#[test]
fn every_script_in_a_pass_reaches_the_same_physics() {
    let mut world = World::default();
    let first = scripted(&mut world, "First", "Bullet");
    let second = scripted(&mut world, "Second", "Bullet");
    let mut physics = PhysicsWorld2d::new([0.0, 0.0]).expect("a world");
    for entity in [first, second] {
        physics
            .insert_body(
                entity,
                RigidBody2d {
                    kind: RigidBodyKind::Dynamic,
                    ..RigidBody2d::default()
                },
                Collider2d::circle(1.0),
            )
            .expect("a body");
    }

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources(
            r"
        script Bullet {
            fn start() { Physics.set_velocity(this.entity, 7.0, 0.0); }
        }
        ",
        ),
        &mut physics,
        &[],
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    for entity in [first, second] {
        assert!(
            (physics.linear_velocity(entity).expect("a body")[0] - 7.0).abs() < 1.0e-3,
            "one script missed physics"
        );
    }
}
