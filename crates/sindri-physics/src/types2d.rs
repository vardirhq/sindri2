//! The 2D data model: bodies, colliders, and where they are.

use serde::{Deserialize, Serialize};
use sindri_core::EntityId;

use crate::shared::{CollisionLayers, RigidBodyKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose2d {
    pub position: [f32; 2],
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigidBody2d {
    pub kind: RigidBodyKind,
    pub pose: PhysicsPose2d,
    pub linear_velocity: [f32; 2],
    pub angular_velocity: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub lock_rotation: bool,
}

impl Default for RigidBody2d {
    fn default() -> Self {
        Self {
            kind: RigidBodyKind::Dynamic,
            pose: PhysicsPose2d::default(),
            linear_velocity: [0.0; 2],
            angular_velocity: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            lock_rotation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum ColliderShape2d {
    Box { half_extents: [f32; 2] },
    Circle { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider2d {
    pub shape: ColliderShape2d,
    pub offset: [f32; 2],
    pub rotation: f32,
    pub sensor: bool,
    pub layers: CollisionLayers,
    pub friction: f32,
    pub restitution: f32,
}

impl Collider2d {
    pub const fn circle(radius: f32) -> Self {
        Self {
            shape: ColliderShape2d::Circle { radius },
            offset: [0.0; 2],
            rotation: 0.0,
            sensor: false,
            layers: CollisionLayers::ALL,
            friction: 0.5,
            restitution: 0.0,
        }
    }

    pub const fn rectangle(half_extents: [f32; 2]) -> Self {
        Self {
            shape: ColliderShape2d::Box { half_extents },
            offset: [0.0; 2],
            rotation: 0.0,
            sensor: false,
            layers: CollisionLayers::ALL,
            friction: 0.5,
            restitution: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsEventKind {
    CollisionStarted,
    CollisionStopped,
    SensorEntered,
    SensorExited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicsEvent2d {
    pub first: EntityId,
    pub second: EntityId,
    pub kind: PhysicsEventKind,
}
