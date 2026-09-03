//! Screen-space text shaped and rasterised from project-owned font bytes.
//!
//! Glyphon owns shaping, fallback and its glyph atlas. Sindri owns the stable
//! boundary around it: a frame carries logical font references and immutable
//! text instances, while a host binds bytes fetched through `sindri-assets`.
//! No system font is part of that contract, which keeps native and browser
//! output on the same face.

use std::collections::BTreeMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer as GlyphonRenderer, Viewport as GlyphonViewport,
};
use thiserror::Error;

use crate::Viewport;

/// One laid-out string in physical viewport pixels.
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
/// Its own function because it is the whole of the fix and the whole of what
/// can be got wrong: a string is laid out from its top-left, and every overlay
/// element around it is placed by its centre.
#[must_use]
pub fn aligned_origin(instance: &TextInstance, size: [f32; 2]) -> [f32; 2] {
    let [across, down] = instance.align();
    [
        instance.position()[0] + across.offset(size[0]),
        instance.position()[1] + down.offset(size[1]),
    ]
}

/// How big a shaped string turned out, across and down.
///
/// The width is the longest line rather than the box it was shaped in, which is
/// the viewport and would answer the same for every string in it.
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
/// goes: a title told to sit at the middle of the screen had its *top-left*
/// put there and ran off to the right. Every other overlay element is placed by
/// its centre, so the anchor an author chose meant one thing for an image and
/// something else for the words on it.
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
    /// How far back from the point a string of this size starts.
    fn offset(self, size: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Middle => -size * 0.5,
            Self::End => -size,
        }
    }
}

/// The smallest text that can put anything on a screen.
///
/// A renderer is the one place that still speaks in pixels: a scene asks for a
/// share of the screen, and this is where that share stops being able to be a
/// glyph. Text is clamped up to it rather than refused, because reaching it
/// means the window is small, which is a thing windows do.
pub const MIN_TEXT_PIXELS: f32 = 1.0;

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
        // A size arrives here in pixels, worked out from a share of the screen
        // — so one below a pixel means the window is tiny, not that the scene
        // is wrong, and a window someone dragged small should shrink its text
        // rather than stop the frame. The scene said what share of the screen
        // it wanted; this is the floor at which that share stops being a glyph.
        let font_size = font_size.max(MIN_TEXT_PIXELS);
        let line_height = line_height.max(MIN_TEXT_PIXELS);
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
}

