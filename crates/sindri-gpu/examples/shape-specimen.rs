//! Every shape, modifier and blend on one sheet, drawn offscreen.
//!
//! The companion to `text-specimen`, and for the same reason: a distance field's
//! antialiasing, a dash's spacing and a sweep's direction are things you look at.
//!
//! ```bash
//! cargo run -p sindri-gpu --example shape-specimen -- out.png
//! ```

use std::{error::Error, fs, io::BufWriter, path::Path};

use glam::{Mat4, Quat, Vec3};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    Bloom, BloomSettings, ClearOperations, DepthTarget, ExtractedFrame, FrameCamera, FrameCommand,
    FramePass, FrameRenderers, FrameTarget, GlyphRenderer, OffscreenTarget, RenderLayer,
    RenderStage, Shape, ShapeBlend, ShapeInstance, ShapeRenderer, SpriteBatchRenderer,
    TextRenderer, TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
    orthographic_projection,
};

const WIDTH: u32 = 1100;
const HEIGHT: u32 = 900;
const HALF_HEIGHT: f32 = 1.0;

// The reference game's palette, which is what this is really testing.
const MINT: [f32; 4] = [0.49, 1.0, 0.77, 1.0];
const CORAL: [f32; 4] = [1.0, 0.42, 0.33, 1.0];
const VIOLET: [f32; 4] = [0.65, 0.45, 1.0, 1.0];
const CYAN: [f32; 4] = [0.45, 0.9, 1.0, 1.0];
const YELLOW: [f32; 4] = [1.0, 0.85, 0.3, 1.0];
const DIM: [f32; 4] = [0.16, 0.30, 0.26, 1.0];

/// A shape placed at `x`, `y` and `size` units across.
fn at(x: f32, y: f32, size: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::new(size, size, 1.0),
        Quat::IDENTITY,
        Vec3::new(x, y, 0.0),
    )
}

