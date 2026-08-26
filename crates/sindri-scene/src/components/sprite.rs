//! `sindri.sprite`: an image placed in the world.

use serde::Deserialize;
use sindri_core::{SceneComponent, SpriteRef, SpriteRefError};

use super::opaque_white;

/// An image drawn in the world, like anything else in the scene.
///
/// A sprite is placed by its transform and drawn through the world camera: it
/// moves when the camera moves, it has a Z, and opaque geometry in front of it
/// hides it. What a HUD needs instead — a viewport edge to hold on to and a
/// camera that cannot move it — is [`super::UiImageComponent`], which is a
/// different component rather than a field on this one.
///
/// That split is the whole of the difference. A sprite used to carry a `space`
/// saying which of the two it was, so the same component meant two things and
/// showed two sets of fields depending on which: an anchor mattered on a
/// screen sprite and decided nothing on a world one. A component that has to
/// hide half of itself is two components.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpriteComponent {
    pub texture: String,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    /// The explicit override on draw order. Within a layer sprites sort by how
    /// far from the camera they are; a layer beats that, so a sprite in a
    /// higher one draws in front of something nearer the camera.
    #[serde(default)]
    pub layer: i32,
}

impl SpriteComponent {
    /// The texture this sprite draws, and which named part of it.
    ///
    /// `textures/tiles.png#floor` draws one sprite of a sliced sheet;
    /// `textures/badge.png` draws the whole image. Which part is no longer the
    /// sprite's business to describe — the sheet beside the image says how it
    /// is cut, and this only picks one of the names it gives.
    ///
    /// Checked here rather than at deserialization because a scene carrying a
    /// bad reference should still open — the editor exists to fix it, and
    /// refusing the file would be refusing to let anyone.
    pub fn reference(&self) -> Result<SpriteRef, SpriteRefError> {
        SpriteRef::parse(&self.texture)
    }
}

impl SceneComponent for SpriteComponent {
    const TYPE_NAME: &'static str = "sindri.sprite";
}
