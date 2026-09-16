//! One sprite as the GPU sees it, and where it sits in draw order.

use glam::Mat4;

use crate::UvRect;

/// What a batch of sprites does about the depth the opaque stage wrote.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpriteDepth {
    /// Draws over the world whatever the depth buffer holds.
    #[default]
    Ignore,
    /// Hidden by opaque geometry nearer the camera.
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
    color_multiply: [f32; 4],
    color_offset: [f32; 4],
}

impl SpriteInstance {
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

    /// A sprite drawn from the whole of its texture with identity colour math.
    pub fn new(model: Mat4, tint: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            tint,
            uv_rect: UvRect::FULL.to_array(),
            color_multiply: [1.0; 4],
            color_offset: [0.0; 4],
        }
    }

    /// Draws only part of the texture, which is what a sprite sheet is.
    #[must_use]
    pub fn with_uv_rect(mut self, uv_rect: UvRect) -> Self {
        self.uv_rect = uv_rect.to_array();
        self
    }

    /// Applies advanced per-channel colour math without changing batch keys.
    #[must_use]
    pub const fn with_color_transform(
        mut self,
        multiply: [f32; 4],
        offset: [f32; 4],
    ) -> Self {
        self.color_multiply = multiply;
        self.color_offset = offset;
        self
    }

    pub fn uv_rect(self) -> UvRect {
        UvRect::from_array(self.uv_rect)
    }

    pub const fn tint(self) -> [f32; 4] {
        self.tint
    }

    pub const fn color_multiply(self) -> [f32; 4] {
        self.color_multiply
    }

    pub const fn color_offset(self) -> [f32; 4] {
        self.color_offset
    }

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
