//! What the 2D world does with what it is given.

use std::time::Duration;

use sindri_core::EntityId;

use super::*;

const STEP: Duration = Duration::from_millis(16);

fn entity(index: u32) -> EntityId {
    EntityId::from_bits(u64::from(index) << 32)
}

#[test]
fn dynamic_body_is_advanced_by_the_fixed_step() {
    let mut world = PhysicsWorld2d::new([0.0, -9.81]).unwrap();
    let falling = entity(1);
    world
        .insert_body(falling, RigidBody2d::default(), &[Collider2d::circle(0.5)])
        .unwrap();

    let before = world.pose(falling).unwrap();
    world.step(STEP).unwrap();
    let after = world.pose(falling).unwrap();

    assert!(after.position[1] < before.position[1]);
    assert!(world.linear_velocity(falling).unwrap()[1] < 0.0);
}

#[test]
fn sensor_events_contain_only_sindri_entities() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let player = entity(1);
    let pickup = entity(2);
    world
        .insert_body(player, RigidBody2d::default(), &[Collider2d::circle(0.5)])
        .unwrap();
    let mut sensor = Collider2d::circle(1.0);
    sensor.sensor = true;
    world
        .insert_static_collider(pickup, PhysicsPose2d::default(), &[sensor])
        .unwrap();

    let events = world.step(STEP).unwrap();
    assert!(events.contains(&PhysicsEvent2d {
        first: player,
        second: pickup,
        kind: PhysicsEventKind::SensorEntered,
    }));
}

#[test]
fn collision_layers_filter_pairs_before_the_public_event_surface() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let first = entity(1);
    let second = entity(2);
    let mut left = Collider2d::circle(1.0);
    left.layers = CollisionLayers::new(1, 1);
    let mut right = Collider2d::circle(1.0);
    right.layers = CollisionLayers::new(2, 2);
    world
        .insert_body(first, RigidBody2d::default(), &[left])
        .unwrap();
    world
        .insert_static_collider(second, PhysicsPose2d::default(), &[right])
        .unwrap();

    assert!(world.step(STEP).unwrap().is_empty());
}

#[test]
fn a_removed_entity_cannot_leave_a_reused_physics_record() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let old = EntityId::from_bits(7_u64 << 32);
    let reused = EntityId::from_bits((7_u64 << 32) | 1);
    world
        .insert_body(old, RigidBody2d::default(), &[Collider2d::circle(0.5)])
        .unwrap();
    assert!(world.remove(old));
    assert!(!world.contains(old));
    world
        .insert_body(reused, RigidBody2d::default(), &[Collider2d::circle(0.5)])
        .unwrap();
    assert!(world.contains(reused));
    assert!(!world.contains(old));
}

#[test]
fn invalid_dimensions_are_rejected_before_reaching_the_backend() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let bad = Collider2d::rectangle([0.0, 1.0]);
    // Reported as a piece even when there is only one, because a collider is a
    // list now. Saying "piece 0" for a lone collider is mild noise; reporting
    // the same fault differently depending on how many siblings it has would
    // be worse.
    assert_eq!(
        world.insert_body(entity(1), RigidBody2d::default(), &[bad]),
        Err(PhysicsError::ColliderPiece {
            index: 0,
            reason: Box::new(PhysicsError::NonPositive("box_half_extent_x")),
        })
    );
}

#[test]
fn dynamic_only_operations_are_checked_at_the_sindri_boundary() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let wall = entity(1);
    world
        .insert_static_collider(
            wall,
            PhysicsPose2d::default(),
            &[Collider2d::rectangle([1.0, 1.0])],
        )
        .unwrap();
    assert_eq!(
        world.apply_impulse(wall, [1.0, 0.0]),
        Err(PhysicsError::WrongBodyKind(
            wall,
            "apply impulse",
            RigidBodyKind::Static,
        ))
    );
}

#[test]
fn the_3d_contract_is_sindri_owned_even_before_the_3d_runtime_slice() {
    let body = RigidBody3d::default();
    let collider = Collider3d {
        shape: ColliderShape3d::Sphere { radius: 0.5 },
        offset: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        sensor: false,
        layers: CollisionLayers::ALL,
        friction: 0.5,
        restitution: 0.0,
    };
    let json = serde_json::to_string(&(body, collider)).unwrap();
    assert!(json.contains("sphere"));
    assert!(!json.contains("rapier"));
}

#[test]
fn rapier3d_compiles_behind_the_private_boundary() {
    let _backend = rapier3d::prelude::PhysicsWorld::new();
}

