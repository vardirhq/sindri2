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
    SpriteBatchStats, TextError, TextRenderer, TextureRegistry, TexturedCubeRenderer, encode_clear,
};
use thiserror::Error;

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
    pub text: &'a mut TextRenderer,
    pub textures: &'a TextureRegistry,
}

/// Draws every pass of `frame`, in the order preparation put them in.
///
/// One call is one submission. Each sprite batch writes its own buffers, and
/// `queue.write_buffer` stages those writes until the queue is next submitted,
/// so `encoder` must be submitted before this is called again with the same
/// renderers. A host drawing two frames at once — the editor's scene view and
/// game view — submits between them.
pub fn encode_prepared_frame(
    renderers: FrameRenderers<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: FrameTarget<'_>,
    frame: &PreparedFrame,
) -> Result<SpriteBatchStats, FrameEncodeError> {
    let FrameRenderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        text: text_renderer,
        textures,
    } = renderers;
    // Once, before anything draws. The frame owns what it starts as; a scene
    // with two meshes would otherwise have the second clear away the first, and
    // a scene with none would leave its sprites drawing against a depth buffer
    // nothing had filled.
    encode_clear(encoder, target.color, target.depth, frame.clear());
    // Each batch draws from its own slot, and this is what hands the first one
    // back at the start of every submission. Without it a host would allocate a
    // slot per batch per frame for as long as it ran.
    sprite_renderer.begin_submission();
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
                sprite_renderer.draw(
                    device,
                    queue,
                    encoder,
                    target.color,
                    target.depth,
                    textures,
                    *texture,
                    pass.camera.view_projection,
                    *depth,
                    instances,
                )?;
            }
            FrameCommand::Text { instances } => {
                text_renderer.draw(
                    device,
                    queue,
                    encoder,
                    target.color,
                    frame.viewport(),
                    instances,
                )?;
            }
        }
    }
    Ok(sprite_renderer.stats())
}

#[derive(Debug, Error)]
pub enum FrameEncodeError {
    #[error(transparent)]
    Sprites(#[from] SpriteBatchError),
    #[error(transparent)]
    Text(#[from] TextError),
}
