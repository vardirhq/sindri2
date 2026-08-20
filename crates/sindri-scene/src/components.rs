use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_render::{UvRect, UvRectError};

/// A camera authored into a scene.
///
/// The projection tag chooses which fields apply, so a scene cannot describe a
/// perspective camera with an orthographic size.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "projection", rename_all = "snake_case")]
pub enum CameraComponent {
    /// Renders the 3D world. Its eye comes from the entity's `Transform3D`.
    Perspective {
        target: [f32; 3],
        up: [f32; 3],
        vertical_fov_degrees: f32,
        near: f32,
        far: f32,
    },
    /// Renders the 2D overlay, and defines the space sprite anchors resolve in.
    Orthographic {
        center: [f32; 2],
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

impl SceneComponent for CameraComponent {
    const TYPE_NAME: &'static str = "sindri.camera";
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MeshPrimitive {
    Cube,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MeshComponent {
    pub primitive: MeshPrimitive,
    pub texture: String,
    #[serde(default)]
    pub layer: i32,
}

impl SceneComponent for MeshComponent {
    const TYPE_NAME: &'static str = "sindri.mesh";
}

/// The space a sprite is placed and drawn in.
///
/// Screen is the default because it is what every sprite was before there was a
/// choice, so no existing scene changes meaning by gaining the field.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SpriteSpace {
    /// Drawn through the overlay camera, anchored to its extent. A HUD is not
    /// in the world, so no world camera moves it and nothing in the world can
    /// hide it. Its Z says how far back in the stack it sits and nothing else:
    /// it orders the sprite without moving it, so no HUD can be lost off the
    /// far plane by typing a big number.
    #[default]
    Screen,
    /// Placed in the world by its transform and drawn through the world camera,
    /// like any other thing in the scene: it moves when the camera moves, it
    /// has a Z, and opaque geometry in front of it hides it.
    World,
}

/// Where a screen-space sprite's origin sits inside the overlay camera's view.
///
/// Anchoring is resolved against the overlay camera's extent, so a sprite keeps
/// its relationship to an edge as the window changes shape. A world-space
/// sprite has no edge to hold on to, which is what [`SpriteComponent::screen_anchor`]
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
    /// Which part of the texture to draw, as `[x, y, width, height]` in
    /// normalized coordinates.
    ///
    /// The whole texture unless a scene says otherwise, so every scene written
    /// before sheets existed reads exactly as it did — and saves exactly as it
    /// did too, since a component is a typed view of a stored payload and the
    /// payload is what gets written back.
    ///
    /// Read through [`SpriteComponent::uv_rect`], which checks it: a rect of no
    /// area or one reaching past the edge is a picture that is quietly wrong
    /// rather than an error.
    #[serde(default = "full_uv_rect")]
    pub uv_rect: [f32; 4],
    /// The explicit override on draw order. Within a layer sprites sort by how
    /// far from the camera they are; a layer beats that, so a sprite in a
    /// higher one draws in front of something nearer the camera.
    #[serde(default)]
    pub layer: i32,
}

const fn opaque_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn full_uv_rect() -> [f32; 4] {
    UvRect::FULL.to_array()
}

impl SpriteComponent {
    /// The part of the texture this sprite draws, checked.
    ///
    /// Checked here rather than at deserialization because a scene carrying a
    /// bad rect should still open — the editor exists to fix it, and refusing
    /// the file would be refusing to let anyone.
    pub fn uv_rect(&self) -> Result<UvRect, UvRectError> {
        let [x, y, width, height] = self.uv_rect;
        UvRect::new(x, y, width, height)
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
