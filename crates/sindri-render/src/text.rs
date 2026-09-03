//! Text as geometry: strings shaped from project-owned font bytes and turned
//! into one textured quad per glyph.
//!
//! Cosmic-text owns shaping, fallback and font matching. Sindri owns the stable
//! boundary around it: a frame carries logical font references and immutable
//! text instances, while a host binds bytes fetched through `sindri-assets`. No
//! system font is part of that contract, which keeps native and browser output
//! on the same face.
//!
//! Everything here is measured in the units the pass's camera draws — overlay
//! units for a HUD, world units for a canvas placed in the scene — and never in
//! viewport pixels. That is the whole difference from the screen-space text pass
//! this replaced: a string is now flat geometry on the surface it was authored
//! against, so it pans, zooms and turns with that surface, and a viewport
//! resizing does not re-rasterise a single glyph.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use thiserror::Error;

use crate::{GlyphAtlas, RASTER_EM, SpriteInstance, Texture2D, TextureError};

/// One laid-out string, in the units of the camera its pass is drawn through.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInstance {
    text: String,
    font: String,
    position: [f32; 2],
    font_size: f32,
    line_height: f32,
    color: [f32; 4],
    /// Which end of the string `position` names, across and then down.
    align: [TextAlign; 2],
}

/// Where a string of this size actually starts, given the point it was told to
/// sit at and which end of it that point names.
///
/// Its own function because it is the whole of what can be got wrong: a string
/// is laid out from its top-left, and every element around it is placed by its
/// centre.
#[must_use]
pub fn aligned_origin(instance: &TextInstance, size: [f32; 2]) -> [f32; 2] {
    let [across, down] = instance.align();
    [
        instance.position()[0] + across.start_after(size[0]),
        instance.position()[1] + down.top_above(size[1]),
    ]
}

/// How big a shaped string turned out, in raster pixels, across and down.
///
/// The width is the longest line rather than the box it was shaped in, which
/// would answer the same for every string.
fn laid_out(buffer: &Buffer, line_height: f32) -> [f32; 2] {
    let mut width = 0.0_f32;
    let mut lines = 0.0_f32;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        lines += 1.0;
    }
    [width, lines * line_height]
}

/// Which end of a string the point it was given belongs to.
///
/// A string is laid out from a corner, so a point alone does not say where it
/// goes: a title told to sit at the middle of the screen had its *top-left* put
/// there and ran off to the right. Every other element is placed by its centre,
/// so the anchor an author chose meant one thing for an image and something else
/// for the words on it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAlign {
    /// The point is where the string begins — its left edge, or its top.
    #[default]
    Start,
    /// The point is the middle of the string.
    Middle,
    /// The point is where the string ends — its right edge, or its bottom.
    End,
}

impl TextAlign {
    /// How far right of the point a string of this width starts.
    fn start_after(self, width: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Middle => -width * 0.5,
            Self::End => -width,
        }
    }

    /// How far *above* the point the top of a string of this height sits.
    ///
    /// Text is laid out downwards while the units it is placed in run upwards,
    /// so this is the one place that flip lives. Reading it off `start_after`
    /// with a minus sign is exactly the sort of thing that ends up written once
    /// per caller and once wrong.
    fn top_above(self, height: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Middle => height * 0.5,
            Self::End => height,
        }
    }
}

impl TextInstance {
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
        })
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

    /// How many of the units this string is placed in one raster pixel covers.
    ///
    /// Glyphs are baked once at [`RASTER_EM`] and every quad is a scaled copy,
    /// so this single number is what turns a shaped layout into geometry.
    fn units_per_raster_pixel(&self) -> f32 {
        self.font_size / RASTER_EM
    }
}

/// One glyph batch, ready to draw through the pass's own camera.
pub struct GlyphQuads<'a> {
    /// The atlas every quad in `instances` samples.
    pub atlas: &'a Texture2D,
    /// Which build of that atlas this is, for a renderer caching bind groups.
    pub generation: u64,
    pub instances: Vec<SpriteInstance>,
}