/// Glyph shaping, caching and rendering shared by every viewport.
pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: GlyphonViewport,
    atlas: TextAtlas,
    renderer: GlyphonRenderer,
    /// Logical asset reference to the family declared inside its bytes.
    fonts: BTreeMap<String, String>,
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let cache = Cache::new(device);
        let viewport = GlyphonViewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer =
            GlyphonRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            viewport,
            atlas,
            renderer,
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

    /// How much of the viewport one string covers, in physical pixels.
    ///
    /// `None` for an unbound font, which is what [`Self::draw`] does with one
    /// too: a string whose face never arrived is not drawn, so it covers
    /// nothing and there is nothing to hit-test against.
    ///
    /// The width is the widest laid-out line rather than the box the text was
    /// laid out *into*: glyphon is given the whole viewport to wrap in, so the
    /// box is the viewport and would answer "yes" to every point in it. The
    /// height is the lines it actually used.
    ///
    /// This exists because an editor cannot otherwise say where a string is.
    /// Every other drawn thing has a size in the scene — a sprite has one, a UI
    /// image has one — and a string's is decided by glyph layout inside this
    /// module. A guessed box picks the wrong thing near its edges, which is
    /// worse than not picking at all, so the answer comes from the same
    /// shaping the frame is drawn from.
    pub fn measure(&mut self, instance: &TextInstance, viewport: Viewport) -> Option<[f32; 2]> {
        let family = self.fonts.get(instance.font()).cloned()?;
        let buffer = self.shape(instance, &family, viewport);
        Some(laid_out(&buffer, instance.line_height()))
    }

    /// One instance laid out, the only place that is decided.
    ///
    /// Shared by drawing and measuring rather than written twice, because two
    /// copies is exactly how a pick box ends up disagreeing with the picture it
    /// is meant to be over.
    fn shape(&mut self, instance: &TextInstance, family: &str, viewport: Viewport) -> Buffer {
        let mut buffer = Buffer::new(
            &mut self.font_system,
            Metrics::new(instance.font_size(), instance.line_height()),
        );
        let width = f32::from(u16::try_from(viewport.width).unwrap_or(u16::MAX));
        let height = f32::from(u16::try_from(viewport.height).unwrap_or(u16::MAX));
        buffer.set_size(Some(width), Some(height));
        buffer.set_text(
            instance.text(),
            &Attrs::new().family(Family::Name(family)),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer
    }

    /// Shapes and draws one ordered text pass into an existing frame target.
    ///
    /// An unbound font skips its string. Asset diagnostics name it separately;
    /// silently choosing a machine font here would make a browser and desktop
    /// disagree while both appeared to work.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: Viewport,
        instances: &[TextInstance],
    ) -> Result<(), TextError> {
        self.viewport.update(
            queue,
            Resolution {
                width: viewport.width,
                height: viewport.height,
            },
        );

        let resolved: Vec<(&TextInstance, String)> = instances
            .iter()
            .filter_map(|instance| {
                self.fonts
                    .get(instance.font())
                    .cloned()
                    .map(|family| (instance, family))
            })
            .collect();
        let mut buffers = Vec::with_capacity(resolved.len());
        for (instance, family) in &resolved {
            buffers.push(self.shape(instance, family, viewport));
        }
        let areas = buffers
            .iter()
            .zip(resolved.iter())
            .map(|(buffer, (instance, _))| {
                let [width, height] = laid_out(buffer, instance.line_height());
                let [across, down] = instance.align();
                TextArea {
                    buffer,
                    left: instance.position()[0] + across.offset(width),
                    top: instance.position()[1] + down.offset(height),
                    scale: 1.0,
                    bounds: TextBounds {
                        left: i32::try_from(viewport.x).unwrap_or(i32::MAX),
                        top: i32::try_from(viewport.y).unwrap_or(i32::MAX),
                        right: i32::try_from(viewport.x.saturating_add(viewport.width))
                            .unwrap_or(i32::MAX),
                        bottom: i32::try_from(viewport.y.saturating_add(viewport.height))
                            .unwrap_or(i32::MAX),
                    },
                    default_color: glyphon_color(instance.color()),
                    custom_glyphs: &[],
                }
            });
        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash_cache,
            )
            .map_err(|error| TextError::Prepare(error.to_string()))?;

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sindri text pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .map_err(|error| TextError::Render(error.to_string()))?;
        }
        self.atlas.trim();
        Ok(())
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn glyphon_color(color: [f32; 4]) -> Color {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgba(
        channel(color[0]),
        channel(color[1]),
        channel(color[2]),
        channel(color[3]),
    )
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
    #[error("could not prepare text: {0}")]
    Prepare(String),
    #[error("could not render text: {0}")]
    Render(String),
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
                20.0,
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
                16.0,
                20.0,
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
        assert!((TextAlign::Start.offset(120.0) - 0.0).abs() < f32::EPSILON);
        assert!((TextAlign::Middle.offset(120.0) + 60.0).abs() < f32::EPSILON);
        assert!((TextAlign::End.offset(120.0) + 120.0).abs() < f32::EPSILON);
    }

    /// A string of no width sits at its point however it is aligned, so an
    /// empty label does not jump about.
    #[test]
    fn nothing_is_in_the_same_place_whichever_end_it_is_measured_from() {
        for align in [TextAlign::Start, TextAlign::Middle, TextAlign::End] {
            assert!(align.offset(0.0).abs() < f32::EPSILON, "{align:?}");
        }
    }
}
