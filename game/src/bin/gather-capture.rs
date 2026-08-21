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
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
#[cfg(not(target_arch = "wasm32"))]
use sindri_core::AssetId;
#[cfg(not(target_arch = "wasm32"))]
use sindri_gather::{Session, TEXTURES, extractor, world};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{InputEvent, InputState, Key};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, OffscreenTarget, SpriteBatchRenderer, Texture2D,
    TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{CameraView, TextureBindings};

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: u32 = 960;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: u32 = 600;
/// The run the picture is of: hold these keys for that many fixed steps.
///
/// Written as keys rather than as positions because that is what a player
/// gives the game. It walks up onto the row an orb sits on and then along it,
/// which gathers one — enough for a lamp to light in the corner, and for the
/// picture to be of a game being played rather than of a scene.
#[cfg(not(target_arch = "wasm32"))]
const RUN: &[(&[Key], u32)] = &[
    (&[Key::ArrowRight, Key::ArrowUp], 18),
    (&[Key::ArrowRight], 40),
];
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

    let mut textures = TextureRegistry::new(&gpu.device, &gpu.queue);
    let mut bindings = TextureBindings::new();
    for (id, bytes) in TEXTURES {
        let asset = TextureAssetDecoder
            .decode(AssetBytes::new((*id).parse::<AssetId>()?, bytes.to_vec()))?;
        let texture = Texture2D::from_rgba8(
            &gpu.device,
            &gpu.queue,
            id,
            asset.width(),
            asset.height(),
            asset.rgba8(),
        )?;
        bindings.bind(*id, textures.insert(texture));
    }

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
            session.step(&mut world, &held, STEP_SECONDS)?;
        }
    }
    let prepared = scene.extract_animated(
        &world,
        Viewport::new(WIDTH, HEIGHT),
        CameraView::default(),
        &bindings,
        session.animations(),
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