/// Glyph shaping, rasterisation and the atlas they land in, shared by every
/// viewport.
pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: GlyphAtlas,
    /// Logical asset reference to the family declared inside its bytes.
    fonts: BTreeMap<String, String>,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            atlas: GlyphAtlas::new(),
            fonts: BTreeMap::new(),
        }
    }

    /// Makes validated font bytes available under the scene reference.
    pub fn bind_font(
        &mut self,
        reference: impl Into<String>,
        family: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Option<String> {
        self.font_system.db_mut().load_font_data(bytes);
        self.fonts.insert(reference.into(), family.into())
    }

    pub fn unbind_font(&mut self, reference: &str) -> Option<String> {
        self.fonts.remove(reference)
    }

    pub fn clear_bindings(&mut self) {
        self.fonts.clear();
    }

    pub fn has_font(&self, reference: &str) -> bool {
        self.fonts.contains_key(reference)
    }

    /// How much of its own units one string covers, across and down.
    ///
    /// `None` for an unbound font, which is what [`Self::quads`] does with one
    /// too: a string whose face never arrived is not drawn, so it covers nothing
    /// and there is nothing to hit-test against.
    ///
    /// The width is the widest laid-out line and the height is the lines it
    /// actually used. Independent of any viewport, because the answer is a size
    /// in the scene now rather than a rectangle on a screen — which is what lets
    /// an editor's pick box be built once and stay right through a zoom.
    ///
    /// This exists because an editor cannot otherwise say where a string is.
    /// Every other drawn thing has a size in the scene; a string's is decided by
    /// glyph layout inside this module, and a guessed box picks the wrong thing
    /// near its edges. So the answer comes from the same shaping the frame is
    /// drawn from.
    pub fn measure(&mut self, instance: &TextInstance) -> Option<[f32; 2]> {
        let family = self.fonts.get(instance.font()).cloned()?;
        let scale = instance.units_per_raster_pixel();
        let buffer = self.shape(instance, &family);
        let [width, height] = laid_out(&buffer, instance.line_height() / scale);
        Some([width * scale, height * scale])
    }

    /// One instance laid out, the only place that is decided.
    ///
    /// Shared by drawing and measuring rather than written twice, because two
    /// copies is exactly how a pick box ends up disagreeing with the picture it
    /// is meant to be over.
    ///
    /// Always shaped at [`RASTER_EM`], whatever size the instance asked for: the
    /// atlas holds one baked copy of each glyph, and a string that shaped at its
    /// own size would key glyphs that are not in it. The requested size is a
    /// scale applied to the result.
    fn shape(&mut self, instance: &TextInstance, family: &str) -> Buffer {
        let scale = instance.units_per_raster_pixel();
        let mut buffer = Buffer::new(
            &mut self.font_system,
            Metrics::new(RASTER_EM, instance.line_height() / scale),
        );
        // No wrap width. A string's box is not authored, and wrapping to the
        // viewport was only ever meaningful while text was measured in it —
        // a canvas in the scene has no viewport of its own to wrap to. Authored
        // newlines still break lines.
        buffer.set_size(None, None);
        buffer.set_text(
            instance.text(),
            &Attrs::new().family(Family::Name(family)),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer
    }

    /// Shapes one ordered text pass into quads, and the atlas they sample.
    ///
    /// An unbound font skips its string. Asset diagnostics name it separately;
    /// silently choosing a machine font here would make a browser and a desktop
    /// disagree while both appeared to work.
    ///
    /// `None` when nothing is drawn — every string had an unbound font, or none
    /// of them put a mark anywhere.
    pub fn quads(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[TextInstance],
    ) -> Result<Option<GlyphQuads<'_>>, TextError> {
        let resolved: Vec<(&TextInstance, String)> = instances
            .iter()
            .filter_map(|instance| {
                self.fonts
                    .get(instance.font())
                    .cloned()
                    .map(|family| (instance, family))
            })
            .collect();
        if resolved.is_empty() {
            return Ok(None);
        }
        let mut buffers = Vec::with_capacity(resolved.len());
        for (instance, family) in &resolved {
            buffers.push(self.shape(instance, family));
        }

        // Two passes over the glyphs, and the split is load bearing: the atlas
        // may grow while it is being filled, and growing repacks it, so every
        // rect handed out before that moment stops being where its glyph is.
        // Filling it first and reading it after means one frame's quads all
        // describe the same atlas.
        for buffer in &buffers {
            for run in buffer.layout_runs() {
                for glyph in run.glyphs {
                    let key = glyph.physical((0.0, 0.0), 1.0).cache_key;
                    self.atlas
                        .slot(&mut self.font_system, &mut self.swash_cache, key);
                }
            }
        }

        let mut quads = Vec::new();
        for (buffer, (instance, _)) in buffers.iter().zip(resolved.iter()) {
            let scale = instance.units_per_raster_pixel();
            let [width, height] = laid_out(buffer, instance.line_height() / scale);
            let origin = aligned_origin(instance, [width * scale, height * scale]);
            for run in buffer.layout_runs() {
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    // Pen offsets within a shaped line, so a few hundred at
                    // most and exact in an f32 many times over.
                    #[allow(clippy::cast_precision_loss)]
                    let (pen_x, pen_y) = (physical.x as f32, physical.y as f32);
                    let Some(slot) = self.atlas.slot(
                        &mut self.font_system,
                        &mut self.swash_cache,
                        physical.cache_key,
                    ) else {
                        continue;
                    };
                    // Raster pixels from the string's top-left corner, down and
                    // to the right, which is the space cosmic-text lays out in.
                    let left = pen_x + slot.offset[0];
                    let top = run.line_y.round() + pen_y - slot.offset[1];
                    let size = [slot.size[0] * scale, slot.size[1] * scale];
                    let centre = [
                        origin[0] + (left + slot.size[0] * 0.5) * scale,
                        origin[1] - (top + slot.size[1] * 0.5) * scale,
                    ];
                    let model = Mat4::from_translation(Vec3::new(centre[0], centre[1], 0.0))
                        * Mat4::from_scale(Vec3::new(size[0], size[1], 1.0));
                    quads.push(SpriteInstance::new(model, instance.color()).with_uv_rect(slot.uv));
                }
            }
        }
        if quads.is_empty() {
            return Ok(None);
        }
        let (atlas, generation) = self.atlas.texture(device, queue)?;
        Ok(Some(GlyphQuads {
            atlas,
            generation,
            instances: quads,
        }))
    }

    /// How many distinct glyphs are baked into the atlas.
    ///
    /// A host's only window onto how much text has cost it.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.atlas.glyph_count()
    }
}

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

