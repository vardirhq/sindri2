//! What a scene and the physics world have to agree about.

use std::time::Duration;

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_physics::{PhysicsEventKind, RigidBodyKind};

use crate::physics::{Collider2dComponent, RigidBody2dComponent};
use crate::{SceneExtractor, ScenePhysics2d};

const STEP: Duration = Duration::from_nanos(16_666_667);

fn components() -> ComponentSchemaRegistry {
    SceneExtractor::new()
        .expect("the builtin schemas register")
        .components()
        .clone()
}

fn circle(radius: f32, sensor: bool) -> serde_json::Value {
    json!({
        "shape": { "shape": "circle", "radius": radius },
        "offset": [0.0, 0.0],
        "rotation": 0.0,
        "sensor": sensor,
        "layers": { "memberships": 4_294_967_295_u32, "filter": 4_294_967_295_u32 },
        "friction": 0.5,
        "restitution": 0.0
    })
}

fn body(kind: &str) -> serde_json::Value {
    json!({
        "kind": kind,
        "pose": { "position": [0.0, 0.0], "rotation": 0.0 },
        "linear_velocity": [0.0, 0.0],
        "angular_velocity": 0.0,
        "gravity_scale": 1.0,
        "linear_damping": 0.0,
        "angular_damping": 0.0,
        "lock_rotation": false
    })
}

/// An entity with a collider, and optionally a body, at a position.
fn spawn(world: &mut World, at: [f32; 2], kind: Option<&str>, sensor: bool) -> EntityId {
    let mut entity = EntityData {
        transform_3d: Some(Transform3D {
            position: [at[0], at[1], 0.0],
            ..Transform3D::default()
        }),
        ..EntityData::default()
    };
    entity.components.insert(
        Collider2dComponent::TYPE_NAME.to_owned(),
        circle(1.0, sensor),
    );
    if let Some(kind) = kind {
        entity
            .components
            .insert(RigidBody2dComponent::TYPE_NAME.to_owned(), body(kind));
    }
    world.spawn(entity)
}

fn position(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .expect("still there")
        .transform_3d
        .expect("a transform")
        .position
}

