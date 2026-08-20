//! Turning a prepared frame into GPU commands.
//!
//! This is the last stage of the three the frame pipeline describes: extraction
//! derives a frame from a world, preparation validates and orders it, and this
//! draws it. It knows nothing about worlds, components, or scenes — a prepared
//! frame is a list of commands and the matrices to draw them with, which is
//! exactly what makes it belong here rather than in whichever host happened to
//! need it first.
//!
//! It lived in the cube example for a release, which made the editor depend on
//! an example to draw anything at all.

use crate::{
    DepthTarget, DrawContext, FrameCommand, PreparedFrame, SpriteBatchError, SpriteBatchRenderer,
    SpriteBatchStats, TextureRegistry, TexturedCubeRenderer, encode_clear,
};

/// Where a frame is drawn.
#[derive(Clone, Copy)]
pub struct FrameTarget<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a DepthTarget,
}

/// The renderers a frame is drawn with.
pub struct FrameRenderers<'a> {
    pub cube: &'a mut TexturedCubeRenderer,
    pub sprites: &'a mut SpriteBatchRenderer,
    pub textures: &'a TextureRegistry,
}

/// Draws every pass of `frame`, in the order preparation put them in.
pub fn encode_prepared_frame(
    renderers: FrameRenderers<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: FrameTarget<'_>,
    frame: &PreparedFrame,
) -> Result<SpriteBatchStats, SpriteBatchError> {
    let FrameRenderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        textures,
    } = renderers;
    let mut sprite_stats = SpriteBatchStats::default();
    // Once, before anything draws. The frame owns what it starts as; a scene
    // with two meshes would otherwise have the second clear away the first, and
    // a scene with none would leave its sprites drawing against a depth buffer
    // nothing had filled.
    encode_clear(encoder, target.color, target.depth, frame.clear());
    for pass in frame.passes() {
        match &pass.command {
            FrameCommand::TexturedCube { model, texture } => cube_renderer.encode(
                DrawContext {
                    device,
                    queue,
                    textures,
                    texture: *texture,
                },
                encoder,
                target.color,
                target.depth,
                pass.camera.view_projection * *model,
            ),
            FrameCommand::SpriteBatch {
                texture,
                depth,
                instances,
            } => {
                sprite_stats =
                    sprite_renderer.prepare(device, queue, textures, *texture, instances)?;
                sprite_renderer.encode(
                    queue,
                    encoder,
                    target.color,
                    target.depth,
                    pass.camera.view_projection,
                    *depth,
                );
            }
        }
    }
    Ok(sprite_stats)
}