/// A compound moves as one object: its pieces never collide with each other,
/// and they all arrive where the body is.
///
/// This is the property that makes a compound *one* collider rather than
/// several, and it is why pieces need no child entities — each already carries
/// its own offset.
#[test]
fn a_compound_falls_as_one_body() {
    let mut world = PhysicsWorld2d::new([0.0, -9.81]).unwrap();
    let ship = entity(1);
    let pod = |x: f32| Collider2d {
        offset: [x, 0.0],
        ..Collider2d::circle(0.25)
    };
    world
        .insert_body(
            ship,
            RigidBody2d::default(),
            &[Collider2d::rectangle([0.5, 0.2]), pod(-0.6), pod(0.6)],
        )
        .expect("a compound of three pieces registers");

    for _ in 0..30 {
        world.step(STEP).expect("the world steps");
    }

    let pose = world.pose(ship).expect("the ship has a pose");
    assert!(
        pose.position[1] < -0.05,
        "the compound did not fall: {pose:?}"
    );
    // Pieces pushing each other apart would show up here long before it showed
    // up as a wrong height.
    assert!(
        pose.position[0].abs() < 1.0e-3,
        "the pieces pushed the body sideways, to {}",
        pose.position[0]
    );
}

/// Mass comes from every piece, not just the first.
///
/// Worth pinning because it is the change most likely to alter behaviour
/// quietly: a body that used to weigh one box now weighs the whole compound,
/// and a centre of mass that moved is a body that falls over differently.
#[test]
fn a_compound_weighs_all_of_its_pieces() {
    let heavier = |pieces: &[Collider2d]| {
        let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
        let body = entity(1);
        world
            .insert_body(body, RigidBody2d::default(), pieces)
            .expect("it registers");
        world.mass(body).expect("a dynamic body has a mass")
    };

    let one = heavier(&[Collider2d::rectangle([0.5, 0.5])]);
    let two = heavier(&[
        Collider2d::rectangle([0.5, 0.5]),
        Collider2d {
            offset: [2.0, 0.0],
            ..Collider2d::rectangle([0.5, 0.5])
        },
    ]);
    assert!(
        two > one * 1.9,
        "two equal pieces weigh {two}, one weighs {one}"
    );
}

/// A bad piece names itself, and leaves the world as it was.
///
/// A compound is authored as a list, so "restitution must be between 0 and 1"
/// without an index is a needle in it. Nothing is inserted either, because a
/// half-built body is worse than none.
#[test]
fn a_bad_piece_is_named_by_its_index_and_nothing_is_built() {
    let mut world = PhysicsWorld2d::new([0.0, -9.81]).unwrap();
    let entity = entity(1);
    let error = world
        .insert_body(
            entity,
            RigidBody2d::default(),
            &[
                Collider2d::circle(0.5),
                Collider2d::circle(0.5),
                Collider2d::circle(-1.0),
            ],
        )
        .expect_err("a negative radius is refused");

    let PhysicsError::ColliderPiece { index, .. } = error else {
        panic!("expected the failing piece to be named, got {error:?}");
    };
    assert_eq!(index, 2);
    assert!(!world.contains(entity), "the refused body was still built");
}

/// A collider with no pieces is refused rather than silently making a body that
/// nothing can touch.
#[test]
fn a_collider_with_no_pieces_is_refused() {
    let mut world = PhysicsWorld2d::new([0.0, -9.81]).unwrap();
    assert!(matches!(
        world.insert_body(entity(1), RigidBody2d::default(), &[]),
        Err(PhysicsError::NoColliderPieces(_))
    ));
}

/// The names beside the enum are the names serde uses.
///
/// A list of spellings next to a type is a second copy of it, and a second copy
/// drifts. This is the check that makes the copy safe to rely on.
#[test]
fn every_collider_shape_name_is_one_serde_reads() {
    for name in ColliderShape2d::SHAPES {
        let payload = match name {
            "box" => serde_json::json!({ "shape": name, "half_extents": [0.5, 0.5] }),
            "circle" => serde_json::json!({ "shape": name, "radius": 0.5 }),
            "capsule" => serde_json::json!({ "shape": name, "half_height": 0.4, "radius": 0.2 }),
            other => panic!("{other} has no case here, so the list has grown"),
        };
        serde_json::from_value::<ColliderShape2d>(payload)
            .unwrap_or_else(|error| panic!("'{name}' is not a shape serde reads: {error}"));
    }
}
