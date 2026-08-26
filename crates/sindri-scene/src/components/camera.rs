//! `sindri.camera`: how a scene's own camera sees the world.

use serde::Deserialize;
use sindri_core::SceneComponent;

/// A camera authored into a scene.
///
/// Every authored camera renders the world. The projection tag chooses which
/// fields apply, so a scene cannot describe a perspective camera with an
/// orthographic size. Position and orientation come from the entity's
/// `Transform3D`: local -Z is forward and local +Y is up.
///
/// Screen-space sprites and text are viewport-owned and do not require a camera
/// entity.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "projection", rename_all = "snake_case")]
pub enum CameraComponent {
    Perspective {
        vertical_fov_degrees: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

impl CameraComponent {
    /// The projections a camera may have, by the names a scene stores.
    ///
    /// Named here because the tag decides which other fields the component
    /// has: an editor that offers this choice has to write the fields the
    /// chosen projection needs, and it should not be inventing its own idea of
    /// what those are.
    pub const PROJECTIONS: [&'static str; 2] = ["perspective", "orthographic"];

    /// What a perspective camera frames when nothing has said otherwise.
    ///
    /// A quarter turn of vertical view: wide enough to see a scene, narrow
    /// enough not to distort it.
    pub const DEFAULT_VERTICAL_FOV_DEGREES: f32 = 60.0;

    /// How much world an orthographic camera frames vertically by default.
    ///
    /// Six units rather than the renderer's own two, because two is the screen
    /// overlay's extent and a world camera framing two units of world puts an
    /// author inside whatever they were looking at.
    pub const DEFAULT_VERTICAL_SIZE: f32 = 6.0;

    /// The near and far planes a camera starts with.
    pub const DEFAULT_NEAR: f32 = 0.1;
    pub const DEFAULT_FAR: f32 = 100.0;

    /// The name of the projection this camera has.
    #[must_use]
    pub const fn projection_name(&self) -> &'static str {
        match self {
            Self::Perspective { .. } => Self::PROJECTIONS[0],
            Self::Orthographic { .. } => Self::PROJECTIONS[1],
        }
    }
}

impl SceneComponent for CameraComponent {
    const TYPE_NAME: &'static str = "sindri.camera";
}
