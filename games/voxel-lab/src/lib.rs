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

use glam::{Mat4, Vec3};
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
use sindri_scene::{
    TextureBindings, TileCellDocument, TileVolumeComponent, VoxelHit, cube_faces, voxel,
};

const WIDTH: u32 = 900;
const HEIGHT: u32 = 700;
/// One cell's box, in the volume's own units. Cubes, here.
pub const CELL: [f32; 3] = [1.0, 1.0, 1.0];
/// How far a click reaches, in cells.
pub const REACH: f32 = 64.0;

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
fn instances(
    volume: &TileVolumeComponent,
    textures: &TextureBindings,
) -> Vec<(sindri_render::TextureId, Vec<SpriteInstance>)> {
    let faces = cube_faces(volume, &tile_set(), CELL).expect("the cells resolve");
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
fn aspect_ratio() -> f32 {
    viewport_width() / viewport_height()
}

#[allow(clippy::cast_precision_loss)]
fn viewport_width() -> f32 {
    WIDTH as f32
}

#[allow(clippy::cast_precision_loss)]
fn viewport_height() -> f32 {
    HEIGHT as f32
}

/// Where the camera stands for one of the four corners.
pub fn camera(turn: f32) -> FrameCamera {
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

/// The ray a click at this pixel sends into the scene.
///
/// The pixel is where the pointer is; the ray is what the world is asked
/// about. Unprojecting the near and far planes and taking the line between
/// them works whatever the camera is doing, which is the point -- an
/// orthographic view has no eye point to draw the ray from.
pub fn ray_through(view_projection: Mat4, pixel: [f32; 2]) -> (Vec3, Vec3) {
    let inverse = view_projection.inverse();
    let x = pixel[0] / viewport_width() * 2.0 - 1.0;
    let y = 1.0 - pixel[1] / viewport_height() * 2.0;
    let near = inverse.project_point3(Vec3::new(x, y, 0.0));
    let far = inverse.project_point3(Vec3::new(x, y, 1.0));
    (near, far - near)
}

/// Where a point in the volume lands on screen, which is the same arithmetic
/// backwards and is how this lab aims a click at a face it means to hit.
pub fn pixel_of(view_projection: Mat4, point: Vec3) -> [f32; 2] {
    let clip = view_projection * point.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    [
        (ndc.x * 0.5 + 0.5) * viewport_width(),
        (0.5 - ndc.y * 0.5) * viewport_height(),
    ]
}

/// What a click does: attach a block against the face clicked, or take away
/// the block itself. No mode, no level, no chosen target -- the face carries
/// all of it.
fn click(volume: &mut TileVolumeComponent, hit: VoxelHit, place: Option<&str>) {
    let cell = match place {
        Some(_) => hit.against(),
        None => hit.cell,
    };
    let at = [cell.x, cell.y, cell.z];
    volume.cells.retain(|existing| existing.position != at);
    if let Some(tile) = place {
        volume.cells.push(TileCellDocument {
            position: at,
            tile: tile.to_owned(),
            visual_override: None,
        });
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
    let mut cube = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new();
    let mut glyphs = GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut shapes = ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut renderers = Renderers {
        cube: &mut cube,
        sprites: &mut sprites,
        text: &mut text,
        glyphs: &mut glyphs,
        shapes: &mut shapes,
        registry: &registry,
    };
    fs::create_dir_all(out)?;

    let mut volume: TileVolumeComponent = serde_json::from_str(CELLS).expect("the volume parses");
    let view = camera(0.0);

    // Four corners first: the same blocks, four ways round.
    for (index, turn) in [0.0, FRAC_PI_2, PI, PI + FRAC_PI_2].into_iter().enumerate() {
        let batches = instances(&volume, &textures);
        let frame = frame(camera(turn), &batches);
        shoot(
            &gpu,
            &target,
            &depth,
            &mut renderers,
            frame,
            &out.join(format!("voxel-lab-{index}.png")),
        )?;
    }

    // Then a session: every step is a pixel, a ray, and what the face it hit
    // says to do. Aimed at the centre of a face that is known to be there, so
    // a click that lands somewhere else is a bug rather than a bad aim.
    let clicks: [(Vec3, Option<&str>, &str); 3] = [
        // The top of the stack: stack another on it.
        (Vec3::new(1.0, 1.0, 3.0), Some("stone"), "stacked"),
        // The east side of a ground block: hang one off the edge. East and
        // south are the sides this camera can see; aiming at a west face
        // picks whatever stands in front of it, correctly.
        (Vec3::new(3.5, 2.0, 0.5), Some("stone"), "attached"),
        // And take the slab away again.
        (Vec3::new(3.0, 3.0, 1.25), None, "removed"),
    ];
    for (index, (aim, place, name)) in clicks.into_iter().enumerate() {
        let pixel = pixel_of(view.view_projection, aim);
        let (origin, direction) = ray_through(view.view_projection, pixel);
        let Some(hit) = voxel::pick(&volume, CELL, origin, direction, REACH) else {
            return Err(format!("the click at {pixel:?} reached no block").into());
        };
        click(&mut volume, hit, place);
        println!("{name}: clicked {:?} of {:?}", hit.face, hit.cell);
        let batches = instances(&volume, &textures);
        let frame = frame(view, &batches);
        shoot(
            &gpu,
            &target,
            &depth,
            &mut renderers,
            frame,
            &out.join(format!("voxel-lab-click-{index}-{name}.png")),
        )?;
    }
    Ok(())
}

/// The renderers one shot needs, which is every renderer a frame may call on.
struct Renderers<'a> {
    cube: &'a mut TexturedCubeRenderer,
    sprites: &'a mut SpriteBatchRenderer,
    text: &'a mut TextRenderer,
    glyphs: &'a mut GlyphRenderer,
    shapes: &'a mut ShapeRenderer,
    registry: &'a TextureRegistry,
}

/// Draw one frame and write it out.
fn shoot(
    gpu: &GpuContext,
    target: &OffscreenTarget,
    depth: &DepthTarget,
    renderers: &mut Renderers<'_>,
    frame: ExtractedFrame,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("voxel lab"),
        });
    encode_prepared_frame(
        FrameRenderers {
            cube: renderers.cube,
            sprites: renderers.sprites,
            text: renderers.text,
            glyphs: renderers.glyphs,
            shapes: renderers.shapes,
            textures: renderers.registry,
        },
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        FrameTarget {
            color: target.view(),
            depth,
        },
        &frame.prepare()?,
    )?;
    let readback = target.copy_to_buffer(&gpu.device, &mut encoder)?;
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback.read_rgba8(&gpu.device)?;
    let file = fs::File::create(path)?;
    let mut png = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
