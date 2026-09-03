//! Every text option on one sheet, drawn offscreen so it can be looked at.
//!
//! A specimen rather than a test: what an outline, a shadow, a wrap width or an
//! auto-fit *does* is a thing you look at, and an assertion that a number came
//! back is not the same as knowing the words are where they should be. This
//! renders through the real renderers at a real size and writes a PNG.
//!
//! ```bash
//! cargo run -p sindri-gpu --example text-specimen -- out.png
//! ```
//!
//! It lives here because this is the crate that already pairs an adapter with
//! `sindri-render` in its dev builds; `sindri-render` itself depends on wgpu,
//! glam and bytemuck alone, and adding an adapter to it to take a photograph
//! would be the wrong way round.

use std::{error::Error, fs, io::BufWriter, path::Path};

use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    ClearOperations, DepthTarget, ExtractedFrame, FrameCamera, FrameCommand, FramePass,
    FrameRenderers, FrameTarget, GlyphRenderer, LineAlign, OffscreenTarget, RenderLayer,
    RenderStage, ShapeRenderer, SpriteBatchRenderer, TextAlign, TextCase, TextFit, TextInstance,
    TextRenderer, TextShadow, TextStroke, TextStyle, TextWrap, TextureRegistry,
    TexturedCubeRenderer, Viewport, encode_prepared_frame, orthographic_projection,
};

const FONT: &str = "fonts/Inter.ttf";
const WIDTH: u32 = 1000;
const HEIGHT: u32 = 1000;

/// The overlay this sheet is laid out in: two units tall, centred on nothing,
/// running out to the aspect ratio either side. The same space a HUD uses.
const HALF_HEIGHT: f32 = 1.0;

const WHITE: [f32; 4] = [0.96, 0.96, 0.98, 1.0];
const AMBER: [f32; 4] = [0.95, 0.70, 0.25, 1.0];
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn label(text: &str, y: f32, size: f32, color: [f32; 4]) -> Result<TextInstance, Box<dyn Error>> {
    Ok(TextInstance::new(
        text,
        FONT,
        [0.0, y],
        size,
        size * 1.25,
        color,
        [TextAlign::Middle, TextAlign::Middle],
    )?)
}

/// One row per option, top to bottom, each showing the option turned on beside
/// the words that name it.
fn specimen() -> Result<Vec<TextInstance>, Box<dyn Error>> {
    Ok(vec![
        label("Plain 0.09", 0.86, 0.09, WHITE)?,
        label("Outlined", 0.70, 0.11, AMBER)?.with_outline(TextStroke {
            width: 0.01,
            color: BLACK,
        }),
        label("Shadowed", 0.54, 0.11, WHITE)?.with_shadow(TextShadow {
            offset: [0.008, -0.010],
            color: [0.0, 0.0, 0.0, 0.8],
            softness: 0.004,
        }),
        label("Bold", 0.40, 0.09, WHITE)?.with_style(TextStyle {
            bold: true,
            italic: false,
        }),
        label("Italic", 0.28, 0.09, WHITE)?.with_style(TextStyle {
            bold: false,
            italic: true,
        }),
        label("Upper case & spaced", 0.16, 0.05, WHITE)?
            .with_case(TextCase::Upper)
            .with_letter_spacing(0.012),
        // Wrapped and justified inside a box narrower than the line.
        label(
            "A paragraph wrapped inside a box and justified to both of its edges.",
            -0.06,
            0.042,
            WHITE,
        )?
        .in_box([1.1, 0.30], TextWrap::Word)
        .with_line_align(LineAlign::Justify),
        // The same idea, shrunk to fit its box rather than wrapped to it.
        label(
            "Auto-sized down to fit a box it would otherwise overflow",
            -0.44,
            0.09,
            AMBER,
        )?
        .in_box([1.2, 0.16], TextWrap::Word)
        .fitted(TextFit::checked(0.02, 0.09).expect("a real range")),
        // A reveal, part way through.
        label("Revealed one glyph at a time", -0.72, 0.05, WHITE)?.with_visible_glyphs(13),
        label("Tiny 0.022 — still sharp", -0.88, 0.022, WHITE)?,
    ])
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/render-artifacts/text-specimen.png".to_owned());
    pollster::block_on(run(Path::new(&path)))
}

async fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);

    let mut text = TextRenderer::new();
    let bytes = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../game/assets/fonts/Inter.ttf"
    ))?;
    text.bind_font(FONT, "Inter", bytes);

    let aspect = f64::from(WIDTH) / f64::from(HEIGHT);
    #[allow(clippy::cast_possible_truncation)]
    let half_width = HALF_HEIGHT * aspect as f32;
    let mut frame = ExtractedFrame::new(
        Viewport::new(WIDTH, HEIGHT),
        ClearOperations {
            color: [0.07, 0.08, 0.11, 1.0],
            depth: 1.0,
        },
    );
    frame.push(FramePass::new(
        RenderStage::Overlay,
        RenderLayer::OVERLAY,
        FrameCamera {
            view_projection: orthographic_projection(
                -half_width,
                half_width,
                -HALF_HEIGHT,
                HALF_HEIGHT,
                -1.0,
                1.0,
            ),
        },
        FrameCommand::Text {
            instances: specimen()?,
        },
    ));
    let prepared = frame.prepare()?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Text specimen encoder"),
        });
    encode_prepared_frame(
        FrameRenderers {
            cube: &mut TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            sprites: &mut SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            text: &mut text,
            glyphs: &mut GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            shapes: &mut ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            textures: &TextureRegistry::new(&gpu.device, &gpu.queue),
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
    let mut png = png::Encoder::new(BufWriter::new(fs::File::create(path)?), WIDTH, HEIGHT);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&pixels)?;
    println!(
        "wrote {} with {} glyphs baked",
        path.display(),
        text.glyph_count()
    );
    Ok(())
}
