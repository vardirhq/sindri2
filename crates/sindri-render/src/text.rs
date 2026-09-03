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
        })
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
        let mut width = 0.0_f32;
        let mut lines = 0.0_f32;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            lines += 1.0;
        }
        Some([width, lines * instance.line_height()])
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
            .map(|(buffer, (instance, _))| TextArea {
                buffer,
                left: instance.position()[0],
                top: instance.position()[1],
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
    use super::{TextError, TextInstance};

    #[test]
    fn text_instances_refuse_values_a_gpu_cannot_place() {
        assert!(matches!(
            TextInstance::new("hello", "font.ttf", [0.0, 0.0], 0.0, 20.0, [1.0; 4]),
            Err(TextError::InvalidFontSize(0.0))
        ));
        assert!(matches!(
            TextInstance::new("hello", "font.ttf", [f32::NAN, 0.0], 16.0, 20.0, [1.0; 4]),
            Err(TextError::NonFinitePosition(_))
        ));
    }
}
