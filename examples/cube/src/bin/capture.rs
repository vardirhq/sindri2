#[cfg(not(target_arch = "wasm32"))]
use std::{
    error::Error,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]
use glam::Mat4;
#[cfg(not(target_arch = "wasm32"))]
use sindri_cube::{demo_badge_texture, sprite_overlay_transform};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, OffscreenTarget, OrthographicCamera, PerspectiveCamera, SpriteRenderer,
    TexturedCubeRenderer,
};

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
    let cube_renderer =
        TexturedCubeRenderer::new(&gpu.device, &gpu.queue, OffscreenTarget::FORMAT);
    let sprite_renderer = SpriteRenderer::new(
        &gpu.device,
        OffscreenTarget::FORMAT,
        demo_badge_texture(&gpu.device, &gpu.queue),
    );
    let perspective_camera = PerspectiveCamera::default();
    let orthographic_camera = OrthographicCamera::default();
    let model = Mat4::from_rotation_y(0.72) * Mat4::from_rotation_x(-0.42);

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri screenshot encoder"),
        });
    cube_renderer.encode(
        &gpu.queue,
        &mut encoder,
        target.view(),
        &depth,
        perspective_camera.view_projection(1.0) * model,
    );
    sprite_renderer.encode(
        &gpu.queue,
        &mut encoder,
        target.view(),
        orthographic_camera.view_projection(1.0) * sprite_overlay_transform(1.0),
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
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let path = std::env::args_os().nth(1).map_or_else(
        || PathBuf::from("target/render-artifacts/cube-sprite-overlay.png"),
        PathBuf::from,
    );
    if let Err(error) = pollster::block_on(capture(&path)) {
        panic!("offscreen capture failed: {error}");
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
