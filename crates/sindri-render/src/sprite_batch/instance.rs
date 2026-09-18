//! One sprite as the GPU sees it, and where it sits in draw order.

use glam::Mat4;

use crate::UvRect;

/// What a batch of sprites does about the depth the opaque stage wrote.
///
/// Blending is order dependent, so a blended batch never writes: a depth write
/// from one would make the result depend on draw order
/// twice. What differs is whether something in front can hide them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpriteDepth {
    /// Draws over the world whatever the depth buffer holds. A screen-space
    /// overlay is not in the world, so nothing in the world may occlude it.
    #[default]
    Ignore,
    /// Hidden by opaque geometry nearer the camera, which is what being in the
    /// world means.
    Test,
    /// Opaque geometry: hidden by what is nearer, and hiding what is further.
    ///
    /// The one mode that writes. A quad that writes depth is not a sprite in
    /// the painter's sense any more -- it is a surface, and what covers what
    /// stops being a sort order the extractor has to get right and becomes an
    /// answer the depth buffer already holds. Blocks are drawn this way, which
    /// is what lets a camera go round them.
    ///
    /// Writing and blending do not mix: a blended pixel's result depends on
    /// what was drawn before it, so writing depth from one would make the
    /// picture depend on draw order in exactly the way depth is meant to stop.
    /// This mode draws opaque and discards the fully transparent, which is how
    /// a cutout texture keeps its shape without putting holes in the depth it
    /// leaves behind.
    Write,
}

impl SpriteDepth {
    pub(super) const fn compare(self) -> wgpu::CompareFunction {
        match self {
            Self::Ignore => wgpu::CompareFunction::Always,
            Self::Test | Self::Write => wgpu::CompareFunction::Less,
        }
    }

    /// Whether this mode puts anything into the depth buffer.
    pub(super) const fn writes(self) -> bool {
        matches!(self, Self::Write)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
    uv_rect: [f32; 4],
    color_multiply: [f32; 4],
    color_offset: [f32; 4],
    corner_shade: [f32; 4],
}

impl SpriteInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4,
        9 => Float32x4,
        10 => Float32x4
    ];

    /// A sprite drawn from the whole of its texture with identity colour math.
    pub fn new(model: Mat4, tint: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            tint,
            uv_rect: UvRect::FULL.to_array(),
            color_multiply: [1.0; 4],
            color_offset: [0.0; 4],
            corner_shade: [1.0; 4],
        }
    }

    /// Draws only part of the texture, which is what a sprite sheet is.
    ///
    /// Per instance rather than per batch, deliberately: a sheet's whole point
    /// is many different frames of one texture, and if the rect belonged to the
    /// batch then every frame would be its own draw call — which is the cost the
    /// sheet exists to avoid.
    #[must_use]
    pub fn with_uv_rect(mut self, uv_rect: UvRect) -> Self {
        self.uv_rect = uv_rect.to_array();
        self
    }

    /// How much light reaches each corner of the quad, from its own corner
    /// outward: `[0, 0]`, `[1, 0]`, `[1, 1]`, `[0, 1]` in the quad's own space.
    ///
    /// One value a corner rather than one a quad, because the thing being
    /// described happens at corners: where two blocks meet, the crease between
    /// them is darker than either face's middle. A flat tint per face cannot
    /// say that, and without it a stack of cubes reads as a flat arrangement of
    /// lit shapes rather than as solid things touching.
    #[must_use]
    pub const fn with_corner_shade(mut self, corners: [f32; 4]) -> Self {
        self.corner_shade = corners;
        self
    }

    /// Applies advanced per-channel colour math without changing batch keys.
    #[must_use]
    pub const fn with_color_transform(mut self, multiply: [f32; 4], offset: [f32; 4]) -> Self {
        self.color_multiply = multiply;
        self.color_offset = offset;
        self
    }

    /// The part of the texture this instance draws.
    pub fn uv_rect(self) -> UvRect {
        UvRect::from_array(self.uv_rect)
    }

    /// The per-instance tint in straight `[r, g, b, a]` order.
    pub const fn tint(self) -> [f32; 4] {
        self.tint
    }

    /// The per-channel multiplier applied after the tint.
    pub const fn color_multiply(self) -> [f32; 4] {
        self.color_multiply
    }

    /// The per-channel offset added after the multiply.
    pub const fn color_offset(self) -> [f32; 4] {
        self.color_offset
    }

    /// The per-instance model transform.
    pub fn model(self) -> Mat4 {
        Mat4::from_cols_array_2d(&self.model)
    }

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
pub(super) struct BatchUniform {
    pub(super) view_projection: [[f32; 4]; 4],
}