fn specimen() -> Vec<ShapeInstance> {
    let row = 0.42;
    vec![
        // Kinds, filled and stroked.
        ShapeInstance::stroked(
            at(-1.05, row, 0.24),
            Shape::Polygon { sides: 6.0 },
            0.05,
            MINT,
        ),
        ShapeInstance::stroked(
            at(-0.70, row, 0.24),
            Shape::Polygon { sides: 5.0 },
            0.05,
            CORAL,
        ),
        ShapeInstance::stroked(
            at(-0.35, row, 0.24),
            Shape::Polygon { sides: 3.0 },
            0.05,
            YELLOW,
        ),
        ShapeInstance::stroked(at(0.0, row, 0.24), Shape::Ellipse, 0.05, CYAN),
        ShapeInstance::stroked(at(0.35, row, 0.24), Shape::Rect, 0.05, VIOLET)
            .with_corner_radius(0.18),
        ShapeInstance::filled(at(0.70, row, 0.24), Shape::Polygon { sides: 6.0 }, VIOLET),
        // A filled shape that also carries a stroke, which is the card look.
        ShapeInstance::filled(at(1.05, row, 0.24), Shape::Rect, [0.05, 0.06, 0.09, 1.0])
            .with_corner_radius(0.18)
            .with_stroke(0.045, VIOLET),
        // Dashes: the "weapons offline" marker and a tick dial.
        ShapeInstance::stroked(at(-1.05, 0.0, 0.26), Shape::Ellipse, 0.05, YELLOW).dashed(9.0, 0.5),
        ShapeInstance::stroked(at(-0.70, 0.0, 0.26), Shape::Ellipse, 0.035, MINT)
            .dashed(24.0, 0.35),
        ShapeInstance::stroked(
            at(-0.35, 0.0, 0.26),
            Shape::Polygon { sides: 6.0 },
            0.05,
            CYAN,
        )
        .dashed(12.0, 0.6),
        // Sweeps: cooldowns and charge meters, drawn from the top clockwise.
        ShapeInstance::stroked(at(0.0, 0.0, 0.26), Shape::Ellipse, 0.06, DIM),
        ShapeInstance::stroked(at(0.0, 0.0, 0.26), Shape::Ellipse, 0.06, MINT).swept(0.0, 0.25),
        ShapeInstance::stroked(at(0.35, 0.0, 0.26), Shape::Ellipse, 0.06, DIM),
        ShapeInstance::stroked(at(0.35, 0.0, 0.26), Shape::Ellipse, 0.06, CORAL).swept(0.0, 0.66),
        ShapeInstance::stroked(at(0.70, 0.0, 0.26), Shape::Ellipse, 0.06, DIM),
        ShapeInstance::stroked(at(0.70, 0.0, 0.26), Shape::Ellipse, 0.06, VIOLET).swept(0.5, 0.3),
        // Concentric rings at a range of widths: the thin end is where a
        // distance field earns its keep, and where a scaled texture fails.
        ShapeInstance::stroked(at(1.05, 0.0, 0.30), Shape::Ellipse, 0.012, MINT),
        ShapeInstance::stroked(at(1.05, 0.0, 0.22), Shape::Ellipse, 0.02, MINT),
        ShapeInstance::stroked(at(1.05, 0.0, 0.13), Shape::Ellipse, 0.05, MINT),
        // Size sweep: one instance, seven scales, all one pixel of edge.
        ShapeInstance::stroked(
            at(-1.05, -0.42, 0.04),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
        ShapeInstance::stroked(
            at(-0.92, -0.42, 0.07),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
        ShapeInstance::stroked(
            at(-0.74, -0.42, 0.11),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
        ShapeInstance::stroked(
            at(-0.50, -0.42, 0.17),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
        ShapeInstance::stroked(
            at(-0.18, -0.42, 0.26),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
        ShapeInstance::stroked(
            at(0.25, -0.42, 0.40),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
        ShapeInstance::stroked(
            at(0.90, -0.42, 0.60),
            Shape::Polygon { sides: 6.0 },
            0.09,
            MINT,
        ),
    ]
}

/// Drawn additively, so overlapping strokes read as light rather than paint.
fn glow() -> Vec<ShapeInstance> {
    let mut shapes = Vec::new();
    // A shockwave: three expanding rings, each fainter than the last.
    for (index, size) in [0.20_f32, 0.30, 0.40].into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let fade = 0.55 - index as f32 * 0.16;
        shapes.push(ShapeInstance::stroked(
            at(-0.62, -0.80, size),
            Shape::Ellipse,
            0.035,
            [VIOLET[0], VIOLET[1], VIOLET[2], fade],
        ));
    }
    // Overlapping discs, to show additive actually accumulating.
    for offset in [-0.06_f32, 0.0, 0.06] {
        shapes.push(ShapeInstance::filled(
            at(0.30 + offset, -0.80, 0.16),
            Shape::Ellipse,
            [CYAN[0], CYAN[1], CYAN[2], 0.5],
        ));
    }
    shapes
}

/// The whole sheet, as one frame.
///
/// Split out because the drawing is the interesting half and the device setup
/// around it is not.
fn sheet(camera: FrameCamera) -> ExtractedFrame {
    let mut frame = ExtractedFrame::new(
        Viewport::new(WIDTH, HEIGHT),
        // Black, because that is the ground this language is drawn on.
        ClearOperations {
            color: [0.0, 0.0, 0.0, 1.0],
            depth: 1.0,
        },
    );
    frame.push(pass(
        FrameCommand::Shapes {
            blend: ShapeBlend::Over,
            instances: vec![ShapeInstance::stroked(
                at(0.0, 0.0, 2.4),
                Shape::Grid { cells: 24.0 },
                0.0015,
                [0.20, 0.34, 0.30, 1.0],
            )],
        },
        camera,
    ));
    frame.push(pass(
        FrameCommand::Shapes {
            blend: ShapeBlend::Over,
            instances: specimen(),
        },
        camera,
    ));
    frame.push(pass(
        FrameCommand::Shapes {
            blend: ShapeBlend::Add,
            instances: glow(),
        },
        camera,
    ));
    frame
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/render-artifacts/shape-specimen.png".to_owned());
    pollster::block_on(run(Path::new(&path)))
}

fn pass(command: FrameCommand, camera: FrameCamera) -> FramePass {
    FramePass::new(RenderStage::Overlay, RenderLayer::OVERLAY, camera, command)
}

async fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let gpu = GpuContext::request(&instance, None, &GpuRequestOptions::default()).await?;
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT)?;
    let depth = DepthTarget::new(&gpu.device, WIDTH, HEIGHT);
    // The sheet is drawn lit, because unlit is not what any of this is for: a
    // stroke on black without bloom is a coloured line, and the point of the
    // shapes is that they are a light source.
    let mut bloom = Bloom::new(&gpu.device, OffscreenTarget::FORMAT);
    bloom.resize(&gpu.device, WIDTH, HEIGHT);
    let scene = bloom
        .scene_view()
        .expect("the chain was just sized")
        .clone();

    let aspect = f64::from(WIDTH) / f64::from(HEIGHT);
    #[allow(clippy::cast_possible_truncation)]
    let half_width = HALF_HEIGHT * aspect as f32;
    let camera = FrameCamera {
        view_projection: orthographic_projection(
            -half_width,
            half_width,
            -HALF_HEIGHT,
            HALF_HEIGHT,
            -1.0,
            1.0,
        ),
    };

    let frame = sheet(camera);
    // The grid goes down first, under everything, as one instance.
    let prepared = frame.prepare()?;

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shape specimen encoder"),
        });
    encode_prepared_frame(
        FrameRenderers {
            cube: &mut TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            sprites: &mut SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            text: &mut TextRenderer::new(),
            glyphs: &mut GlyphRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            shapes: &mut ShapeRenderer::new(&gpu.device, OffscreenTarget::FORMAT),
            textures: &TextureRegistry::new(&gpu.device, &gpu.queue),
        },
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        FrameTarget {
            color: &scene,
            depth: &depth,
        },
        &prepared,
    )?;
    bloom.resolve(
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        target.view(),
        BloomSettings::default(),
    );
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
