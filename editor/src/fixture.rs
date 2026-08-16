//! The smallest scene that exercises the editor.
//!
//! The demo scene is a rendering proof: eight entities, five of them the same
//! sprite five times, tuned so transparency ordering is visible. Testing the
//! editor against it means every assertion carries the demo's choices, and
//! retuning the demo breaks the editor's tests for no reason.
//!
//! This is the other thing: one cube, one sprite, and the cameras they need.
//! It exists to be asserted about, so it holds one of each rather than a
//! composition. Open it by hand with
//! `cargo run -p sindri-editor -- editor/assets/fixture.scene.json`.
//!
//! ## Why two cameras
//!
//! "One camera" is not enough and cannot be. A mesh needs a world camera or
//! extraction fails with `MissingWorldCamera`; a sprite resolves its anchor
//! against the overlay camera's extent and fails with `MissingOverlayCamera`
//! without one. A scene holding both a cube and a sprite therefore holds a
//! perspective camera and an orthographic one. Two is the minimum, not a
//! convenience.
//!
//! ## Textures
//!
//! Both texture references — `procedural:checkerboard` and
//! `textures/badge.png` — are ones `sindri_cube::demo_textures` already binds,
//! so the fixture renders in the editor without new asset plumbing. A
//! reference nothing binds would still draw, as the missing-texture checker;
//! that is a property worth having and a poor thing to open the editor into.

use std::path::PathBuf;

use crate::scene_file::{SceneFile, SceneFileError};

/// Where the fixture lives, relative to the repository root.
///
/// This is the path a person types. [`path`] is the one code should use.
pub const FIXTURE_SCENE_PATH: &str = "editor/assets/fixture.scene.json";

/// The fixture's location on this machine.
///
/// Resolved against the crate rather than the working directory, so a test
/// finds it whatever directory the test runner chose.
pub fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("fixture.scene.json")
}

/// Opens the fixture through the same path the editor opens any scene.
pub fn open() -> Result<SceneFile, SceneFileError> {
    SceneFile::open(path())
}
