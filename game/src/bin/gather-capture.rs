//! A deterministic picture of the game, offscreen.
//!
//! The window is not always available and never reproducible; this is. It plays
//! a fixed run — the same session the window drives, stepped a fixed number of
//! times with a fixed key held — and photographs where that leaves the game, at
//! a fixed size with no window.
//!
//! Playing rather than photographing the opening frame is the point. A picture
//! of the scene at rest proves the scene loads; a picture part-way through a
//! run proves the scripts ran, the player moved, an orb was gathered, a lamp
//! lit, and the walk animation is on one of its frames rather than showing the
//! whole sheet.

#[cfg(not(target_arch = "wasm32"))]
use std::{error::Error, fs, io::BufWriter, path::Path};

#[cfg(not(target_arch = "wasm32"))]
use sindri_gather::{Session, bind_fonts, extractor, world};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{InputEvent, InputState, Key};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, OffscreenTarget, SpriteBatchRenderer, TextRenderer,
    TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{CameraView, SceneRuntime};

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: u32 = 960;
/// The same size the screen UI is laid out against, as the capture draws it.
const VIEWPORT: (f32, f32) = (960.0, 600.0);
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: u32 = 600;
/// The run the picture is of: hold these keys for that many fixed steps.
///
/// Written as keys rather than as positions because that is what a player
/// gives the game. It walks along both isometric grid axes toward an orb, which
/// gathers one — enough for a lamp to light in the corner, and for the picture
/// to be of a game being played rather than of a scene.
#[cfg(not(target_arch = "wasm32"))]
const RUN: &[(&[Key], u32)] = &[(&[Key::ArrowLeft, Key::ArrowUp], 55)];
#[cfg(not(target_arch = "wasm32"))]
const STEP_SECONDS: f32 = 1.0 / 60.0;

#[cfg(not(target_arch = "wasm32"))]
async fn capture(path: &Path) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut cubes = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new(&gpu.device, &gpu.queue, OffscreenTarget::FORMAT);
    bind_fonts(&mut text)?;

    let (textures, bindings) = sindri_gather::bind_textures(&gpu.device, &gpu.queue)?;

    let scene = extractor()?;
    let mut world = world()?;
    let mut session = Session::new(scene.components().clone());
    for (keys, steps) in RUN {
        // Rebuilt each leg rather than released, because that is the state the
        // window reports: a key that is down stays down until it comes up.
        let mut held = InputState::default();
        for key in *keys {
            held.apply(InputEvent::KeyPressed(*key));
        }
        for _ in 0..*steps {
            session.step(&mut world, &held, VIEWPORT, STEP_SECONDS)?;
        }
    }
    let prepared = scene.extract_animated(
        &world,
        Viewport::new(WIDTH, HEIGHT),
        CameraView::default(),
        &bindings,
        SceneRuntime::default()
            .with_animations(session.animations())
            .with_effects(session.effects()),
    )?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri gather capture encoder"),
        });
    let stats = encode_prepared_frame(
        FrameRenderers {
            cube: &mut cubes,
            sprites: &mut sprites,
            text: &mut text,
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
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
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
        .unwrap_or_else(|| "target/render-artifacts/gather.png".to_owned());
    pollster::block_on(capture(Path::new(&path)))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
