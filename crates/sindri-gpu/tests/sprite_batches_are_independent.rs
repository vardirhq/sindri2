//! Does each sprite batch draw with its own camera and its own instances?
//!
//! It did not. `queue.write_buffer` stages a write that lands *before* the
//! command buffer executes, so a renderer holding one uniform buffer and one
//! instance buffer wrote both once per batch and every pass in the frame drew
//! with whatever the last batch had put there. A frame with a single sprite
//! batch is unaffected, which is every proof and every test the workspace had —
//! and every scene that mixes world sprites with a screen overlay was drawing
//! the world through the overlay's camera.
//!
//! Found by building a game: the floor rendered at the heads-up display's
//! scale, which is the sort of thing only something with both ever notices.

use glam::{Mat4, Vec3};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    ClearOperations, DepthTarget, OffscreenTarget, OrthographicCamera, SpriteBatchRenderer,
    SpriteDepth, SpriteInstance, Texture2D, TextureRegistry, encode_clear,
};

const REQUIRE_GPU: &str = "SINDRI_REQUIRE_GPU";
const SIZE: u32 = 128;
const CHANNEL_TOLERANCE: i32 = 4;

const WIDE: [u8; 4] = [220, 60, 60, 255];
const NARROW: [u8; 4] = [60, 120, 240, 255];

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

fn solid(gpu: &GpuContext, label: &str, color: [u8; 4]) -> Texture2D {
    Texture2D::from_rgba8(&gpu.device, &gpu.queue, label, 1, 1, &color)
        .expect("a one-pixel texture is valid")
}

/// Two batches, two cameras, one frame.
///
/// The first draws a sprite through a camera that frames four units, so a
/// two-unit sprite covers half the height. The second draws through a camera
/// framing sixteen, so the same sprite covers an eighth. If the batches share a
/// uniform, the first is drawn at the second's scale and shrinks to nothing.
#[test]
fn each_batch_draws_with_its_own_camera() {
    let Some(gpu) = gpu() else {
        return;
    };
    let target = OffscreenTarget::new(&gpu.device, SIZE, SIZE).expect("the target is valid");
    let depth = DepthTarget::new(&gpu.device, SIZE, SIZE);
    let mut textures = TextureRegistry::new(&gpu.device, &gpu.queue);
    let wide = textures.insert(solid(&gpu, "wide", WIDE));
    let narrow = textures.insert(solid(&gpu, "narrow", NARROW));
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);

    let camera = |vertical_size: f32| {
        OrthographicCamera {
            center: glam::Vec2::ZERO,
            vertical_size,
            near: 0.0,
            far: 10.0,
        }
        .view_projection(1.0)
    };
    let quad = SpriteInstance::new(
        Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0)),
        [1.0, 1.0, 1.0, 1.0],
    );

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri two batch encoder"),
        });
    encode_clear(
        &mut encoder,
        target.view(),
        &depth,
        ClearOperations::default(),
    );
    sprites.begin_submission();
    for (texture, vertical_size) in [(wide, 4.0), (narrow, 16.0)] {
        sprites
            .draw(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                target.view(),
                &depth,
                &textures,
                texture,
                camera(vertical_size),
                SpriteDepth::Ignore,
                &[quad],
            )
            .expect("one sprite fits the batch");
    }
    let readback = target
        .copy_to_buffer(&gpu.device, &mut encoder)
        .expect("the target copies back");
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback
        .read_rgba8(&gpu.device)
        .expect("the frame reads back");

    let at = |x: u32, y: u32| {
        let i = (y * SIZE + x) as usize * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let near = |actual: [u8; 4], expected: [u8; 4]| {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (i32::from(*a) - i32::from(e)).abs() <= CHANNEL_TOLERANCE)
    };

    // Between the two sprites' edges: the wide camera's sprite reaches three
    // quarters across, the narrow one's barely past the middle. This is the
    // pixel that was the clear colour while the bug was there.
    let between = at(SIZE / 2 + SIZE / 8, SIZE / 2);
    assert!(
        near(between, WIDE),
        "the first batch drew at the second batch's scale: {between:?}"
    );
    // And the second batch is still small and on top, so it has not simply
    // taken the first one's camera either.
    assert!(
        near(at(SIZE / 2, SIZE / 2), NARROW),
        "the second batch draws over the first at its own scale"
    );
    let outside = at(SIZE / 2 + SIZE / 2 - 2, SIZE / 2);
    assert!(
        !near(outside, WIDE) && !near(outside, NARROW),
        "and neither batch covers the whole frame: {outside:?}"
    );
}

/// The instances are per batch too: a second batch with fewer sprites must not
/// leave the first drawing the second's.
#[test]
fn each_batch_draws_its_own_instances() {
    let Some(gpu) = gpu() else {
        return;
    };
    let target = OffscreenTarget::new(&gpu.device, SIZE, SIZE).expect("the target is valid");
    let depth = DepthTarget::new(&gpu.device, SIZE, SIZE);
    let mut textures = TextureRegistry::new(&gpu.device, &gpu.queue);
    let wide = textures.insert(solid(&gpu, "wide", WIDE));
    let narrow = textures.insert(solid(&gpu, "narrow", NARROW));
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let view = OrthographicCamera {
        center: glam::Vec2::ZERO,
        vertical_size: 4.0,
        near: 0.0,
        far: 10.0,
    }
    .view_projection(1.0);

    let quad = |x: f32| {
        SpriteInstance::new(
            Mat4::from_translation(Vec3::new(x, 0.0, 0.0))
                * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0)),
            [1.0, 1.0, 1.0, 1.0],
        )
    };

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri instance count encoder"),
        });
    encode_clear(
        &mut encoder,
        target.view(),
        &depth,
        ClearOperations::default(),
    );
    sprites.begin_submission();
    // Two sprites on the left and right, then one in the middle.
    sprites
        .draw(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            target.view(),
            &depth,
            &textures,
            wide,
            view,
            SpriteDepth::Ignore,
            &[quad(-1.2), quad(1.2)],
        )
        .expect("two sprites fit");
    sprites
        .draw(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            target.view(),
            &depth,
            &textures,
            narrow,
            view,
            SpriteDepth::Ignore,
            &[quad(0.0)],
        )
        .expect("one sprite fits");
    let readback = target
        .copy_to_buffer(&gpu.device, &mut encoder)
        .expect("the target copies back");
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback
        .read_rgba8(&gpu.device)
        .expect("the frame reads back");

    let at = |x: u32, y: u32| {
        let i = (y * SIZE + x) as usize * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let near = |actual: [u8; 4], expected: [u8; 4]| {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (i32::from(*a) - i32::from(e)).abs() <= CHANNEL_TOLERANCE)
    };

    let left = at(SIZE / 4, SIZE / 2);
    assert!(
        near(left, WIDE),
        "the left sprite of the first batch: {left:?}"
    );
    let right = at(SIZE - SIZE / 4, SIZE / 2);
    assert!(near(right, WIDE), "and its right one: {right:?}");
    let middle = at(SIZE / 2, SIZE / 2);
    assert!(near(middle, NARROW), "the second batch's one: {middle:?}");
}
