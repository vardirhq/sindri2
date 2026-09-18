//! A tile volume drawn as solid blocks, from four sides.
//!
//! The question this exists to answer is whether a volume can be a *shape*
//! rather than a picture of one. The other path arranges flat quads for one
//! fixed viewpoint, so it has exactly one correct camera; this one builds the
//! cells as boxes and lets the depth buffer decide what covers what, which
//! should mean every camera is correct and none of them is special.
//!
//! So it renders the same blocks from four corners. If the idea works, the
//! four pictures are four views of one object.
//!
//! ```bash
//! cargo run -p voxel-lab --bin voxel-lab-capture -- target/render-artifacts
//! ```

use std::{
    error::Error,
    f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI},
    fs,
    io::BufWriter,
    path::Path,
};

use glam::Vec3;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::{AssetId, SpriteRef, SpriteSheetDocument, TileSetDocument};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    ClearOperations, DepthTarget, ExtractedFrame, FrameCamera, FrameCommand, FramePass,
    FrameRenderers, FrameTarget, GlyphRenderer, OffscreenTarget, RenderLayer, RenderStage,
    ShapeRenderer, SpriteBatchRenderer, SpriteDepth, SpriteInstance, TextRenderer, Texture2D,
    TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame, look_at,
    orthographic_projection,
};
use sindri_scene::{TextureBindings, TileVolumeComponent, cube_faces};

const WIDTH: u32 = 900;
const HEIGHT: u32 = 700;
/// Half the height the orthographic camera shows, in cells.
const ZOOM: f32 = 5.5;
/// Classic 2:1 isometric: a 45° yaw, and a pitch whose sine is a half.
const PITCH: f32 = FRAC_PI_6;

/// A few blocks worth looking at: ground, a stack, an overhang and a slab.
///
/// The overhang is the interesting one. A flat map cannot express it at all,
/// and the painter-order path draws it by arranging quads for one viewpoint —
/// so if it survives being walked round, the geometry is real.
const CELLS: &str = r#"{
  "tileset": "blocks.tileset.json",
  "cells": [
    { "position": [0, 0, 0], "tile": "grass" }, { "position": [1, 0, 0], "tile": "grass" },
    { "position": [2, 0, 0], "tile": "grass" }, { "position": [3, 0, 0], "tile": "grass" },
    { "position": [0, 1, 0], "tile": "grass" }, { "position": [1, 1, 0], "tile": "earth" },
    { "position": [2, 1, 0], "tile": "earth" }, { "position": [3, 1, 0], "tile": "grass" },
    { "position": [0, 2, 0], "tile": "grass" }, { "position": [1, 2, 0], "tile": "earth" },
    { "position": [2, 2, 0], "tile": "earth" }, { "position": [3, 2, 0], "tile": "grass" },
    { "position": [0, 3, 0], "tile": "grass" }, { "position": [1, 3, 0], "tile": "grass" },
    { "position": [2, 3, 0], "tile": "grass" }, { "position": [3, 3, 0], "tile": "grass" },

    { "position": [1, 1, 1], "tile": "stone" }, { "position": [1, 2, 1], "tile": "stone" },
    { "position": [1, 1, 2], "tile": "stone" },
    { "position": [2, 1, 2], "tile": "stone" },
    { "position": [3, 1, 2], "tile": "stone" },

    { "position": [3, 3, 1], "tile": "slab" }
  ]
}"#;

/// Tops from one bake, sides from the other, and a slab that fills half a cell.
fn tile_set() -> TileSetDocument {
    // `art` is which baked block the faces come from, so a slab can be stone
    // without the sheets needing a second copy of stone at half the height:
    // what makes it a slab is the cell it fills, not the picture.
    let tile = |name: &str, art: &str, height: &str| {
        format!(
            r#""{name}": {{ {height} "faces": {{
                 "top":    {{ "sprite": "textures/block-tops.png#{art}",  "size": [1.0, 1.0] }},
                 "bottom": {{ "sprite": "textures/block-tops.png#{art}",  "size": [1.0, 1.0] }},
                 "south":  {{ "sprite": "textures/block-sides.png#{art}", "size": [1.0, 1.0] }},
                 "east":   {{ "sprite": "textures/block-sides.png#{art}", "size": [1.0, 1.0] }}
               }} }}"#
        )
    };
    TileSetDocument::from_json(&format!(
        r#"{{ "format_version": 1, "tiles": {{
             {}, {}, {}, {}
           }} }}"#,
        tile("grass", "grass", ""),
        tile("earth", "earth", ""),
        tile("stone", "stone", ""),
        // Half a cell, so it reads as a step rather than a block.
        tile("slab", "stone", r#""height": 0.5,"#)
    ))
    .expect("the tile set parses")
}

