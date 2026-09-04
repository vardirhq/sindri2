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
    Bloom, BloomSettings, DepthTarget, DrawContext, FrameCommand, FramePass, GlyphDrawError,
    GlyphRenderer, PreparedFrame, RenderStage, ShapeDrawError, ShapeRenderer, SpriteBatchError,
    SpriteBatchRenderer, SpriteBatchStats, TextError, TextRenderer, TextureRegistry,
    TexturedCubeRenderer, encode_clear,
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
    /// Shapes strings and owns the glyph atlas.
    pub text: &'a mut TextRenderer,
    /// Draws what the text renderer shaped. Two objects because they are two
    /// jobs: one turns words into quads and needs no GPU to do it, the other
    /// puts quads on a screen and knows nothing about words.
    pub glyphs: &'a mut GlyphRenderer,
    /// Draws evaluated shapes: rings, arcs, polygons, grids.
    pub shapes: &'a mut ShapeRenderer,
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
        glyphs: glyph_renderer,
        shapes: shape_renderer,
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
    glyph_renderer.begin_submission();
    shape_renderer.begin_submission();
    encode_passes(
        &mut Renderers {
            cube: cube_renderer,
            sprites: sprite_renderer,
            text: text_renderer,
            glyphs: glyph_renderer,
            shapes: shape_renderer,
            textures,
        },
        device,
        queue,
        encoder,
        target,
        frame.passes().iter(),
    )?;
    Ok(sprite_renderer.stats())
}

/// Draws a frame with its world lit, and its interface drawn crisply on top.
///
/// The ordering is the whole point. A post-process glow applied to everything
/// takes the interface with it: dark detail sitting inside a bright field —
/// black lettering on a bright button — comes back grey, because the blur fills
/// the letterforms in from the brightness all around them. Nothing tunable
/// fixes that, because a flat bright fill is *brighter* than the thin strokes
/// the glow exists for, so no threshold separates them.
///
/// So the world is drawn into the bloom's own target and lit, and the overlay
/// is drawn afterwards straight onto the result. The game glows; the readings
/// over it stay sharp.
///
/// `bloom` must have been sized to the same viewport as `depth`, and the
/// renderers must have been built for the format `bloom` writes — the scene
/// target and the final target share it, so the same pipelines serve both.
pub fn encode_lit_frame(
    renderers: FrameRenderers<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: FrameTarget<'_>,
    frame: &PreparedFrame,
    lighting: Lighting<'_>,
) -> Result<SpriteBatchStats, FrameEncodeError> {
    let Lighting { bloom, settings } = lighting;
    let FrameRenderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        text: text_renderer,
        glyphs: glyph_renderer,
        shapes: shape_renderer,
        textures,
    } = renderers;
    let Some(scene) = bloom.scene_view().cloned() else {
        // Never sized, so there is nowhere to draw the world. Falling back to
        // an unlit frame beats drawing nothing at all.
        return encode_prepared_frame(
            FrameRenderers {
                cube: cube_renderer,
                sprites: sprite_renderer,
                text: text_renderer,
                glyphs: glyph_renderer,
                shapes: shape_renderer,
                textures,
            },
            device,
            queue,
            encoder,
            target,
            frame,
        );
    };
    let scene_target = FrameTarget {
        color: &scene,
        depth: target.depth,
    };
    encode_clear(encoder, &scene, target.depth, frame.clear());
    sprite_renderer.begin_submission();
    glyph_renderer.begin_submission();
    shape_renderer.begin_submission();

    let mut borrowed = Renderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        text: text_renderer,
        glyphs: glyph_renderer,
        shapes: shape_renderer,
        textures,
    };
    let is_overlay = |pass: &&FramePass| pass.stage == RenderStage::Overlay;
    encode_passes(
        &mut borrowed,
        device,
        queue,
        encoder,
        scene_target,
        frame.passes().iter().filter(|pass| !is_overlay(pass)),
    )?;
    bloom.resolve(device, queue, encoder, target.color, settings);
    // Onto the composite, not over a cleared target: the lit world is already
    // there and the interface goes on top of it.
    encode_passes(
        &mut borrowed,
        device,
        queue,
        encoder,
        target,
        frame.passes().iter().filter(is_overlay),
    )?;
    Ok(borrowed.sprites.stats())
}

/// What lights a frame.
///
/// A pair rather than two parameters, because they always travel together and
/// a call taking eight loose arguments is one where two of the same type can be
/// swapped without the compiler minding.
pub struct Lighting<'a> {
    pub bloom: &'a mut Bloom,
    pub settings: BloomSettings,
}

/// The renderers, borrowed for more than one target.
///
/// [`FrameRenderers`] owns its borrows for a single call; a frame drawn in two
/// halves needs them twice, and this is what lets the second half have them.
struct Renderers<'a, 'r> {
    cube: &'a mut TexturedCubeRenderer,
    sprites: &'a mut SpriteBatchRenderer,
    text: &'a mut TextRenderer,
    glyphs: &'a mut GlyphRenderer,
    shapes: &'a mut ShapeRenderer,
    textures: &'r TextureRegistry,
}

/// Draws some of a frame's passes to one target.
fn encode_passes<'p>(
    renderers: &mut Renderers<'_, '_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: FrameTarget<'_>,
    passes: impl Iterator<Item = &'p FramePass>,
) -> Result<(), FrameEncodeError> {
    let Renderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        text: text_renderer,
        glyphs: glyph_renderer,
        shapes: shape_renderer,
        textures,
    } = renderers;
    for pass in passes {
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
            // Text is geometry, drawn through the same camera as the pass it
            // belongs to. It keeps a command of its own because its quads
            // sample a distance field rather than a picture — an edge found at
            // the size it is drawn, an outline, a softened shadow — and because
            // the atlas they sample is filled while the frame is being built,
            // so it cannot have been in the registry beforehand.
            FrameCommand::Text { instances } => {
                if let Some(quads) = text_renderer.quads(device, queue, instances)? {
                    glyph_renderer.draw(
                        device,
                        queue,
                        encoder,
                        target.color,
                        target.depth,
                        quads.atlas,
                        quads.generation,
                        pass.camera.view_projection,
                        &quads.instances,
                    )?;
                }
            }
            // Shapes sample nothing, so unlike text there is nothing to fill
            // before they draw and nothing to bind when they do: the pass hands
            // its camera and its instances straight to the pipeline.
            FrameCommand::Shapes { blend, instances } => shape_renderer.draw(
                device,
                queue,
                encoder,
                target.color,
                target.depth,
                *blend,
                pass.camera.view_projection,
                instances,
            )?,
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum FrameEncodeError {
    #[error(transparent)]
    Sprites(#[from] SpriteBatchError),
    #[error(transparent)]
    Text(#[from] TextError),
    #[error(transparent)]
    Glyphs(#[from] GlyphDrawError),
    #[error(transparent)]
    Shapes(#[from] ShapeDrawError),
}
