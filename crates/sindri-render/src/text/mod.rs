//! Text as geometry: strings shaped from project-owned font bytes and turned
//! into one distance-field quad per glyph.
//!
//! Cosmic-text owns shaping, fallback and font matching. Sindri owns the stable
//! boundary around it: a frame carries logical font references and immutable
//! text instances, while a host binds bytes fetched through `sindri-assets`. No
//! system font is part of that contract, which keeps native and browser output
//! on the same face.
//!
//! Everything here is measured in the units the pass's camera draws — overlay
//! units for a HUD, world units for a canvas placed in the scene — and never in
//! viewport pixels. That is the difference from the screen-space text pass this
//! replaced: a string is flat geometry on the surface it was authored against,
//! so it pans, zooms and turns with that surface, and a viewport resizing does
//! not re-rasterise a single glyph.
//!
//! Four files, for the four things a string is on its way to the screen:
//!
//! - `options.rs` — the knobs it carries besides its words, each defaulting to
//!   the behaviour text had before the option existed.
//! - `instance.rs` — the immutable value a frame holds: words, face, size, and
//!   those options.
//! - `layout.rs` — where the words end up, which is arithmetic about boxes and
//!   alignment and needs no GPU to check.
//! - `renderer.rs` — shaping, the atlas, and the quads that come out.

mod instance;
mod layout;
mod options;
mod renderer;

use thiserror::Error;

pub use instance::TextInstance;
pub use layout::aligned_origin;
pub use options::{
    LineAlign, TextAlign, TextCase, TextFit, TextShadow, TextStroke, TextStyle, TextWrap,
};
pub use renderer::{GlyphQuads, TextRenderer};

use crate::TextureError;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum TextError {
    #[error("text position must be finite, got {0:?}")]
    NonFinitePosition([f32; 2]),
    #[error("font size must be finite and greater than zero, got {0}")]
    InvalidFontSize(f32),
    #[error("line height must be finite and greater than zero, got {0}")]
    InvalidLineHeight(f32),
    #[error("text color must be finite, got {0:?}")]
    NonFiniteColor([f32; 4]),
    #[error("could not build the glyph atlas: {0}")]
    Atlas(#[from] TextureError),
}
