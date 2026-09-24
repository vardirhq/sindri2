//! A photograph of the Weave showcase, offscreen, so a claim about how a
//! stylesheet looks can be checked rather than asserted.
//!
//! It loads a scene, presents it through its stylesheet at the size given on
//! the command line, exactly as a host does each frame, and draws that one
//! frame through the real renderers.
//!
//! ```bash
//! cargo run -p weave-poc --bin weave-poc-capture -- out.png 1280 720 demo
//! ```

use std::{error::Error, fs, io::BufWriter, path::Path};

use sindri_assets::{AssetBytes, AssetDecoder, FontAssetDecoder};
use sindri_core::{AssetId, SceneDocument, World};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, GlyphRenderer, OffscreenTarget, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, TextureRegistry, TexturedCubeRenderer, Viewport,
    encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TextureBindings};
use sindri_weave::{Presenter, UiStates};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| "target/render-artifacts/weave.png".to_owned());
    let width = args.next().and_then(|v| v.parse().ok()).unwrap_or(1280);
    let height = args.next().and_then(|v| v.parse().ok()).unwrap_or(720);
    let scene = args.next().unwrap_or_else(|| "demo".to_owned());
    pollster::block_on(capture(Path::new(&path), width, height, &scene))
}

async fn capture(path: &Path, width: u32, height: u32, scene: &str) -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let document = SceneDocument::from_json(&fs::read_to_string(
        root.join(format!("{scene}.scene.json")),
    )?)?;
    let authored = World::from_scene(&document)?.world;
    let stylesheet = weave::parse(&fs::read_to_string(root.join(format!("{scene}.weave")))?)?;
    #[allow(clippy::cast_precision_loss)]
    let viewport = weave::Viewport {
        width: width as f32,
        height: height as f32,
    };
    let world = Presenter::new().present(&authored, &[stylesheet], viewport, &UiStates::new())?;

    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, width, height)?;
    let depth = DepthTarget::new(&gpu.device, width, height);
    let mut cubes = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new();
    let mut glyphs = GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut shapes = ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    for id in sindri_scene::referenced_fonts(&world) {
        let bytes = fs::read(root.join(&id))?;
        let asset = FontAssetDecoder.decode(AssetBytes::new(id.parse::<AssetId>()?, bytes))?;
        text.bind_font(&id, asset.family(), asset.bytes().to_vec());
    }
    let textures = TextureRegistry::new(&gpu.device, &gpu.queue);

    let prepared = SceneExtractor::new()?.extract_animated(
        &world,
        Viewport::new(width, height),
        CameraView::default(),
        &TextureBindings::default(),
        SceneRuntime::default(),
    )?;
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Weave capture encoder"),
        });
    encode_prepared_frame(
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

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut png = png::Encoder::new(BufWriter::new(file), width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&pixels)?;
    println!("wrote {} ({scene} at {width}x{height})", path.display());
    Ok(())
}
