//! A picture of the level, with nothing playing.
//!
//! Before any script runs there is a question worth settling on its own: does
//! the ground look like ground. This loads the scene, hands it to the
//! extractor every host uses, and photographs the result -- no session, no
//! input, no scripts. When something is wrong with the blocks, this says so
//! without a game in the way.
//!
//! ```bash
//! cargo run -p sindri-causeway --bin level-preview -- out.png
//! ```

#[cfg(not(target_arch = "wasm32"))]
use std::{error::Error, fs, io::BufWriter, path::Path};

#[cfg(not(target_arch = "wasm32"))]
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
#[cfg(not(target_arch = "wasm32"))]
use sindri_core::{AssetId, SceneDocument, SpriteSheetDocument, TileSetDocument, World};
#[cfg(not(target_arch = "wasm32"))]
use sindri_gpu::{GpuContext, GpuRequestOptions};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, GlyphRenderer, OffscreenTarget, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, Texture2D, TextureRegistry, TexturedCubeRenderer, Viewport,
    encode_prepared_frame,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{
    CameraView, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings,
    resolve_grid_placements,
};

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: u32 = 1000;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: u32 = 720;

#[cfg(not(target_arch = "wasm32"))]
const SHEETS: &[&str] = &[
    "blocks-top",
    "blocks-side",
    "slabs-side",
    "wanderer",
    "beacon",
];

#[cfg(not(target_arch = "wasm32"))]
fn bind_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    registry: &mut TextureRegistry,
) -> Result<TextureBindings, Box<dyn Error>> {
    let root = Path::new("game/assets/textures");
    let mut bindings = TextureBindings::new();
    for name in SHEETS {
        let reference = format!("textures/{name}.png");
        let asset = TextureAssetDecoder.decode(AssetBytes::new(
            reference.parse::<AssetId>()?,
            fs::read(root.join(format!("{name}.png")))?,
        ))?;
        let texture = Texture2D::from_rgba8(
            device,
            queue,
            name,
            asset.width(),
            asset.height(),
            asset.rgba8(),
        )?;
        bindings.bind(&reference, registry.insert(texture));
        let sheet = fs::read_to_string(root.join(format!("{name}.sheet.json")))?;
        bindings.bind_sheet(&reference, &SpriteSheetDocument::from_json(&sheet)?)?;
    }
    Ok(bindings)
}

#[cfg(not(target_arch = "wasm32"))]
async fn preview(path: &Path) -> Result<(), Box<dyn Error>> {
    let root = Path::new("game/assets");
    let document =
        SceneDocument::from_json(&fs::read_to_string(root.join("causeway.scene.json"))?)?;
    let extractor = SceneExtractor::new()?;
    let mut world = World::from_scene(&document)?.world;

    let mut tile_sets = TileSetBindings::new();
    tile_sets.bind(
        "causeway.tileset.json",
        TileSetDocument::from_json(&fs::read_to_string(root.join("causeway.tileset.json"))?)?,
    )?;
    resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets))?;

    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    let mut registry = TextureRegistry::new(&gpu.device, &gpu.queue);
    let textures = bind_textures(&gpu.device, &gpu.queue, &mut registry)?;

    let frame = extractor.extract_animated(
        &world,
        Viewport::new(WIDTH, HEIGHT),
        CameraView::default(),
        &textures,
        SceneRuntime::default().with_tile_sets(&tile_sets),
    )?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("level-preview"),
        });
    encode_prepared_frame(
        FrameRenderers {
            cube: &mut TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            sprites: &mut SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            text: &mut TextRenderer::new(),
            glyphs: &mut GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            shapes: &mut ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
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

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut png = png::Encoder::new(BufWriter::new(fs::File::create(path)?), WIDTH, HEIGHT);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/level-preview.png".to_owned());
    pollster::block_on(preview(Path::new(&out)))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
