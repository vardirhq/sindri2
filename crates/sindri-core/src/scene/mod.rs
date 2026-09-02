//! The authored scene format.
//!
//! A scene is a versioned, canonical document: it declares the format version
//! it was written against, it has exactly one serialized form, and it keeps
//! component payloads it does not understand. `document` is what it holds,
//! `canonical` is how it is written down, and `error` is what it refuses.

mod canonical;
mod document;
mod error;
mod graph;

#[cfg(test)]
mod tests;

pub use document::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, SceneMetadata,
};
pub use error::{SceneError, SceneJsonError};

pub(crate) use canonical::collapse_scalar_arrays;
pub(crate) use graph::{roots, validate_entities};
