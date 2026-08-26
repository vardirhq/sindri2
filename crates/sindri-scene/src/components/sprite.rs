//! `sindri.sprite`: an image, where it sits, and what it is anchored by.

use serde::Deserialize;
use sindri_core::{SceneComponent, SpriteRef, SpriteRefError};

/// The space a sprite is placed and drawn in.
///
/// Screen is the default because it is what every sprite was before there was a
/// choice, so no existing scene changes meaning by gaining the field.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SpriteSpace {
    /// Drawn directly in viewport-owned overlay space. A HUD is not in the
    /// world, so no world camera moves it and nothing in the world can hide it.
    /// Its Z says how far back in the stack it sits and nothing else: it orders
    /// the sprite without moving it, so no HUD can be lost off a camera far
    /// plane by typing a big number.
    #[default]
    Screen,
    /// Placed in the world by its transform and drawn through the world camera,
    /// like any other thing in the scene: it moves when the camera moves, it
    /// has a Z, and opaque geometry in front of it hides it.
    World,
}

/// Where a screen-space sprite's origin sits inside the viewport.
///
/// Anchoring is resolved against the viewport extent, so a sprite keeps its
/// relationship to an edge as the window changes shape. A world-space sprite
/// has no edge to hold on to, which is what [`SpriteComponent::screen_anchor`]
/// says in the type.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpriteAnchor {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl SpriteAnchor {
    /// The anchor as a fraction of the half-extent, in `[-1, 1]` per axis.
    pub const fn unit_offset(self) -> [f32; 2] {
        match self {
            Self::Center => [0.0, 0.0],
            Self::Top => [0.0, 1.0],
            Self::Bottom => [0.0, -1.0],
            Self::Left => [-1.0, 0.0],
            Self::Right => [1.0, 0.0],
            Self::TopLeft => [-1.0, 1.0],
            Self::TopRight => [1.0, 1.0],
            Self::BottomLeft => [-1.0, -1.0],
            Self::BottomRight => [1.0, -1.0],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpriteComponent {
    pub texture: String,
    #[serde(default)]
    pub space: SpriteSpace,
    /// Only a screen-space sprite anchors. Read it through
    /// [`SpriteComponent::screen_anchor`] rather than directly, so a
    /// world-space sprite cannot be quietly anchored to an edge it does not
    /// have.
    #[serde(default)]
    pub anchor: SpriteAnchor,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    /// The explicit override on draw order. Within a layer sprites sort by how
    /// far from the camera they are; a layer beats that, so a sprite in a
    /// higher one draws in front of something nearer the camera.
    #[serde(default)]
    pub layer: i32,
}

pub(super) const fn opaque_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
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

    /// The anchor this sprite resolves against, or `None` when it is in the
    /// world, where there is no screen edge to anchor to.
    pub const fn screen_anchor(&self) -> Option<SpriteAnchor> {
        match self.space {
            SpriteSpace::Screen => Some(self.anchor),
            SpriteSpace::World => None,
        }
    }
}

impl SceneComponent for SpriteComponent {
    const TYPE_NAME: &'static str = "sindri.sprite";
}
