//! A deterministic picture of the game being played, offscreen.
//!
//! The window is not always available and never reproducible; this is. It
//! plays a fixed run -- the same session the window drives -- and photographs
//! where that leaves the game.
//!
//! The run is *clicks*, not keys, because clicking is the game. Each one is
//! aimed by projecting the block it means through the same camera the frame is
//! drawn with, so what this proves is the whole loop: a point on the picture
//! becomes a ray, the ray finds a face, the face names a cell, and a block
//! appears there. A run written as cells would prove only that `set_block`
//! works.

#[cfg(not(target_arch = "wasm32"))]
use std::{error::Error, fs, io::BufWriter, path::Path};

#[cfg(not(target_arch = "wasm32"))]
use glam::{Mat4, Vec3};
#[cfg(not(target_arch = "wasm32"))]
use sindri_causeway::{
    Session, bind_fonts, bind_tile_sets, extractor, presented_world, stylesheets, world,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{InputEvent, InputState, MouseButton};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, GlyphRenderer, OffscreenTarget, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{CameraView, SceneRuntime};

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: u32 = 1000;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: u32 = 720;
#[cfg(not(target_arch = "wasm32"))]
const VIEWPORT: (f32, f32) = (1000.0, 720.0);
#[cfg(not(target_arch = "wasm32"))]
const STEP_SECONDS: f32 = 1.0 / 60.0;

/// The blocks to click the top of, in order.
///
/// Four across the channel, then three stacked into a stair, because a walker
/// may step up one block and the beacon stands three above the shore. The
/// last two name a cell the one before it created, which is the point: a
/// click lands on what is there now, not on what the level authored.
#[cfg(not(target_arch = "wasm32"))]
/// Each is a cell and whether to click its east side rather than its top.
///
/// Sides for the causeway, because a water tile's top sits a course below the
/// shore beside it and the shore stands in front of the last one. Tops for the
/// stair, on the side of the pillar that faces the camera -- a cell the pillar
/// hides cannot be clicked at all.
const RUN: &[((i32, i32, i32), bool)] = &[
    ((6, 7, 0), true),
    ((7, 7, 0), true),
    ((8, 7, 0), true),
    ((9, 7, 0), true),
    ((15, 7, 0), false),
    ((15, 7, 1), false),
    ((16, 7, 0), false),
];

/// Where the top of a cell lands on the picture, as a fraction across and down.
#[cfg(not(target_arch = "wasm32"))]
fn top_of(cell: (i32, i32, i32), view_projection: Mat4) -> [f32; 2] {
    #[allow(clippy::cast_precision_loss)]
    let centre = Vec3::new(cell.0 as f32, (cell.2 + 1) as f32, cell.1 as f32);
    project(centre, view_projection)
}

/// The same, for the east side -- one of the two sides this camera can see.
#[cfg(not(target_arch = "wasm32"))]
fn east_of(cell: (i32, i32, i32), view_projection: Mat4) -> [f32; 2] {
    #[allow(clippy::cast_precision_loss)]
    let centre = Vec3::new(cell.0 as f32 + 0.5, cell.2 as f32 + 0.5, cell.1 as f32);
    project(centre, view_projection)
}

#[cfg(not(target_arch = "wasm32"))]
fn project(point: Vec3, view_projection: Mat4) -> [f32; 2] {
    let clip = view_projection * point.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    [(ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5]
}

/// The run: build the way, then let somebody walk it.
#[cfg(not(target_arch = "wasm32"))]
fn play(
    world: &mut sindri_core::World,
    scene: &sindri_scene::SceneExtractor,
    session: &mut Session,
) -> Result<(), Box<dyn Error>> {
    // A settling step first, so the ground has answered where everything
    // stands before anything is aimed at it.
    session.step(world, &InputState::default(), VIEWPORT, STEP_SECONDS)?;
    let camera = sindri_scene::world_camera_of(world, scene.components(), VIEWPORT.0 / VIEWPORT.1)?
        .ok_or("the scene has no world camera")?;

    for (cell, side) in RUN {
        let point = if *side {
            east_of(*cell, camera.view_projection)
        } else {
            top_of(*cell, camera.view_projection)
        };
        let mut held = InputState::default();
        held.apply(InputEvent::PointerMoved {
            x: point[0] * VIEWPORT.0,
            y: point[1] * VIEWPORT.1,
        });
        held.apply(InputEvent::ButtonPressed(MouseButton::Left));
        session.step(world, &held, VIEWPORT, STEP_SECONDS)?;
        // A frame boundary, exactly as a host puts one there: without it the
        // press edge is still set on the next step and one click lays two
        // blocks.
        held.begin_frame(std::time::Duration::from_secs_f32(STEP_SECONDS));
        held.apply(InputEvent::ButtonReleased(MouseButton::Left));
        session.step(world, &held, VIEWPORT, STEP_SECONDS)?;
    }

    // Long enough for the walk itself: the way is open, and the picture should
    // be of somebody using it.
    let idle = InputState::default();
    for _ in 0..900 {
        session.step(world, &idle, VIEWPORT, STEP_SECONDS)?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn capture(path: &Path) -> Result<(), Box<dyn Error>> {
    // A script that failed is the most likely reason a picture is wrong, and a
    // capture that swallows the reason is a capture that wastes a run.
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .try_init()
        .ok();
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut cubes = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new();
    let mut glyphs = GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut shapes = ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    bind_fonts(&mut text)?;

    let (textures, bindings) = sindri_causeway::bind_textures(&gpu.device, &gpu.queue)?;
    let tile_sets = bind_tile_sets()?;

    let scene = extractor()?;
    let (mut world, loaded) = world()?;
    let mut session = Session::new(scene.components().clone())
        .with_scenes(sindri_causeway::scenes()?, loaded)
        .with_tile_sets(tile_sets.clone());

    play(&mut world, &scene, &mut session)?;

    let presented = presented_world(
        &world,
        &stylesheets()?,
        weave::Viewport {
            width: VIEWPORT.0,
            height: VIEWPORT.1,
        },
    )?;
    let prepared = scene.extract_animated(
        &presented,
        Viewport::new(WIDTH, HEIGHT),
        CameraView::default(),
        &bindings,
        SceneRuntime::default()
            .with_animations(session.animations())
            .with_effects(session.effects())
            .with_tile_sets(&tile_sets),
    )?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri Causeway capture encoder"),
        });
    let stats = encode_prepared_frame(
        FrameRenderers {
            cube: &mut cubes,
            sprites: &mut sprites,
            text: &mut text,
            glyphs: &mut glyphs,
            shapes: &mut shapes,
            textures: &textures,
        },
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        FrameTarget {
            color: target.view(),
            depth: &depth,
        },
        &prepared,
    )?;
    let readback = target.copy_to_buffer(&gpu.device, &mut encoder)?;
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback.read_rgba8(&gpu.device)?;
    println!(
        "batched {} sprites into {} draw calls",
        stats.sprite_count(),
        stats.draw_calls()
    );

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(BufWriter::new(fs::File::create(path)?), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/render-artifacts/causeway.png".to_owned());
    pollster::block_on(capture(Path::new(&path)))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
