//! The parallel 3D data model.
//!
//! It exists ahead of a 3D runtime so the 2D API cannot quietly become
//! the dimension-neutral contract.

use serde::{Deserialize, Serialize};

use crate::shared::{CollisionLayers, RigidBodyKind};

/// The 3D body model is deliberately parallel to 2D, but no 3D runtime is
/// claimed by this foundation slice.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigidBody3d {
    pub kind: RigidBodyKind,
    pub position: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub lock_rotation: bool,
}

impl Default for RigidBody3d {
    fn default() -> Self {
        Self {
            kind: RigidBodyKind::Dynamic,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            lock_rotation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum ColliderShape3d {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider3d {
    pub shape: ColliderShape3d,
    pub offset: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub sensor: bool,
    pub layers: CollisionLayers,
    pub friction: f32,
    pub restitution: f32,
}
