//! A photograph of the game, offscreen, so a claim about what is on screen can
//! be checked rather than asserted.
//!
//! It plays the project with `Run` — the same harness the tests use — and then
//! draws one frame through the real renderers at a size given on the command
//! line. Nothing here knows anything about Orbital Last Stand: the assets come
//! off disk by the IDs the scene names, so the same binary photographs any
//! project laid out the way this one is.
//!
//! ```bash
//! cargo run -p orbital-last-stand --bin orbital-capture -- out.png 390 844
//! ```

use std::{error::Error, fs, io::BufWriter, path::Path};

use orbital_last_stand::Run;
use sindri_assets::{AssetBytes, AssetDecoder, FontAssetDecoder, TextureAssetDecoder};
use sindri_core::{AssetId, SpriteSheetDocument, sheet_id_for};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, OffscreenTarget, SpriteBatchRenderer, TextRenderer,
    Texture2D, TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneRuntime, TextureBindings, UiCanvas, WorldProjection};

/// What the picture is of: how long to play before the shutter opens, and what
/// the person did in that time.
struct Shot {
    seconds: f32,
    /// Press this element once play has settled, by the name the scene gave it.
    click: Option<&'static str>,
}

fn shot_for(name: &str) -> Shot {
    match name {
        // A run in progress: enemies out, bullets flying, the HUD live.
        "playing" => Shot {
            seconds: 12.0,
            click: Some("TitleStart"),
        },
        // The title screen, which is what a player sees first and what every
        // layout mistake shows up on.
        _ => Shot {
            seconds: 0.2,
            click: None,
        },
    }
}

/// Everything the scene names, loaded off disk and bound for drawing.
fn bind(
    run: &Run,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    text: &mut TextRenderer,
) -> Result<(TextureRegistry, TextureBindings), Box<dyn Error>> {
    let root = orbital_last_stand::project().join("assets");
    let mut textures = TextureRegistry::new(device, queue);
    let mut bindings = TextureBindings::new();

    for id in run.referenced_textures() {
        // A procedural texture is drawn by the engine and has no file.
        if id.starts_with("sindri:") {
            continue;
        }
        let bytes = fs::read(root.join(&id))?;
        let asset = TextureAssetDecoder.decode(AssetBytes::new(id.parse::<AssetId>()?, bytes))?;
        let texture = Texture2D::from_rgba8(
            device,
            queue,
            &id,
            asset.width(),
            asset.height(),
            asset.rgba8(),
        )?;
        bindings.bind(&id, textures.insert(texture));

        let Some(sheet) = id.parse::<AssetId>().ok().and_then(|id| sheet_id_for(&id)) else {
            continue;
        };
        if let Ok(json) = fs::read_to_string(root.join(sheet.as_str())) {
            bindings.bind_sheet(&id, &SpriteSheetDocument::from_json(&json)?)?;
        }
    }

    for id in run.referenced_fonts() {
        let bytes = fs::read(root.join(&id))?;
        let asset = FontAssetDecoder.decode(AssetBytes::new(id.parse::<AssetId>()?, bytes))?;
        text.bind_font(&id, asset.family(), asset.bytes().to_vec());
    }
    Ok((textures, bindings))
}

async fn capture(
    path: &Path,
    width: u32,
    height: u32,
    what: &str,
    canvas: UiCanvas,
    view: CameraView,
) -> Result<(), Box<dyn Error>> {
    let mut run = Run::open().map_err(|error| -> Box<dyn Error> { error.into() })?;
    run.viewport = (width as f32, height as f32);

    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, width, height)?;
    let depth = DepthTarget::new(&gpu.device, width, height);
    let mut cubes = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut text = TextRenderer::new();
    let (textures, bindings) = bind(&run, &gpu.device, &gpu.queue, &mut text)?;

    let shot = shot_for(what);
    for _ in 0..8 {
        run.step(1.0 / 60.0);
    }
    if let Some(element) = shot.click {
        run.click(element);
    }
    let steps = (shot.seconds * 60.0) as usize;
    for step in 0..steps {
        let notes = run.step(1.0 / 60.0);
        assert!(notes.is_empty(), "step {step}: {notes:#?}");
    }

    let prepared = run.scene_extractor().extract_animated(
        &run.world,
        Viewport::new(width, height),
        view,
        &bindings,
        SceneRuntime::default()
            .with_effects(&run.effects)
            .with_canvas(canvas),
    )?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Orbital capture encoder"),
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
        "{what} at {width}x{height}: {} sprites in {} draw calls",
        stats.sprite_count(),
        stats.draw_calls()
    );

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut png = png::Encoder::new(BufWriter::new(file), width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| "target/render-artifacts/orbital.png".to_owned());
    let width = args.next().and_then(|v| v.parse().ok()).unwrap_or(390);
    let height = args.next().and_then(|v| v.parse().ok()).unwrap_or(844);
    let what = args.next().unwrap_or_else(|| "title".to_owned());
    // How the UI is looked at. `scene` puts it on a canvas in the world seen
    // through the authored camera; `orbit` looks at that same canvas from off to
    // one side, which is what the editor's Scene view does and the one picture
    // that shows whether text is really on the canvas or stuck to the screen.
    let (canvas, view) = match args.next().as_deref() {
        Some("scene") => (
            UiCanvas::InScene { aspect: 9.0 / 19.5 },
            CameraView::default(),
        ),
        Some("orbit") => (
            UiCanvas::InScene { aspect: 9.0 / 19.5 },
            CameraView {
                orbit: glam::Vec2::new(0.6, 0.35),
                distance_scale: 0.3,
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
        ),
        _ => (UiCanvas::OnViewport, CameraView::default()),
    };
    pollster::block_on(capture(
        Path::new(&path),
        width,
        height,
        &what,
        canvas,
        view,
    ))
}
