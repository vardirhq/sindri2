//! What a scene refuses, and why.

use thiserror::Error;

use crate::SceneMigrationError;

use super::document::SceneEntityId;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SceneError {
    #[error("scene entity IDs cannot be empty")]
    EmptyEntityId,
    #[error("scene contains duplicate entity IDs")]
    DuplicateEntityId,
    #[error("scene format {found} is unsupported; this runtime supports {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("entity {entity:?} refers to missing parent {parent:?}")]
    MissingParent {
        entity: SceneEntityId,
        parent: SceneEntityId,
    },
    #[error("hierarchy cycle detected at entity {0:?}")]
    HierarchyCycle(SceneEntityId),
    #[error("entity {0:?} has a transform containing a non-finite value")]
    NonFiniteTransform(SceneEntityId),
}

/// Failures raised while reading or writing serialized scenes.
///
/// [`SceneError`] stays comparable and free of I/O concerns; this type carries
/// the JSON and migration failures that surround it.
#[derive(Debug, Error)]
pub enum SceneJsonError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Invalid(#[from] SceneError),
    #[error(transparent)]
    Migration(#[from] SceneMigrationError),
}
