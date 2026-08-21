//! Whether a sprite's texture rect reaches the pixels.
//!
//! Everything upstream of the shader is checkable without a device — the rect is
//! validated at construction, carried on the instance, and `sindri-scene`'s
//! tests prove it survives extraction and that frames of one sheet stay in one
//! batch. None of that would notice a shader that ignored the attribute, and the
//! result would be every frame of every sheet drawing the whole sheet: obviously
//! wrong to look at, invisible to every other test.
//!
//! So this draws one and reads the pixel back.

use glam::{Mat4, Vec3};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    ClearOperations, DepthTarget, OffscreenTarget, OrthographicCamera, SpriteBatchRenderer,
    SpriteDepth, SpriteInstance, Texture2D, TextureRegistry, UvRect, encode_clear,
};

/// Set wherever a software adapter is installed on purpose. A GPU test that
/// skips on the machine that exists to run it is a check that quietly stopped
/// checking, so CI demands the adapter rather than hoping for it.
const REQUIRE_GPU: &str = "SINDRI_REQUIRE_GPU";

const SIZE: u32 = 64;
/// Rounding through an sRGB target moves a byte or so; sampling the wrong
/// quadrant moves all of them.
const CHANNEL_TOLERANCE: i32 = 3;

/// A two-by-two sheet whose cells are told apart at a glance.
const TOP_LEFT: [u8; 4] = [220, 40, 40, 255];
const TOP_RIGHT: [u8; 4] = [40, 200, 60, 255];
const BOTTOM_LEFT: [u8; 4] = [50, 90, 230, 255];
const BOTTOM_RIGHT: [u8; 4] = [230, 200, 40, 255];

fn gpu() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();
    match pollster::block_on(GpuContext::request(
        &instance,
        None,
        &GpuRequestOptions::default(),
    )) {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            assert!(
                std::env::var_os(REQUIRE_GPU).is_none(),
                "{REQUIRE_GPU} is set but no adapter could be requested: {error}"
            );
            eprintln!("skipping: no GPU adapter ({error})");
            None
        }
    }
}

/// The sheet, as raw rows: two texels across, two down.
fn sheet(gpu: &GpuContext) -> Texture2D {
    let mut pixels = Vec::with_capacity(16);
    pixels.extend_from_slice(&TOP_LEFT);
    pixels.extend_from_slice(&TOP_RIGHT);
    pixels.extend_from_slice(&BOTTOM_LEFT);
    pixels.extend_from_slice(&BOTTOM_RIGHT);
    Texture2D::from_rgba8(&gpu.device, &gpu.queue, "sheet", 2, 2, &pixels)
        .expect("a two by two sheet is valid")
}

/// Draws one sprite filling the frame, showing `rect` of the sheet, and reads
/// back the middle pixel and the middle of each quadrant.
fn sample(gpu: &GpuContext, rect: UvRect) -> ([u8; 4], [[u8; 4]; 4]) {
    let target = OffscreenTarget::new(&gpu.device, SIZE, SIZE).expect("the target is valid");
    let depth = DepthTarget::new(&gpu.device, SIZE, SIZE);
    let mut textures = TextureRegistry::new(&gpu.device, &gpu.queue);
    let sheet = textures.insert(sheet(gpu));
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);

    // Straight on and filling the frame, so the middle pixel is unambiguously
    // inside the sprite and nothing but the rect decides its colour.
    let view_projection = OrthographicCamera {
        center: glam::Vec2::ZERO,
        vertical_size: 1.0,
        near: 0.0,
        far: 10.0,
    }
    .view_projection(1.0);

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri sprite rect test encoder"),
        });
    encode_clear(
        &mut encoder,
        target.view(),
        &depth,
        ClearOperations::default(),
    );
    sprites.begin_submission();
    sprites
        .draw(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            target.view(),
            &depth,
            &textures,
            sheet,
            view_projection,
            SpriteDepth::Ignore,
            &[SpriteInstance::new(
                Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0)),
                [1.0, 1.0, 1.0, 1.0],
            )
            .with_uv_rect(rect)],
        )
        .expect("one sprite fits the batch");
    let readback = target
        .copy_to_buffer(&gpu.device, &mut encoder)
        .expect("the target copies back");
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback
        .read_rgba8(&gpu.device)
        .expect("the frame reads back");

    let middle = ((SIZE / 2) * SIZE + SIZE / 2) as usize * 4;
    let centre = [
        pixels[middle],
        pixels[middle + 1],
        pixels[middle + 2],
        pixels[middle + 3],
    ];

    // A quarter and three quarters along each axis, which is the middle of each
    // quadrant of the frame. Sampling the exact centre instead would land on the
    // corner where all four texels of a two-by-two meet, and answer a question
    // about the sampler rather than about the rect.
    let quadrants = [(1, 1), (3, 1), (1, 3), (3, 3)].map(|(x, y): (u32, u32)| {
        let offset = ((y * SIZE / 4) * SIZE + x * SIZE / 4) as usize * 4;
        [
            pixels[offset],
            pixels[offset + 1],
            pixels[offset + 2],
            pixels[offset + 3],
        ]
    });
    (centre, quadrants)
}

fn assert_near(actual: [u8; 4], expected: [u8; 4], what: &str) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (i32::from(*a) - i32::from(e)).abs() <= CHANNEL_TOLERANCE);
    assert!(close, "{what}: drew {actual:?} rather than {expected:?}");
}

/// The whole point: each cell of the sheet draws its own colour, which can only
/// happen if the shader read the rect.
#[test]
fn each_cell_of_a_sheet_draws_its_own_pixels() {
    let Some(gpu) = gpu() else {
        return;
    };
    for (column, row, expected, name) in [
        (0, 0, TOP_LEFT, "top left"),
        (1, 0, TOP_RIGHT, "top right"),
        (0, 1, BOTTOM_LEFT, "bottom left"),
        (1, 1, BOTTOM_RIGHT, "bottom right"),
    ] {
        let rect = UvRect::cell(column, row, 2, 2).expect("a cell of a two by two sheet");
        assert_near(sample(&gpu, rect).0, expected, name);
    }
}

/// A sprite with no rect maps the whole texture across itself, as every sprite
/// did before rects existed — so its four quadrants show the sheet's four cells.
/// A sprite showing one cell shows that cell everywhere.
///
/// Stated as a set rather than a list on purpose: which corner of the frame a
/// cell lands in is a question about orientation, and this is a question about
/// coverage.
#[test]
fn a_rect_decides_how_much_of_the_texture_the_sprite_covers() {
    let Some(gpu) = gpu() else {
        return;
    };
    let cells = [TOP_LEFT, TOP_RIGHT, BOTTOM_LEFT, BOTTOM_RIGHT];

    let (_, whole) = sample(&gpu, UvRect::FULL);
    let mut seen = whole.to_vec();
    seen.sort_unstable();
    let mut expected = cells.to_vec();
    expected.sort_unstable();
    assert_eq!(
        seen, expected,
        "with no rect the sprite has to cover all four cells, and it drew {whole:?}"
    );

    let (_, one_cell) = sample(
        &gpu,
        UvRect::cell(0, 0, 2, 2).expect("a cell of a two by two sheet"),
    );
    for (index, pixel) in one_cell.iter().enumerate() {
        assert_near(*pixel, TOP_LEFT, &format!("quadrant {index} of one cell"));
    }
}