fn bind_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    registry: &mut TextureRegistry,
) -> Result<TextureBindings, Box<dyn Error>> {
    let mut bindings = TextureBindings::new();
    for name in ["block-tops", "block-sides"] {
        let root = Path::new("games/voxel-lab/assets/textures");
        let png = fs::read(root.join(format!("{name}.png")))?;
        let asset = TextureAssetDecoder.decode(AssetBytes::new(
            format!("textures/{name}.png").parse::<AssetId>()?,
            png,
        ))?;
        let texture = Texture2D::from_rgba8(
            device,
            queue,
            name,
            asset.width(),
            asset.height(),
            asset.rgba8(),
        )?;
        let reference = format!("textures/{name}.png");
        bindings.bind(&reference, registry.insert(texture));
        let sheet = fs::read_to_string(root.join(format!("{name}.sheet.json")))?;
        bindings.bind_sheet(&reference, &SpriteSheetDocument::from_json(&sheet)?)?;
    }
    Ok(bindings)
}

/// The volume's faces, as instances grouped by the texture they draw from.
fn instances(textures: &TextureBindings) -> Vec<(sindri_render::TextureId, Vec<SpriteInstance>)> {
    let volume: TileVolumeComponent = serde_json::from_str(CELLS).expect("the volume parses");
    let faces = cube_faces(&volume, &tile_set(), [1.0, 1.0, 1.0]).expect("the cells resolve");
    let mut batches: std::collections::BTreeMap<String, (sindri_render::TextureId, Vec<_>)> =
        std::collections::BTreeMap::new();
    for face in faces {
        let reference = SpriteRef::parse(&face.sprite).expect("a sprite reference");
        let (texture, rect) = textures.resolve_sprite(&reference);
        // The face's own light, as a tint on an otherwise unshaded texture.
        // Shading a block here rather than in its art is what lets the camera
        // move: baked light belongs to the angle it was baked from.
        let shade = [face.shade, face.shade, face.shade, 1.0];
        batches
            .entry(reference.texture().to_owned())
            .or_insert_with(|| (texture, Vec::new()))
            .1
            .push(SpriteInstance::new(face.model, shade).with_uv_rect(rect));
    }
    batches.into_values().collect()
}

/// The viewport's shape, which the orthographic box has to match or the blocks
/// come out stretched.
#[allow(clippy::cast_precision_loss)]
fn aspect_ratio() -> f32 {
    WIDTH as f32 / HEIGHT as f32
}

/// Where the camera stands for one of the four corners.
fn camera(turn: f32) -> FrameCamera {
    let yaw = FRAC_PI_4 + turn;
    let centre = Vec3::new(1.5, 1.5, 0.75);
    let distance = 20.0;
    let eye = centre
        + Vec3::new(
            yaw.cos() * PITCH.cos(),
            yaw.sin() * PITCH.cos(),
            PITCH.sin(),
        ) * distance;
    let aspect = aspect_ratio();
    FrameCamera {
        view_projection: orthographic_projection(
            -ZOOM * aspect,
            ZOOM * aspect,
            -ZOOM,
            ZOOM,
            0.1,
            100.0,
        ) * look_at(eye, centre, Vec3::Z),
    }
}

fn frame(
    camera: FrameCamera,
    batches: &[(sindri_render::TextureId, Vec<SpriteInstance>)],
) -> ExtractedFrame {
    let mut extracted = ExtractedFrame::new(
        Viewport::new(WIDTH, HEIGHT),
        ClearOperations {
            color: [0.09, 0.10, 0.13, 1.0],
            depth: 1.0,
        },
    );
    for (texture, instances) in batches {
        extracted.push(FramePass::new(
            // Blocks are opaque surfaces, so they belong in the stage that
            // writes depth rather than among the sorted transparent sprites.
            RenderStage::Opaque3d,
            RenderLayer(0),
            camera,
            FrameCommand::SpriteBatch {
                texture: *texture,
                // The whole point: these write depth, and the GPU decides what
                // covers what.
                depth: SpriteDepth::Write,
                instances: instances.clone(),
            },
        ));
    }
    extracted
}

pub async fn capture(out: &Path) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut registry = TextureRegistry::new(&gpu.device, &gpu.queue);
    let textures = bind_textures(&gpu.device, &gpu.queue, &mut registry)?;
    let batches = instances(&textures);
    let mut cube = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new();
    let mut glyphs = GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut shapes = ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);

    fs::create_dir_all(out)?;
    // Four corners, a quarter turn apart. If the idea works these are four
    // views of one object rather than four pictures that happen to share a
    // subject.
    for (index, turn) in [0.0, FRAC_PI_2, PI, PI + FRAC_PI_2].into_iter().enumerate() {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("voxel lab"),
            });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut cube,
                sprites: &mut sprites,
                text: &mut text,
                glyphs: &mut glyphs,
                shapes: &mut shapes,
                textures: &registry,
            },
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            FrameTarget {
                color: target.view(),
                depth: &depth,
            },
            &frame(camera(turn), &batches).prepare()?,
        )?;
        let readback = target.copy_to_buffer(&gpu.device, &mut encoder)?;
        gpu.queue.submit([encoder.finish()]);
        let pixels = readback.read_rgba8(&gpu.device)?;
        let path = out.join(format!("voxel-lab-{index}.png"));
        let file = fs::File::create(&path)?;
        let mut png = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()?.write_image_data(&pixels)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
