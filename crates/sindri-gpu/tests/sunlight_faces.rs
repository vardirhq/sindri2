//! Which side of a surface the sun lights.
//!
//! The textured shader works out which way a surface faces from how its
//! position changes across the screen, and the sign of that is easy to get
//! backwards: a framebuffer's rows run downward. Backwards, every face turned
//! away from the sun is the one lit, and a scene looks lit from the opposite
//! side to where its sun is aimed. Only a drawn frame can say which it is.

use glam::Vec3;
use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{
    ClearOperations, DepthTarget, DrawContext, OffscreenTarget, PerspectiveCamera, Texture2D,
    TextureRegistry, TexturedCubeRenderer, TexturedVertex, WorldLighting, encode_clear,
};

const REQUIRE_GPU: &str = "SINDRI_REQUIRE_GPU";
const SIZE: u32 = 32;

/// The centre of a white floor seen from above, lit only by a sun travelling
/// along `direction`, or `None` without an adapter.
fn floor_brightness(direction: [f32; 3]) -> Option<u8> {
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
    let target = OffscreenTarget::new(&gpu.device, SIZE, SIZE).expect("the target is valid");
    let depth = DepthTarget::new(&gpu.device, SIZE, SIZE);
    let mut textures = TextureRegistry::new(&gpu.device, &gpu.queue);
    let white = textures.insert(
        Texture2D::from_rgba8(&gpu.device, &gpu.queue, "white", 2, 2, &[255; 16])
            .expect("a two by two texture is valid"),
    );
    let mut cube = TexturedCubeRenderer::new(&gpu.device, OffscreenTarget::FORMAT);
    cube.set_lighting(WorldLighting {
        ambient_color: [1.0; 3],
        ambient_intensity: 0.0,
        directional_direction: direction,
        directional_color: [1.0; 3],
        directional_intensity: 1.0,
    });
    // Looking straight down at the floor, with north up the screen.
    let view_projection = PerspectiveCamera {
        eye: Vec3::new(0.0, 5.0, 0.0),
        target: Vec3::ZERO,
        up: Vec3::NEG_Z,
        vertical_fov_radians: std::f32::consts::FRAC_PI_4,
        near: 0.1,
        far: 100.0,
    }
    .view_projection(1.0);
    let corner = |x: f32, z: f32, u: f32, v: f32| TexturedVertex::new([x, 0.0, z], [u, v]);
    let vertices = [
        corner(-3.0, -3.0, 0.0, 0.0),
        corner(3.0, -3.0, 1.0, 0.0),
        corner(3.0, 3.0, 1.0, 1.0),
        corner(-3.0, 3.0, 0.0, 1.0),
    ];
    // Both windings, so the answer is about lighting and never about which
    // side the pipeline culls.
    let indices = [0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2];
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sindri sunlight test encoder"),
        });
    encode_clear(&mut encoder, target.view(), &depth, ClearOperations::default());
    cube.encode_mesh(
        DrawContext {
            device: &gpu.device,
            queue: &gpu.queue,
            textures: &textures,
            texture: white,
        },
        &mut encoder,
        (target.view(), &depth),
        view_projection,
        &vertices,
        &indices,
    );
    let readback = target
        .copy_to_buffer(&gpu.device, &mut encoder)
        .expect("the target copies back");
    gpu.queue.submit([encoder.finish()]);
    let pixels = readback.read_rgba8(&gpu.device).expect("the frame reads back");
    let middle = ((SIZE / 2) * SIZE + SIZE / 2) as usize * 4;
    Some(pixels[middle])
}

#[test]
fn a_sun_shining_down_lights_the_top_of_the_floor() {
    let Some(brightness) = floor_brightness([0.0, -1.0, 0.0]) else {
        return;
    };
    assert!(brightness > 200, "a floor under the sun is lit: {brightness}");
}

#[test]
fn a_sun_shining_up_leaves_the_top_of_the_floor_dark() {
    let Some(brightness) = floor_brightness([0.0, 1.0, 0.0]) else {
        return;
    };
    assert!(brightness < 30, "a floor lit from below is dark on top: {brightness}");
}
