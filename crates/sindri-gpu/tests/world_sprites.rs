//! What a world-space sprite does that a screen-anchored one does not.
//!
//! Everything else about sprites is provable without a device. Where an
//! instance lands is arithmetic, and `sindri-scene`'s extraction tests check
//! it. Occlusion is not: it happens in a depth comparison inside a pipeline, so
//! the only honest check is to draw the frame and look at the pixels.
//!
//! This is the first test in the workspace that needs a real adapter, which is
//! why `sindri-render` has been a dev-dependency of `sindri-gpu` since before
//! there was one to run.

use glam::{Mat4, Vec3};
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    ClearOperations, DepthTarget, DrawContext, OffscreenTarget, PerspectiveCamera,
    SpriteBatchRenderer, SpriteDepth, SpriteInstance, Texture2D, TextureRegistry,
    TexturedCubeRenderer, encode_clear,
};

/// Set wherever a software adapter is installed on purpose. A GPU test that
/// skips on the machine that exists to run it is a check that quietly stopped
/// checking, so CI demands the adapter rather than hoping for it.
const REQUIRE_GPU: &str = "SINDRI_REQUIRE_GPU";

const SIZE: u32 = 64;
/// How far a channel may drift before the colour is a different one. Rounding
/// through an sRGB target moves a byte or so; a wrong sprite moves all of them.
const CHANNEL_TOLERANCE: i32 = 3;
const MESH_COLOR: [u8; 4] = [18, 34, 55, 255];
const SPRITE_COLOR: [u8; 4] = [40, 200, 90, 255];

/// The centre pixel of a frame holding a sprite, optionally over a mesh.
///
/// The sprite sits at `sprite_z`: positive is between the camera and the mesh,
/// negative is behind it. Both are centred, so the answer is only ever one of
/// the two colours, or the cleared background, and which one is the question.
fn center_pixel(mesh: bool, sprite_z: f32, depth: SpriteDepth) -> Option<[u8; 4]> {
    let instance = wgpu::Instance::default();
    let gpu = match pollster::block_on(GpuContext::request(
        &instance,
        None,
        &GpuRequestOptions::default(),
    )) {
        Ok(gpu) => gpu,
        Err(error) => {
            assert!(
                std::env::var_os(REQUIRE_GPU).is_none(),
                "{REQUIRE_GPU} is set but no adapter could be requested: {error}"
            );
            eprintln!("skipping: no GPU adapter ({error})");
            return None;
        }
    };

    let target =
        OffscreenTarget::new(&gpu.device, SIZE, SIZE).expect("a 64 square target is valid");
    let depth_target = DepthTarget::new(&gpu.device, SIZE, SIZE);
    let mut textures = TextureRegistry::new(&gpu.device, &gpu.queue);
    let mesh_texture = textures.insert(solid(&gpu, "mesh", MESH_COLOR));
    let sprite_texture = textures.insert(solid(&gpu, "sprite", SPRITE_COLOR));
    let mut cube = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    let mut sprites = SpriteBatchRenderer::new(&gpu.device, OffscreenTarget::FORMAT);

    // One camera for both, which is the point: a world sprite is drawn through
    // the camera that draws the world.
    let view_projection = PerspectiveCamera {
        eye: Vec3::new(0.0, 0.0, 5.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        vertical_fov_radians: std::f32::consts::FRAC_PI_4,
        near: 0.1,
        far: 100.0,
    }
    .view_projection(1.0);

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri world sprite test encoder"),
        });
    encode_clear(
        &mut encoder,
        target.view(),
        &depth_target,
        ClearOperations::default(),
    );
    if mesh {
        cube.encode(
            DrawContext {
                device: &gpu.device,
                queue: &gpu.queue,
                textures: &textures,
                texture: mesh_texture,
            },
            &mut encoder,
            target.view(),
            &depth_target,
            view_projection,
        );
    }
    sprites.begin_submission();
    sprites
        .draw(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            target.view(),
            &depth_target,
            &textures,
            sprite_texture,
            view_projection,
            depth,
            &[SpriteInstance::new(
                Mat4::from_translation(Vec3::new(0.0, 0.0, sprite_z)),
                [1.0, 1.0, 1.0, 1.0],
            )],
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
    Some([
        pixels[middle],
        pixels[middle + 1],
        pixels[middle + 2],
        pixels[middle + 3],
    ])
}

fn solid(gpu: &GpuContext, label: &str, color: [u8; 4]) -> Texture2D {
    Texture2D::from_rgba8(&gpu.device, &gpu.queue, label, 2, 2, &color.repeat(4))
        .expect("a two by two texture is valid")
}

fn assert_color(actual: [u8; 4], expected: [u8; 4], what: &str) {
    let drift = actual
        .iter()
        .zip(expected)
        .map(|(left, right)| (i32::from(*left) - i32::from(right)).abs())
        .max()
        .unwrap_or_default();
    assert!(
        drift <= CHANNEL_TOLERANCE,
        "{what}: expected {expected:?}, got {actual:?}"
    );
}

/// The reason world-space sprites test depth at all: a sprite standing behind
/// something opaque is behind it, rather than painted over it.
#[test]
fn a_world_sprite_behind_a_mesh_is_hidden_by_it() {
    let Some(pixel) = center_pixel(true, -3.0, SpriteDepth::Test) else {
        return;
    };
    assert_color(pixel, MESH_COLOR, "a sprite behind the cube");
}

/// And the same sprite moved in front of the mesh covers it, so the test above
/// is not passing because the sprite failed to draw at all.
#[test]
fn a_world_sprite_in_front_of_a_mesh_covers_it() {
    let Some(pixel) = center_pixel(true, 3.0, SpriteDepth::Test) else {
        return;
    };
    assert_color(pixel, SPRITE_COLOR, "a sprite in front of the cube");
}

/// A screen-space overlay is not in the world, so the world cannot hide it.
/// Same sprite, same place, the other depth behaviour.
#[test]
fn ignoring_depth_draws_a_sprite_over_the_mesh_wherever_it_is() {
    let Some(pixel) = center_pixel(true, -3.0, SpriteDepth::Ignore) else {
        return;
    };
    assert_color(pixel, SPRITE_COLOR, "an overlay sprite behind the cube");
}

/// A scene of only sprites is what a 2D game is, and it has no mesh pass to
/// have cleared the depth buffer on its way past. If the frame's own clear ever
/// stops happening, a depth-tested sprite fails against a buffer of zeroes and
/// the whole game disappears — so this is the test that notices.
#[test]
fn a_world_sprite_alone_in_a_frame_is_still_drawn() {
    let Some(pixel) = center_pixel(false, -3.0, SpriteDepth::Test) else {
        return;
    };
    assert_color(pixel, SPRITE_COLOR, "the only sprite in the frame");
}