/// The authored transform is where a body starts. A body's own pose field is
/// where physics writes its answer, not a second authored truth.
#[test]
fn a_body_starts_where_the_scene_put_it() {
    let mut world = World::default();
    let entity = spawn(&mut world, [3.0, 4.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world with no gravity");

    physics
        .step(&mut world, &components(), STEP)
        .expect("one fixed step");
    let pose = physics.world().pose(entity).expect("registered");
    assert!((pose.position[0] - 3.0).abs() < 1.0e-4, "{pose:?}");
    assert!((pose.position[1] - 4.0).abs() < 1.0e-4, "{pose:?}");
}

#[test]
fn a_moving_body_writes_its_position_back_into_the_transform() {
    let mut world = World::default();
    let entity = spawn(&mut world, [0.0, 0.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");

    physics
        .world_mut()
        .set_linear_velocity(entity, [60.0, 0.0])
        .expect("a dynamic body takes a velocity");
    for _ in 0..30 {
        physics.step(&mut world, &components(), STEP).expect("step");
    }
    assert!(
        position(&world, entity)[0] > 10.0,
        "{:?}",
        position(&world, entity)
    );
}

/// The Z position is the author's. `docs/2d-model.md`: a 2D entity keeps to a
/// plane rather than having a transform of its own kind.
#[test]
fn writing_a_position_back_leaves_the_authored_z_alone() {
    let mut world = World::default();
    let entity = spawn(&mut world, [0.0, 0.0], Some("dynamic"), false);
    world
        .get_mut(entity)
        .expect("just spawned")
        .transform_3d
        .as_mut()
        .expect("a transform")
        .position[2] = 7.0;
    let mut physics = ScenePhysics2d::top_down().expect("a world");

    physics.step(&mut world, &components(), STEP).expect("step");
    physics
        .world_mut()
        .set_linear_velocity(entity, [10.0, 0.0])
        .expect("velocity");
    physics.step(&mut world, &components(), STEP).expect("step");

    assert!((position(&world, entity)[2] - 7.0).abs() < 1.0e-5);
}

/// A static body's position is the author's, and writing it back would fight
/// whoever moved it.
#[test]
fn a_static_body_is_not_moved_by_physics() {
    let mut world = World::default();
    let entity = spawn(&mut world, [5.0, 5.0], None, false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    for _ in 0..10 {
        physics.step(&mut world, &components(), STEP).expect("step");
    }
    let at = position(&world, entity);
    assert!(
        (at[0] - 5.0).abs() < 1.0e-5 && (at[1] - 5.0).abs() < 1.0e-5,
        "{at:?}"
    );
}

/// Two sensors overlapping is the event a game is actually built on: a bullet
/// reaching an enemy.
#[test]
fn two_overlapping_sensors_report_entering_and_leaving() {
    let mut world = World::default();
    let mover = spawn(&mut world, [-5.0, 0.0], Some("kinematic_velocity"), true);
    let target = spawn(&mut world, [0.0, 0.0], None, true);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");
    physics
        .world_mut()
        .set_linear_velocity(mover, [120.0, 0.0])
        .expect("velocity");

    let mut entered = false;
    let mut exited = false;
    for _ in 0..120 {
        physics.step(&mut world, &components(), STEP).expect("step");
        for event in physics.events() {
            let pair = [event.first, event.second];
            if !pair.contains(&mover) || !pair.contains(&target) {
                continue;
            }
            match event.kind {
                PhysicsEventKind::SensorEntered => entered = true,
                PhysicsEventKind::SensorExited => exited = true,
                _ => {}
            }
        }
    }
    assert!(entered, "the mover never reached the target");
    assert!(exited, "the mover never left it");
}

/// A body outliving its entity would collide with things on behalf of nothing.
#[test]
fn a_despawned_entity_leaves_the_physics_world() {
    let mut world = World::default();
    let entity = spawn(&mut world, [0.0, 0.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");
    assert!(physics.world().contains(entity));

    world.despawn_recursive(entity).expect("it is there");
    physics.step(&mut world, &components(), STEP).expect("step");
    assert!(!physics.world().contains(entity));
}

/// An entity switched off takes no part in the scene, and colliding is part of
/// the scene.
#[test]
fn a_switched_off_entity_leaves_the_physics_world() {
    let mut world = World::default();
    let entity = spawn(&mut world, [0.0, 0.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");

    world.get_mut(entity).expect("there").disabled = true;
    physics.step(&mut world, &components(), STEP).expect("step");
    assert!(!physics.world().contains(entity));
}

/// Rebuilding a body every step would throw away the velocity the simulation
/// just computed, which is the whole state physics owns.
#[test]
fn an_unchanged_body_keeps_the_velocity_physics_gave_it() {
    let mut world = World::default();
    let entity = spawn(&mut world, [0.0, 0.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");

    physics
        .world_mut()
        .set_linear_velocity(entity, [42.0, 0.0])
        .expect("velocity");
    physics.step(&mut world, &components(), STEP).expect("step");

    let velocity = physics.world().linear_velocity(entity).expect("registered");
    assert!(velocity[0] > 40.0, "the body was rebuilt: {velocity:?}");
}

/// A body spawned mid-run is one the physics world has never seen, which is
/// every bullet in a game that makes them as it goes.
#[test]
fn an_entity_spawned_after_the_first_step_joins_the_physics_world() {
    let mut world = World::default();
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");

    let late = spawn(&mut world, [1.0, 1.0], Some("dynamic"), false);
    physics.step(&mut world, &components(), STEP).expect("step");
    assert!(physics.world().contains(late));
}

#[test]
fn a_step_of_no_time_is_refused() {
    let mut world = World::default();
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    assert!(
        physics
            .step(&mut world, &components(), Duration::ZERO)
            .is_err()
    );
}

#[test]
fn a_body_kind_is_read_from_the_scene() {
    let mut world = World::default();
    let entity = spawn(&mut world, [0.0, 0.0], Some("kinematic_velocity"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");
    assert_eq!(
        physics.world().body_kind(entity).expect("registered"),
        RigidBodyKind::KinematicVelocity
    );
}

/// A level painted as a tilemap, with a tilemap collider: a floor of `#`s
/// four tiles wide, its top edge at y = -1 (the second row), with the map's
/// corner at `corner`.
fn spawn_level(world: &mut World, corner: [f32; 2]) -> EntityId {
    let mut entity = EntityData {
        transform_3d: Some(Transform3D {
            position: [corner[0], corner[1], 0.0],
            ..Transform3D::default()
        }),
        ..EntityData::default()
    };
    entity.components.insert(
        crate::TilemapComponent::TYPE_NAME.to_owned(),
        json!({
            "texture": "tiles.png",
            "palette": ["ground"],
            "columns": 4,
            "rows": 2,
            "tiles": [null, null, null, null, 0, 0, 0, 0]
        }),
    );
    entity.components.insert(
        crate::TilemapCollider2dComponent::TYPE_NAME.to_owned(),
        json!({}),
    );
    world.spawn(entity)
}

/// The point of a tilemap collider: a thing dropped on a painted floor lands
/// on it, with no collider entity placed over the tiles by hand.
#[test]
fn a_body_lands_on_a_painted_floor() {
    let mut world = World::default();
    spawn_level(&mut world, [-2.0, 0.0]);
    // A ball of radius 1 dropped from above the middle of the floor.
    let ball = spawn(&mut world, [0.0, 4.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::new([0.0, -20.0]).expect("a world with gravity");
    for _ in 0..180 {
        physics.step(&mut world, &components(), STEP).expect("step");
    }
    let resting = position(&world, ball)[1];
    // The floor's top is at y = -1, so the ball rests with its centre at 0.
    assert!(
        resting.abs() < 0.05,
        "the ball rests on the floor: {resting}"
    );
}

/// A tilemap with nothing solid painted yet is not a collider with no shape.
#[test]
fn an_unpainted_level_is_not_an_error() {
    let mut world = World::default();
    let level = spawn_level(&mut world, [0.0, 0.0]);
    world
        .get_mut(level)
        .expect("just spawned")
        .components
        .get_mut(crate::TilemapComponent::TYPE_NAME)
        .expect("a tilemap")["tiles"] = json!([null, null, null, null, null, null, null, null]);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    physics.step(&mut world, &components(), STEP).expect("step");
    assert!(physics.world().pose(level).is_err(), "nothing registered");
}

/// A scene says which way is down; the host's gravity is only for a scene
/// that says nothing.
#[test]
fn a_scene_sets_its_own_gravity() {
    let mut world = World::default();
    let mut settings = EntityData::default();
    settings.components.insert(
        crate::PhysicsWorld2dComponent::TYPE_NAME.to_owned(),
        json!({ "gravity": [0.0, -10.0] }),
    );
    let settings = world.spawn(settings);
    let ball = spawn(&mut world, [0.0, 0.0], Some("dynamic"), false);
    let mut physics = ScenePhysics2d::top_down().expect("a world");
    for _ in 0..60 {
        physics.step(&mut world, &components(), STEP).expect("step");
    }
    assert!(
        position(&world, ball)[1] < -3.0,
        "it fell under the scene's gravity"
    );

    world.despawn_recursive(settings).expect("despawned");
    physics.step(&mut world, &components(), STEP).expect("step");
    assert!(
        physics.world().gravity() == [0.0, 0.0],
        "back to the host's"
    );
}
