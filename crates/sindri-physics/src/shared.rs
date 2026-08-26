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

impl RigidBodyKind {
    /// Every kind, in the order a chooser should offer them.
    ///
    /// Named here rather than wherever a list is drawn, so a kind added to the
    /// enum is offered without anyone remembering to add it twice.
    pub const ALL: [Self; 4] = [
        Self::Static,
        Self::Dynamic,
        Self::KinematicPosition,
        Self::KinematicVelocity,
    ];

    /// The name this kind is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
            Self::KinematicPosition => "kinematic_position",
            Self::KinematicVelocity => "kinematic_velocity",
        }
    }
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
