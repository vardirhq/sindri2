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
#[cfg(not(target_arch = "wasm32"))]
use sindri_causeway::{
    Session, bind_fonts, bind_tile_sets, extractor, presented_world, stylesheets, world,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{InputEvent, InputState, Key, MouseButton};
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
/// How many blocks the run lays, and where it aims them.
///
/// Nothing here names a cell. The world is generated, so a coordinate written
/// down is a coordinate that means something different under the next seed --
/// the run points at the middle of the picture and lets the picker say what is
/// there, which is what a player does.
const PLACEMENTS: usize = 6;

/// One click, pressed and released as a host would deliver it.
#[cfg(not(target_arch = "wasm32"))]
fn click(
    world: &mut sindri_core::World,
    session: &mut Session,
    point: [f32; 2],
) -> Result<(), Box<dyn Error>> {
    let mut held = InputState::default();
    held.apply(InputEvent::PointerMoved {
        x: point[0] * VIEWPORT.0,
        y: point[1] * VIEWPORT.1,
    });
    held.apply(InputEvent::ButtonPressed(MouseButton::Left));
    session.step(world, &held, VIEWPORT, STEP_SECONDS)?;
    // A frame boundary, exactly as a host puts one there: without it the press
    // edge is still set on the next step and one click lays two blocks.
    held.begin_frame(std::time::Duration::from_secs_f32(STEP_SECONDS));
    held.apply(InputEvent::ButtonReleased(MouseButton::Left));
    session.step(world, &held, VIEWPORT, STEP_SECONDS)?;
    Ok(())
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

    let _ = camera;
    // Second block in the palette: earth, which fills its cell.
    let mut choose = InputState::default();
    choose.apply(InputEvent::KeyPressed(Key::Digit2));
    session.step(world, &choose, VIEWPORT, STEP_SECONDS)?;
    // A little tower in front of the walker, so the picture shows the game
    // having been played rather than the world as it was generated.
    for _ in 0..PLACEMENTS {
        click(world, session, [0.5, 0.56])?;
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
