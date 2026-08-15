#[cfg(not(target_arch = "wasm32"))]
use std::{
    error::Error,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use sindri_cube::{DemoScene, FrameRenderers, FrameTarget, demo_textures, encode_prepared_frame};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, OffscreenTarget, SpriteBatchRenderer, TexturedCubeRenderer, Viewport,
};

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: u32 = 512;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: u32 = 512;

/// Colours the demo scene authors that must survive the render round trip.
///
/// Source textures are sRGB and shaders work in linear, so the target has to
/// encode on write. A target that does not still renders a perfectly valid
/// image — just the wrong colour — which no crash, lint, or headless test can
/// notice. Checking the pixels is what catches it.
#[cfg(not(target_arch = "wasm32"))]
const AUTHORED_COLORS: [(&str, [u8; 3]); 2] = [
    ("checkerboard orange", [240, 114, 43]),
    ("checkerboard navy", [18, 34, 55]),
];

/// Per-channel slack, generous enough for texture filtering and a software
/// rasteriser but far tighter than a colour-space mistake, which moves channels
/// by 40 to 70.
#[cfg(not(target_arch = "wasm32"))]
const CHANNEL_TOLERANCE: i32 = 16;

/// Each colour must cover at least this many pixels per thousand.
#[cfg(not(target_arch = "wasm32"))]
const MINIMUM_SHARE_PER_THOUSAND: usize = 5;

#[cfg(not(target_arch = "wasm32"))]
fn is_near(pixel: &[u8], expected: [u8; 3]) -> bool {
    pixel
        .iter()
        .zip(expected)
        .take(3)
        .all(|(actual, expected)| {
            (i32::from(*actual) - i32::from(expected)).abs() <= CHANNEL_TOLERANCE
        })
}

/// Reports the most common colours in the image, to make a mismatch diagnosable.
#[cfg(not(target_arch = "wasm32"))]
fn dominant_colors(pixels: &[u8]) -> Vec<([u8; 3], usize)> {
    let mut counts: std::collections::BTreeMap<[u8; 3], usize> = std::collections::BTreeMap::new();
    for pixel in pixels.chunks_exact(4) {
        // Quantise so near-identical shades group together.
        let key = [pixel[0] & !7, pixel[1] & !7, pixel[2] & !7];
        *counts.entry(key).or_default() += 1;
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.truncate(5);
    ranked
}

/// Fails when the rendered image is not the colour the scene authored.
#[cfg(not(target_arch = "wasm32"))]
fn verify_authored_colors(pixels: &[u8]) -> Result<(), String> {
    let total = pixels.len() / 4;
    for (name, expected) in AUTHORED_COLORS {
        let found = pixels
            .chunks_exact(4)
            .filter(|pixel| is_near(pixel, expected))
            .count();
        if found * 1000 < total * MINIMUM_SHARE_PER_THOUSAND {
            let dominant = dominant_colors(pixels);
            return Err(format!(
                "expected {name} {expected:?} to cover at least \
                 {MINIMUM_SHARE_PER_THOUSAND} pixels per thousand, but only {found} of {total} \
                 pixels are within {CHANNEL_TOLERANCE} per channel.\n\
                 The most common colours were {dominant:?}.\n\
                 A whole-image shift like this usually means a colour target is \
                 not sRGB, so linear output was stored without being encoded."
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn capture(path: &Path) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut cube_renderer = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprite_renderer = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let (textures, bindings) = demo_textures(&gpu.device, &gpu.queue);
    let scene = DemoScene::load()?;
    let prepared = scene.extract_frame(Viewport::new(WIDTH, HEIGHT), &bindings)?;

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
