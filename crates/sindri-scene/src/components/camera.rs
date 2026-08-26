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

impl SceneComponent for CameraComponent {
    const TYPE_NAME: &'static str = "sindri.camera";
}
