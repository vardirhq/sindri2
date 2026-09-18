//! The same blocks, drawn the way a game draws them.
//!
//! The lab beside this one builds its own faces, camera and frame, which
//! proves the geometry and proves nothing about the engine. This loads a scene
//! file, hands it to the extractor every host uses, and photographs what comes
//! back. What it is really checking is that a grid saying `"space": "solid"`
//! is enough — no special host, no second render path, an ordinary camera
//! entity with an isometric pose.
//!
//! ```bash
//! cargo run -p voxel-lab --bin voxel-lab-scene -- target/render-artifacts
//! ```

use std::{error::Error, fs, io::BufWriter, path::Path};

use sindri_core::{SceneDocument, World};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, GlyphRenderer, OffscreenTarget, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, TextureRegistry, TexturedCubeRenderer, Viewport,
    encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TileSetBindings};

const WIDTH: u32 = 900;
const HEIGHT: u32 = 700;

fn main() -> Result<(), Box<dyn Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/render-artifacts".to_owned());
    pollster::block_on(run(Path::new(&out)))
}

async fn run(out: &Path) -> Result<(), Box<dyn Error>> {
    let root = Path::new("games/voxel-lab/assets");
    let document =
        SceneDocument::from_json(&fs::read_to_string(root.join("voxel-lab.scene.json"))?)?;
    let extractor = SceneExtractor::new()?;
    let world = World::from_scene(&document)?.world;

    let mut tile_sets = TileSetBindings::new();
    tile_sets.bind(
        "blocks.tileset.json",
        sindri_core::TileSetDocument::from_json(&fs::read_to_string(
            root.join("blocks.tileset.json"),
        )?)?,
    )?;

    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut registry = TextureRegistry::new(&gpu.device, &gpu.queue);
    let textures = voxel_lab::bind_textures(&gpu.device, &gpu.queue, &mut registry)?;

    let mut cube = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new();
    let mut glyphs = GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut shapes = ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);

    // Orbiting is the viewer's own adjustment, which every host already has.
    for (index, orbit) in [0.0_f32, 1.2, 2.4].into_iter().enumerate() {
        let frame = extractor.extract_animated(
            &world,
            Viewport::new(WIDTH, HEIGHT),
            CameraView {
                orbit: glam::Vec2::new(orbit, 0.0),
                ..CameraView::default()
            },
            &textures,
            SceneRuntime::default().with_tile_sets(&tile_sets),
        )?;
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scene"),
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
            &frame,
        )?;
        let readback = target.copy_to_buffer(&gpu.device, &mut encoder)?;
        gpu.queue.submit([encoder.finish()]);
        let pixels = readback.read_rgba8(&gpu.device)?;
        fs::create_dir_all(out)?;
        let path = out.join(format!("voxel-scene-{index}.png"));
        let mut png = png::Encoder::new(BufWriter::new(fs::File::create(&path)?), WIDTH, HEIGHT);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()?.write_image_data(&pixels)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
