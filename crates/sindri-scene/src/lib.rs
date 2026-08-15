//! The seam between a simulated world and a drawn frame.
//!
//! `sindri-render` deliberately knows nothing about worlds, components, or
//! scenes, and `sindri-core` knows nothing about drawing. This crate bridges
//! them: it owns the built-in `sindri.*` component schemas and derives an
//! ordered frame from whatever a world currently holds, so gameplay only ever
//! writes to the world and no scene needs hand-written extraction code.

mod components;
mod extract;
mod textures;

pub use components::{
    CameraComponent, MeshComponent, MeshPrimitive, SpriteAnchor, SpriteComponent,
};
pub use extract::{CameraView, SceneExtractError, SceneExtractor, WorldProjection};
pub use textures::{TextureBindings, unresolved_textures};
