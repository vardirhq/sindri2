#[cfg(not(target_arch = "wasm32"))]
use std::{
    error::Error,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use sindri_cube::{DemoScene, demo_textures, verify_authored_colors};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, OffscreenTarget, SpriteBatchRenderer, TexturedCubeRenderer, Viewport,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{FrameRenderers, FrameTarget, encode_prepared_frame};

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: u32 = 512;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: u32 = 512;

#[cfg(not(target_arch = "wasm32"))]
async fn capture(path: &Path) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut cube_renderer = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprite_renderer = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let (textures, bindings) = demo_textures(&gpu.device, &gpu.queue);
    let (scene, world) = DemoScene::load()?;
    let prepared = scene.extract_frame(&world, Viewport::new(WIDTH, HEIGHT), &bindings)?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri screenshot encoder"),
        });
    let stats = encode_prepared_frame(
        FrameRenderers {
            cube: &mut cube_renderer,
            sprites: &mut sprite_renderer,
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
    println!(
        "batched {} sprites into {} draw call, saving {} draw calls",
        stats.sprite_count(),
        stats.draw_calls(),
        stats.draw_calls_saved()
    );
    let readback = target.copy_to_buffer(&gpu.device, &mut encoder)?;
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback.read_rgba8(&gpu.device)?;

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {}", path.display());

    // Checked after writing, so a failing frame is still uploaded to look at.
    verify_authored_colors(&pixels)?;
    println!("verified authored scene colours survived the render round trip");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let path = std::env::args_os().nth(1).map_or_else(
        || PathBuf::from("target/render-artifacts/scene-frame-pipeline.png"),
        PathBuf::from,
    );
    if let Err(error) = pollster::block_on(capture(&path)) {
        panic!("offscreen capture failed: {error}");
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
