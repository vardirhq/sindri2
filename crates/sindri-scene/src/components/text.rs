//! `sindri.text`: a string, a project font, and a size.

use serde::Deserialize;
use sindri_core::SceneComponent;

use super::sprite::{SpriteAnchor, opaque_white};

/// Screen-space text drawn directly in viewport-owned overlay space.
///
/// The font is a project asset reference rather than a family installed on the
/// machine. That keeps a scene reproducible across the editor, captures, and
/// the browser: a host binds the bytes at that reference before drawing.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TextComponent {
    pub text: String,
    pub font: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "opaque_white")]
    pub color: [f32; 4],
    #[serde(default)]
    pub anchor: SpriteAnchor,
    #[serde(default)]
    pub layer: i32,
}

const fn default_font_size() -> f32 {
    24.0
}

const fn default_line_height() -> f32 {
    30.0
}

impl SceneComponent for TextComponent {
    const TYPE_NAME: &'static str = "sindri.text";
}
