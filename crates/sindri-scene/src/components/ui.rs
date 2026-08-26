//! `sindri.ui.*`: what a viewport draws on top of the world.
//!
//! One family, one rule: everything here is placed against the viewport rather
//! than in the scene. A UI component is anchored to an edge of the screen, is
//! drawn through a projection the viewport owns, and no authored camera can
//! move it or lose it behind geometry. That is what makes a UI entity a
//! different kind of thing from a world entity, and it is why these are their
//! own components instead of a `space` field on the world ones: the difference
//! is not a value a component holds, it is which fields the component has.

use serde::Deserialize;
use sindri_core::{SceneComponent, SpriteRef, SpriteRefError};

use super::opaque_white;

/// Where a UI element's origin sits inside the viewport.
///
/// Anchoring is resolved against the viewport extent, so an element keeps its
/// relationship to an edge as the window changes shape. The entity's transform
/// is read as an offset from the anchor, which is what lets a HUD row be five
/// entities at five X offsets from one corner.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiAnchor {
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

impl UiAnchor {
    /// Every anchor, in the order a chooser should offer them.
    ///
    /// Named here rather than in whatever draws the list, so an anchor added
    /// to the enum appears in the editor without anyone remembering to add it
    /// twice.
    pub const ALL: [Self; 9] = [
        Self::Center,
        Self::Top,
        Self::Bottom,
        Self::Left,
        Self::Right,
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    /// The name this anchor is stored under, which is the one a payload holds.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }

    /// The anchor as a fraction of the half-extent, in `[-1, 1]` per axis.
    #[must_use]
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

/// An image drawn on the viewport: a HUD pip, a banner, a crosshair.
///
/// Its Z says how far back in the stack it sits and nothing else: it orders the
/// element without moving it, so no HUD can be lost off a camera's far plane by
/// someone typing a big number.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiImageComponent {
    pub texture: String,
    #[serde(default)]
    pub anchor: UiAnchor,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    /// The explicit override on draw order within the overlay.
    #[serde(default)]
    pub layer: i32,
}

impl UiImageComponent {
    /// The texture this element draws, and which named part of it.
    ///
    /// The same reference a world sprite uses, checked the same way and for the
    /// same reason: a scene carrying a bad reference has to open, because the
    /// editor is where it gets fixed.
    pub fn reference(&self) -> Result<SpriteRef, SpriteRefError> {
        SpriteRef::parse(&self.texture)
    }
}

impl SceneComponent for UiImageComponent {
    const TYPE_NAME: &'static str = "sindri.ui.image";
}

/// Text drawn on the viewport, anchored the same way an image is.
///
/// The font is a project asset reference rather than a family installed on the
/// machine. That keeps a scene reproducible across the editor, captures, and
/// the browser: a host binds the bytes at that reference before drawing.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiTextComponent {
    pub text: String,
    pub font: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "opaque_white")]
    pub color: [f32; 4],
    #[serde(default)]
    pub anchor: UiAnchor,
    #[serde(default)]
    pub layer: i32,
}

const fn default_font_size() -> f32 {
    24.0
}

const fn default_line_height() -> f32 {
    30.0
}

impl SceneComponent for UiTextComponent {
    const TYPE_NAME: &'static str = "sindri.ui.text";
}
