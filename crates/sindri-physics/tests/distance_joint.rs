use std::time::Duration;

use sindri_core::EntityId;
use sindri_physics::{Collider2d, PhysicsPose2d, PhysicsWorld2d, RigidBody2d, RigidBodyKind};

const STEP: Duration = Duration::from_millis(16);

fn entity(index: u32) -> EntityId {
    EntityId::from_bits(u64::from(index) << 32)
}

#[test]
fn a_distance_joint_keeps_a_dynamic_body_attached_to_a_moving_head() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).expect("physics world");
    let head = entity(1);
    let tail = entity(2);

    world
        .insert_body(
            head,
            RigidBody2d {
                kind: RigidBodyKind::KinematicVelocity,
                linear_velocity: [4.0, 0.0],
                gravity_scale: 0.0,
                ..RigidBody2d::default()
            },
            &[Collider2d::circle(0.2)],
        )
        .expect("head body");
    world
        .insert_body(
            tail,
            RigidBody2d {
                pose: PhysicsPose2d {
                    position: [-0.55, 0.0],
                    rotation: 0.0,
                },
                gravity_scale: 0.0,
                linear_damping: 1.0,
                ..RigidBody2d::default()
            },
            &[Collider2d::circle(0.2)],
        )
        .expect("tail body");

    world
        .connect_distance(head, tail, 0.65)
        .expect("distance joint");
    assert_eq!(world.joint_count(), 1);

    for _ in 0..60 {
        world.step(STEP).expect("jointed world steps");
    }

    let head_pose = world.pose(head).expect("head pose");
    let tail_pose = world.pose(tail).expect("tail pose");
    let dx = head_pose.position[0] - tail_pose.position[0];
    let dy = head_pose.position[1] - tail_pose.position[1];
    let distance = (dx * dx + dy * dy).sqrt();
    assert!(
        distance <= 0.68,
        "tail escaped the 0.65 joint and reached {distance}"
    );

    assert!(world.remove(head));
    assert_eq!(
        world.joint_count(),
        0,
        "removing an endpoint must remove its attached joint"
    );
}

#[test]
fn a_distance_joint_cannot_connect_a_body_to_itself() {
    let mut world = PhysicsWorld2d::new([0.0, 0.0]).expect("physics world");
    let body = entity(1);
    let error = world
        .connect_distance(body, body, 0.5)
        .expect_err("self-joint must be rejected before backend lookup");
    assert!(error.to_string().contains("to itself"));
}
