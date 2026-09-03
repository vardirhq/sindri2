//! One glyph as the GPU sees it.

use glam::Mat4;

use crate::UvRect;

/// A single glyph quad: where it is, what colour it is, and how its field is
/// read.
///
/// Deliberately not a `SpriteInstance` with extra fields. A sprite is a picture
/// multiplied by a tint; a glyph is a distance field turned into an edge, an
/// outline and a softness. Sharing the type would mean every sprite in every
/// scene carrying three vectors it has no use for.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphInstance {
    model: [[f32; 4]; 4],
    face: [f32; 4],
    outline: [f32; 4],
    uv_rect: [f32; 4],
    shape: [f32; 4],
}

impl GlyphInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4,
        9 => Float32x4
    ];

    /// A glyph drawn from `uv` in the atlas, in the colour `face`.
    ///
    /// `outline_width` and `softness` are in the field's own stored units,
    /// which is what the shader compares against — see
    /// [`super::field_per_raster_pixel`] for what turns a width on screen into
    /// one of them.
    #[must_use]
    pub fn new(model: Mat4, uv: UvRect, face: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            face,
            outline: [0.0; 4],
            uv_rect: uv.to_array(),
            shape: [0.0; 4],
        }
    }

    /// Draws a stroke of `width` field units around the glyph, in `color`.
    ///
    /// A width of zero puts the outline's threshold on the glyph's own edge, so
    /// the shader's blend between the two collapses to the face alone — which is
    /// why an unset outline costs nothing rather than needing a second pipeline.
    #[must_use]
    pub const fn with_outline(mut self, width: f32, color: [f32; 4]) -> Self {
        self.shape[0] = width;
        self.outline = color;
        self
    }

    /// Widens the edge by `softness` field units, which is what makes a drop
    /// shadow a shadow rather than a second copy of the letter.
    #[must_use]
    pub const fn with_softness(mut self, softness: f32) -> Self {
        self.shape[1] = softness;
        self
    }

    /// Says the atlas holds this glyph's own colours rather than a field, so it
    /// is drawn as the picture it is. An emoji face.
    #[must_use]
    pub const fn colored(mut self) -> Self {
        self.shape[2] = 1.0;
        self
    }

    /// The per-instance model transform.
    #[must_use]
    pub fn model(self) -> Mat4 {
        Mat4::from_cols_array_2d(&self.model)
    }

    /// The colour the glyph's own body is drawn in.
    #[must_use]
    pub const fn face(self) -> [f32; 4] {
        self.face
    }

    /// The part of the atlas this glyph samples.
    #[must_use]
    pub fn uv_rect(self) -> UvRect {
        UvRect::from_array(self.uv_rect)
    }

    #[must_use]
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GlyphUniform {
    pub(super) view_projection: [[f32; 4]; 4],
}
