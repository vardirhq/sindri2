use glam::Mat4;
use thiserror::Error;

use crate::{SpriteInstance, TextureId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn aspect_ratio(self) -> Result<f32, FramePlanError> {
        if self.width == 0 || self.height == 0 {
            return Err(FramePlanError::EmptyViewport);
        }
        let width = u16::try_from(self.width).map_err(|_| FramePlanError::ViewportTooLarge)?;
        let height = u16::try_from(self.height).map_err(|_| FramePlanError::ViewportTooLarge)?;
        Ok(f32::from(width) / f32::from(height))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClearOperations {
    pub color: [f64; 4],
    pub depth: f32,
}

impl Default for ClearOperations {
    fn default() -> Self {
        Self {
            color: [0.018, 0.025, 0.045, 1.0],
            depth: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderStage {
    Opaque3d,
    Transparent2d,
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderLayer(pub i32);

impl RenderLayer {
    pub const WORLD: Self = Self(0);
    pub const OVERLAY: Self = Self(100);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameCamera {
    pub view_projection: Mat4,
}

#[derive(Clone, Debug)]
pub enum FrameCommand {
    TexturedCube {
        model: Mat4,
        texture: TextureId,
    },
    /// One batch per texture: instances sharing a texture draw in a single call.
    SpriteBatch {
        texture: TextureId,
        instances: Vec<SpriteInstance>,
    },
}

#[derive(Clone, Debug)]
pub struct FramePass {
    pub stage: RenderStage,
    pub layer: RenderLayer,
    pub camera: FrameCamera,
    pub command: FrameCommand,
    insertion_order: usize,
}

impl FramePass {
    pub const fn new(
        stage: RenderStage,
        layer: RenderLayer,
        camera: FrameCamera,
        command: FrameCommand,
    ) -> Self {
        Self {
            stage,
            layer,
            camera,
            command,
            insertion_order: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExtractedFrame {
    viewport: Viewport,
    clear: ClearOperations,
    passes: Vec<FramePass>,
}

impl ExtractedFrame {
    pub const fn new(viewport: Viewport, clear: ClearOperations) -> Self {
        Self {
            viewport,
            clear,
            passes: Vec::new(),
        }
    }

    pub fn push(&mut self, mut pass: FramePass) {
        pass.insertion_order = self.passes.len();
        self.passes.push(pass);
    }

    pub fn prepare(mut self) -> Result<PreparedFrame, FramePlanError> {
        self.viewport.aspect_ratio()?;
        if !self.clear.color.into_iter().all(f64::is_finite) || !self.clear.depth.is_finite() {
            return Err(FramePlanError::NonFiniteClearValue);
        }
        self.passes
            .sort_by_key(|pass| (pass.stage, pass.layer, pass.insertion_order));
        Ok(PreparedFrame {
            viewport: self.viewport,
            clear: self.clear,
            passes: self.passes,
        })
    }
}

#[derive(Clone, Debug)]
pub struct PreparedFrame {
    viewport: Viewport,
    clear: ClearOperations,
    passes: Vec<FramePass>,
}

impl PreparedFrame {
    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub const fn clear(&self) -> ClearOperations {
        self.clear
    }

    pub fn passes(&self) -> &[FramePass] {
        &self.passes
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FramePlanError {
    #[error("render viewport width and height must be non-zero")]
    EmptyViewport,
    #[error("render viewport dimensions exceed the supported 65535-pixel limit")]
    ViewportTooLarge,
    #[error("render clear values must be finite")]
    NonFiniteClearValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(stage: RenderStage, layer: RenderLayer) -> FramePass {
        FramePass::new(
            stage,
            layer,
            FrameCamera {
                view_projection: Mat4::IDENTITY,
            },
            FrameCommand::TexturedCube {
                model: Mat4::IDENTITY,
                texture: crate::TextureRegistry::MISSING,
            },
        )
    }

    #[test]
    fn preparation_orders_stage_then_layer_stably() {
        let mut frame = ExtractedFrame::new(Viewport::new(640, 360), ClearOperations::default());
        frame.push(pass(RenderStage::Overlay, RenderLayer(100)));
        frame.push(pass(RenderStage::Opaque3d, RenderLayer(5)));
        frame.push(pass(RenderStage::Opaque3d, RenderLayer(1)));
        frame.push(pass(RenderStage::Opaque3d, RenderLayer(1)));

        let prepared = frame.prepare().unwrap();
        let order: Vec<_> = prepared
            .passes()
            .iter()
            .map(|pass| (pass.stage, pass.layer))
            .collect();
        assert_eq!(
            order,
            vec![
                (RenderStage::Opaque3d, RenderLayer(1)),
                (RenderStage::Opaque3d, RenderLayer(1)),
                (RenderStage::Opaque3d, RenderLayer(5)),
                (RenderStage::Overlay, RenderLayer(100)),
            ]
        );
    }

    #[test]
    fn preparation_rejects_empty_viewports() {
        let frame = ExtractedFrame::new(Viewport::new(0, 360), ClearOperations::default());
        assert_eq!(frame.prepare().unwrap_err(), FramePlanError::EmptyViewport);
    }

    #[test]
    fn viewport_reports_aspect_ratio() {
        let aspect = Viewport::new(1920, 1080).aspect_ratio().unwrap();
        assert!((aspect - 16.0 / 9.0).abs() <= f32::EPSILON);
    }
}
