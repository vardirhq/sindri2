//! The immutable value a frame holds for one string.
//!
//! Built from the six things every string needs and then adjusted: every option
//! past [`TextInstance::new`] has a default that draws exactly what the string
//! drew before that option existed, so a caller wanting none of them writes
//! none of them.

use crate::RASTER_EM;

use super::TextError;
use super::options::{
    LineAlign, TextAlign, TextCase, TextFit, TextShadow, TextStroke, TextStyle, TextWrap,
};

/// One laid-out string, in the units of the camera its pass is drawn through.
///
/// Built from the six things every string needs and then adjusted: every option
/// past [`TextInstance::new`] has a default that draws exactly what the string
/// drew before it existed.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInstance {
    pub(super) text: String,
    pub(super) font: String,
    pub(super) position: [f32; 2],
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    pub(super) color: [f32; 4],
    /// Which end of the string `position` names, across and then down. With a
    /// box, which end of the *box* — and where the block sits inside it.
    pub(super) align: [TextAlign; 2],
    /// The rect the text is laid out in, or zero on an axis with no bound.
    pub(super) bounds: [f32; 2],
    pub(super) wrap: TextWrap,
    pub(super) line_align: LineAlign,
    pub(super) letter_spacing: f32,
    pub(super) style: TextStyle,
    pub(super) case: TextCase,
    /// How many glyphs are drawn, for a reveal. `None` draws them all.
    pub(super) visible: Option<usize>,
    pub(super) outline: TextStroke,
    pub(super) shadow: TextShadow,
    pub(super) fit: Option<TextFit>,
}

impl TextInstance {
    /// A string with every option at its default.
    pub fn new(
        text: impl Into<String>,
        font: impl Into<String>,
        position: [f32; 2],
        font_size: f32,
        line_height: f32,
        color: [f32; 4],
        align: [TextAlign; 2],
    ) -> Result<Self, TextError> {
        if !position.into_iter().all(f32::is_finite) {
            return Err(TextError::NonFinitePosition(position));
        }
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(TextError::InvalidFontSize(font_size));
        }
        if !line_height.is_finite() || line_height <= 0.0 {
            return Err(TextError::InvalidLineHeight(line_height));
        }
        if !color.into_iter().all(f32::is_finite) {
            return Err(TextError::NonFiniteColor(color));
        }
        Ok(Self {
            text: text.into(),
            font: font.into(),
            position,
            font_size,
            line_height,
            color,
            align,
            bounds: [0.0, 0.0],
            wrap: TextWrap::None,
            line_align: LineAlign::Follow,
            letter_spacing: 0.0,
            style: TextStyle::default(),
            case: TextCase::AsWritten,
            visible: None,
            outline: TextStroke::default(),
            shadow: TextShadow::default(),
            fit: None,
        })
    }

    /// The rect the text is laid out in, and how it breaks inside it.
    ///
    /// Zero on an axis means unbounded there, which is what a HUD reading wants:
    /// a score has no box and should not be wrapped or shrunk to fit one. A
    /// width is what wrapping needs; a height is what auto-sizing needs.
    #[must_use]
    pub fn in_box(mut self, bounds: [f32; 2], wrap: TextWrap) -> Self {
        self.bounds = bounds.map(|side| if side.is_finite() { side.max(0.0) } else { 0.0 });
        self.wrap = wrap;
        self
    }

    /// Where each line sits across the box, when that differs from where the
    /// whole block sits.
    #[must_use]
    pub const fn with_line_align(mut self, line_align: LineAlign) -> Self {
        self.line_align = line_align;
        self
    }

    /// Extra space after every glyph, in the units the text is placed in.
    #[must_use]
    pub fn with_letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = if spacing.is_finite() { spacing } else { 0.0 };
        self
    }

    /// The weight and slant asked of the face.
    #[must_use]
    pub const fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Whether the words are shaped as written, upper, or lower.
    #[must_use]
    pub const fn with_case(mut self, case: TextCase) -> Self {
        self.case = case;
        self
    }

    /// Draws only the first `glyphs` of the string, for a reveal.
    ///
    /// Counted in glyphs rather than characters, because glyphs are what a quad
    /// is made of and a ligature has no half. For a typewriter effect over Latin
    /// text the two are the same thing.
    #[must_use]
    pub const fn with_visible_glyphs(mut self, glyphs: usize) -> Self {
        self.visible = Some(glyphs);
        self
    }

    /// A stroke around the glyphs.
    #[must_use]
    pub const fn with_outline(mut self, outline: TextStroke) -> Self {
        self.outline = outline;
        self
    }

    /// A copy drawn behind the text, offset and softened.
    #[must_use]
    pub const fn with_shadow(mut self, shadow: TextShadow) -> Self {
        self.shadow = shadow;
        self
    }

    /// Picks the largest size in `fit` that keeps the text inside its box.
    ///
    /// Needs a box to fit in: with no bounds there is nothing to be too big for,
    /// and the authored size stands.
    #[must_use]
    pub const fn fitted(mut self, fit: TextFit) -> Self {
        self.fit = Some(fit);
        self
    }

    /// Which end of the string its position names, across and then down.
    #[must_use]
    pub const fn align(&self) -> [TextAlign; 2] {
        self.align
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn font(&self) -> &str {
        &self.font
    }

    pub const fn position(&self) -> [f32; 2] {
        self.position
    }

    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    pub const fn line_height(&self) -> f32 {
        self.line_height
    }

    pub const fn color(&self) -> [f32; 4] {
        self.color
    }

    /// The rect the text is laid out in, zero on an unbounded axis.
    #[must_use]
    pub const fn bounds(&self) -> [f32; 2] {
        self.bounds
    }

    /// Whether this string is laid out within a width.
    ///
    /// Both halves are needed: a width with no wrap mode does not bound the
    /// layout, and a wrap mode with no width has nothing to wrap to.
    pub(super) fn wraps(&self) -> bool {
        self.bounds[0] > 0.0 && self.wrap != TextWrap::None
    }

    /// How many of the units this string is placed in one raster pixel covers,
    /// at a given font size.
    ///
    /// Glyphs are baked once at [`RASTER_EM`] and every quad is a scaled copy,
    /// so this single number is what turns a shaped layout into geometry.
    pub(super) fn units_per_raster_pixel(size: f32) -> f32 {
        size / RASTER_EM
    }
}

#[cfg(test)]
mod tests {
    use super::{TextAlign, TextError, TextInstance};

    #[test]
    fn text_instances_refuse_values_a_gpu_cannot_place() {
        assert!(matches!(
            TextInstance::new(
                "hello",
                "font.ttf",
                [0.0, 0.0],
                0.0,
                0.1,
                [1.0; 4],
                [TextAlign::Start; 2]
            ),
            Err(TextError::InvalidFontSize(0.0))
        ));
        assert!(matches!(
            TextInstance::new(
                "hello",
                "font.ttf",
                [f32::NAN, 0.0],
                0.06,
                0.07,
                [1.0; 4],
                [TextAlign::Start; 2]
            ),
            Err(TextError::NonFinitePosition(_))
        ));
    }
}
