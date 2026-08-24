//! The Sindri editor.
//!
//! This crate is an application, and the binary in `main.rs` is all of it that
//! ships. The library exists so the parts that are not drawing — opening and
//! saving scene files, remembering preferences, the fixture scene — can be
//! reached from `tests/`. A binary-only crate cannot have integration tests,
//! and an editor with no way to test a save is an editor whose saves are
//! tested by opening it and looking.

#[cfg(not(target_arch = "wasm32"))]
pub mod animation;
#[cfg(not(target_arch = "wasm32"))]
pub mod console;
#[cfg(not(target_arch = "wasm32"))]
pub mod fixture;
/// Direct manipulation handles for Scene-view transforms.
#[cfg(not(target_arch = "wasm32"))]
pub mod gizmo;
/// The keyboard, as a running script sees it.
#[cfg(not(target_arch = "wasm32"))]
pub mod input;
/// Editing what an entity's components hold.
#[cfg(not(target_arch = "wasm32"))]
pub mod inspector;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
/// Selecting rendered entities through the Scene viewport.
#[cfg(not(target_arch = "wasm32"))]
pub mod picking;
#[cfg(not(target_arch = "wasm32"))]
pub mod preferences;
#[cfg(not(target_arch = "wasm32"))]
pub mod project;
/// The editor-only camera used by the Scene view.
#[cfg(not(target_arch = "wasm32"))]
pub mod scene_camera;
#[cfg(not(target_arch = "wasm32"))]
pub mod scene_file;

/// The Decay scripts an open scene runs.
#[cfg(not(target_arch = "wasm32"))]
pub mod scripts;
/// Slicing an image into named sprites, on the image.
#[cfg(not(target_arch = "wasm32"))]
pub mod slicer;
#[cfg(not(target_arch = "wasm32"))]
pub mod textures;
/// Painting a tilemap from a sliced image through undoable component edits.
#[cfg(not(target_arch = "wasm32"))]
pub mod tilemap;
