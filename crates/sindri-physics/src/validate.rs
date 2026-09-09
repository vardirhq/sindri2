//! What the world refuses to be given, and why.

use sindri_core::EntityId;
use thiserror::Error;

use crate::shared::RigidBodyKind;
use crate::types2d::{Collider2d, ColliderShape2d, PhysicsPose2d, RigidBody2d};

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PhysicsError {
    #[error("entity {0:?} is already registered with physics")]
    EntityAlreadyRegistered(EntityId),
    #[error("entity {0:?} has no physics body")]
    MissingEntity(EntityId),
    #[error("entity {0:?} cannot perform {1} with a {2:?} body")]
    WrongBodyKind(EntityId, &'static str, RigidBodyKind),
    #[error("physics value '{0}' must be finite")]
    NonFinite(&'static str),
    #[error("physics value '{0}' must be positive")]
    NonPositive(&'static str),
    #[error("physics value '{0}' must be non-negative")]
    Negative(&'static str),
    #[error("physics value '{0}' must be between 0 and 1")]
    NotNormalized(&'static str),
    #[error("physics timestep must be finite and greater than zero")]
    InvalidTimestep,
    #[error("collider piece {index} is invalid: {reason}")]
    ColliderPiece {
        index: usize,
        reason: Box<PhysicsError>,
    },
    #[error("entity {0:?} has a collider with no pieces")]
    NoColliderPieces(EntityId),
}

pub(crate) fn validate_body2d(body: &RigidBody2d) -> Result<(), PhysicsError> {
    validate_pose2d(body.pose)?;
    finite2("linear_velocity", body.linear_velocity)?;
    finite("angular_velocity", body.angular_velocity)?;
    finite("gravity_scale", body.gravity_scale)?;
    non_negative("linear_damping", body.linear_damping)?;
    non_negative("angular_damping", body.angular_damping)
}

/// Checks every piece of a compound, naming the one that is wrong.
///
/// The index is in the message because a compound is authored as a list and a
/// bad piece is otherwise a needle in it: "restitution must be between 0 and 1"
/// says nothing about *which* of five colliders to fix.
pub(crate) fn validate_colliders2d(colliders: &[Collider2d]) -> Result<(), PhysicsError> {
    for (index, collider) in colliders.iter().enumerate() {
        validate_collider2d(collider).map_err(|reason| PhysicsError::ColliderPiece {
            index,
            reason: Box::new(reason),
        })?;
    }
    Ok(())
}

pub(crate) fn validate_collider2d(collider: &Collider2d) -> Result<(), PhysicsError> {
    finite2("collider_offset", collider.offset)?;
    finite("collider_rotation", collider.rotation)?;
    non_negative("friction", collider.friction)?;
    normalized("restitution", collider.restitution)?;
    match collider.shape {
        ColliderShape2d::Box { half_extents } => {
            positive("box_half_extent_x", half_extents[0])?;
            positive("box_half_extent_y", half_extents[1])
        }
        ColliderShape2d::Circle { radius } => positive("circle_radius", radius),
        ColliderShape2d::Capsule {
            half_height,
            radius,
        } => {
            positive("capsule_half_height", half_height)?;
            positive("capsule_radius", radius)
        }
    }
}

pub(crate) fn validate_pose2d(pose: PhysicsPose2d) -> Result<(), PhysicsError> {
    finite2("position", pose.position)?;
    finite("rotation", pose.rotation)
}

pub(crate) fn finite(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PhysicsError::NonFinite(name))
    }
}

pub(crate) fn finite2(name: &'static str, value: [f32; 2]) -> Result<(), PhysicsError> {
    if value.into_iter().all(f32::is_finite) {
        Ok(())
    } else {
        Err(PhysicsError::NonFinite(name))
    }
}

pub(crate) fn positive(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(PhysicsError::NonPositive(name))
    }
}

pub(crate) fn non_negative(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    finite(name, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(PhysicsError::Negative(name))
    }
}

pub(crate) fn normalized(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    finite(name, value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(PhysicsError::NotNormalized(name))
    }
}
