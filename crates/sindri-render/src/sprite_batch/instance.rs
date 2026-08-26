//! One sprite as the GPU sees it, and where it sits in draw order.

use glam::Mat4;

use crate::UvRect;

/// What a batch of sprites does about the depth the opaque stage wrote.
///
/// Sprites never write depth under either of these: blending is order
/// dependent, so a depth write would make the result depend on draw order
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
}

impl SpriteDepth {
    pub(super) const fn compare(self) -> wgpu::CompareFunction {
        match self {
            Self::Ignore => wgpu::CompareFunction::Always,
            Self::Test => wgpu::CompareFunction::Less,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
    uv_rect: [f32; 4],
}

impl SpriteInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4
    ];

    /// A sprite drawn from the whole of its texture.
    pub fn new(model: Mat4, tint: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            tint,
            uv_rect: UvRect::FULL.to_array(),
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

    /// The part of the texture this instance draws.
    pub fn uv_rect(self) -> UvRect {
        UvRect::from_array(self.uv_rect)
    }

    /// The per-instance tint in straight `[r, g, b, a]` order.
    pub const fn tint(self) -> [f32; 4] {
        self.tint
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
