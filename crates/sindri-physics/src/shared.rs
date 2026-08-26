//! What both dimensions agree on: what drives a body, and what it
//! collides with.

use serde::{Deserialize, Serialize};

/// What drives a rigid body's motion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RigidBodyKind {
    Static,
    #[default]
    Dynamic,
    KinematicPosition,
    KinematicVelocity,
}

/// A 32-bit collision membership/filter mask owned by Sindri.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CollisionLayers {
    pub memberships: u32,
    pub filter: u32,
}

impl CollisionLayers {
    pub const ALL: Self = Self {
        memberships: u32::MAX,
        filter: u32::MAX,
    };

    pub const fn new(memberships: u32, filter: u32) -> Self {
        Self {
            memberships,
            filter,
        }
    }
}

impl Default for CollisionLayers {
    fn default() -> Self {
        Self::ALL
    }
}