#[cfg(test)]
mod alignment_tests {
    use super::TextAlign;

    /// The point a string is given is one of three places on it, and each one
    /// puts the string somewhere different.
    #[test]
    fn an_alignment_says_how_far_back_a_string_starts() {
        assert!((TextAlign::Start.start_after(1.2) - 0.0).abs() < f32::EPSILON);
        assert!((TextAlign::Middle.start_after(1.2) + 0.6).abs() < f32::EPSILON);
        assert!((TextAlign::End.start_after(1.2) + 1.2).abs() < f32::EPSILON);
    }

    /// Down runs the other way from across, because layout goes down the page
    /// and the units a string is placed in go up it.
    #[test]
    fn the_top_of_a_string_is_above_the_point_it_was_given() {
        assert!((TextAlign::Start.top_above(1.2) - 0.0).abs() < f32::EPSILON);
        assert!((TextAlign::Middle.top_above(1.2) - 0.6).abs() < f32::EPSILON);
        assert!((TextAlign::End.top_above(1.2) - 1.2).abs() < f32::EPSILON);
    }

    /// A string of no size sits at its point however it is aligned, so an empty
    /// label does not jump about.
    #[test]
    fn nothing_is_in_the_same_place_whichever_end_it_is_measured_from() {
        for align in [TextAlign::Start, TextAlign::Middle, TextAlign::End] {
            assert!(align.start_after(0.0).abs() < f32::EPSILON, "{align:?}");
            assert!(align.top_above(0.0).abs() < f32::EPSILON, "{align:?}");
        }
    }
}
