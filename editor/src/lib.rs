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
/// Hearing a clip without a running scene.
/// Getting from no local AI at all to a verified assistant.
#[cfg(not(target_arch = "wasm32"))]
pub mod assistant;
#[cfg(not(target_arch = "wasm32"))]
pub mod audition;
/// Scene-view geometry for authored camera frustums.
#[cfg(not(target_arch = "wasm32"))]
pub mod camera_visualization;
/// What the editor knows about a component type that the registry does not.
#[cfg(not(target_arch = "wasm32"))]
pub mod components;
#[cfg(not(target_arch = "wasm32"))]
pub mod console;
/// Which panel is where, and the drag that moves one.
#[cfg(not(target_arch = "wasm32"))]
pub mod dock;
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
/// One field that finds anything: panels, entities, files, and verbs.
#[cfg(not(target_arch = "wasm32"))]
pub mod palette;
#[cfg(not(target_arch = "wasm32"))]
pub mod picking;
#[cfg(not(target_arch = "wasm32"))]
pub mod preferences;
/// Looking at a file the project browser lists.
#[cfg(not(target_arch = "wasm32"))]
pub mod preview;
#[cfg(not(target_arch = "wasm32"))]
pub mod profile;
#[cfg(not(target_arch = "wasm32"))]
pub mod project;
/// The editor-only camera used by the Scene view.
#[cfg(not(target_arch = "wasm32"))]
pub mod scene_camera;
#[cfg(not(target_arch = "wasm32"))]
pub mod scene_file;
/// Which of a scene's two spaces an entity belongs to.
#[cfg(not(target_arch = "wasm32"))]
pub mod space;

/// Where an entity sits among its siblings.
#[cfg(not(target_arch = "wasm32"))]
pub mod ordering;
/// The Decay scripts an open scene runs.
#[cfg(not(target_arch = "wasm32"))]
pub mod scripts;
/// What "the selection" is once it can be more than one entity.
#[cfg(not(target_arch = "wasm32"))]
pub mod selection;
/// Slicing an image into named sprites, on the image.
#[cfg(not(target_arch = "wasm32"))]
pub mod slicer;
#[cfg(not(target_arch = "wasm32"))]
pub mod textures;
/// Painting a tilemap from a sliced image through undoable component edits.
#[cfg(not(target_arch = "wasm32"))]
pub mod tilemap;
pub mod tile_volume;
/// Seeing a font before naming it in a component.
#[cfg(not(target_arch = "wasm32"))]
pub mod typeface;
/// The editor's design system: its tokens, its icons, and its controls.
#[cfg(not(target_arch = "wasm32"))]
pub mod ui;
/// Project-declared Weave presentation used by Scene and Game previews.
#[cfg(not(target_arch = "wasm32"))]
pub mod weave_styles;
