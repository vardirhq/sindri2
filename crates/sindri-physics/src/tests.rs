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
        .insert_body(falling, RigidBody2d::default(), Collider2d::circle(0.5))
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
        .insert_body(player, RigidBody2d::default(), Collider2d::circle(0.5))
        .unwrap();
    let mut sensor = Collider2d::circle(1.0);
    sensor.sensor = true;
    world
        .insert_static_collider(pickup, PhysicsPose2d::default(), sensor)
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
        .insert_body(first, RigidBody2d::default(), left)
        .unwrap();
    world
        .insert_static_collider(second, PhysicsPose2d::default(), right)
        .unwrap();

    assert!(world.step(STEP).unwrap().is_empty());
}

#[test]
fn a_removed_entity_cannot_leave_a_reused_physics_record() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let old = EntityId::from_bits(7_u64 << 32);
    let reused = EntityId::from_bits((7_u64 << 32) | 1);
    world
        .insert_body(old, RigidBody2d::default(), Collider2d::circle(0.5))
        .unwrap();
    assert!(world.remove(old));
    assert!(!world.contains(old));
    world
        .insert_body(reused, RigidBody2d::default(), Collider2d::circle(0.5))
        .unwrap();
    assert!(world.contains(reused));
    assert!(!world.contains(old));
}

#[test]
fn invalid_dimensions_are_rejected_before_reaching_the_backend() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
    let bad = Collider2d::rectangle([0.0, 1.0]);
    assert_eq!(
        world.insert_body(entity(1), RigidBody2d::default(), bad),
        Err(PhysicsError::NonPositive("box_half_extent_x"))
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
            Collider2d::rectangle([1.0, 1.0]),
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
