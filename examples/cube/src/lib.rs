use std::time::Duration;

use glam::Vec2;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::AssetId;
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{InputState, Key};
use sindri_render::{
    DepthTarget, DrawContext, FrameCommand, PreparedFrame, SpriteBatchError, SpriteBatchRenderer,
    SpriteBatchStats, Texture2D, TextureRegistry, TexturedCubeRenderer, Viewport,
};
use thiserror::Error;

mod scene;

pub use scene::{DemoScene, DemoSceneError};
pub use sindri_scene::{CameraView, TextureBindings, WorldProjection};

#[derive(Clone, Copy)]
pub struct FrameTarget<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a DepthTarget,
}

/// The demo as the windowed host runs it.
///
/// Gameplay writes the world and drawing reads it; nothing in between tells the
/// renderer anything happened.
struct CubeApp {
    depth: DepthTarget,
    cube_renderer: TexturedCubeRenderer,
    sprite_renderer: SpriteBatchRenderer,
    textures: TextureRegistry,
    bindings: TextureBindings,
    scene: DemoScene,
    rotation: Vec2,
}

/// How fast the arrow keys turn the cube, in radians per second.
const ROTATION_RATE: f32 = 1.8;

/// The longest frame the demo will integrate over.
///
/// `sindri-core`'s fixed-step clock does this properly, with a fixed simulation
/// rate and spiral-of-death protection. This example does not run through the
/// engine loop yet, so it caps its own delta rather than letting one stalled
/// frame spin the cube half a turn.
const LONGEST_FRAME: f32 = 0.1;

impl DesktopApp for CubeApp {
    type Error = CubeError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let (textures, bindings) = demo_textures(context.device(), context.queue());
        Ok(Self {
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cube_renderer: TexturedCubeRenderer::new(context.device(), context.format()),
            sprite_renderer: SpriteBatchRenderer::new(context.device(), context.format()),
            textures,
            bindings,
            scene: DemoScene::load()?,
            rotation: Vec2::ZERO,
        })
    }

    fn update(&mut self, input: &InputState, delta: Duration) -> Result<Flow, Self::Error> {
        if input.key_down(Key::Escape) {
            return Ok(Flow::Exit);
        }

        let seconds = delta.as_secs_f32().min(LONGEST_FRAME);
        let axis = Vec2::new(
            input.axis(Key::ArrowLeft, Key::ArrowRight),
            input.axis(Key::ArrowUp, Key::ArrowDown),
        );
        self.rotation += axis * seconds * ROTATION_RATE;
        // Gameplay writes the world. Extraction reads whatever it now holds.
        self.scene.spin_cube(self.rotation)?;
        Ok(Flow::Continue)
    }

    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> {
        self.depth
            .resize(context.device(), context.width(), context.height());
        Ok(())
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        let viewport = Viewport::new(context.width(), context.height());
        let prepared = self.scene.extract_frame(viewport, &self.bindings)?;
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri cube encoder"),
                });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut self.cube_renderer,
                sprites: &mut self.sprite_renderer,
                textures: &self.textures,
            },
            context.device(),
            context.queue(),
            &mut encoder,
            FrameTarget {
                color: view,
                depth: &self.depth,
            },
            &prepared,
        )?;
        context.queue().submit([encoder.finish()]);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CubeError {
    #[error(transparent)]
    Scene(#[from] DemoSceneError),
    #[error(transparent)]
    Batch(#[from] SpriteBatchError),
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let _ = console_log::init_with_level(log::Level::Info);
    }
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    if let Err(error) = sindri_desktop::run::<CubeApp>(WindowConfig::new(
        "Sindri — shared native/web textured cube",
    )) {
        log::error!("{error}");
    }
}

/// The demo's textures, registered under the references its scene names.
///
/// The checkerboard is generated, which is what `procedural:` marks; the badge
/// is a real PNG decoded through the asset pipeline. Both bind the same way,
/// because binding cares about the reference, not where the pixels came from.
pub fn demo_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (TextureRegistry, TextureBindings) {
    let mut registry = TextureRegistry::new(device, queue);
    let mut bindings = TextureBindings::new();

    let checkerboard = registry.insert(
        Texture2D::checkerboard(
            device,
            queue,
            "Sindri cube checkerboard",
            64,
            8,
            [[18, 34, 55, 255], [240, 114, 43, 255]],
        )
        .expect("static checkerboard texture dimensions are valid"),
    );
    bindings.bind("procedural:checkerboard", checkerboard);

    let badge = registry.insert(decode_texture(
        device,
        queue,
        "textures/badge.png",
        BADGE_PNG,
    ));
    bindings.bind("textures/badge.png", badge);

    (registry, bindings)
}

/// The badge image, decoded from a real PNG through the asset pipeline.
///
/// Embedded rather than read from disk so the example needs no I/O on either
/// target; the bytes still travel the same decode path a file or fetch would.
const BADGE_PNG: &[u8] = include_bytes!("../assets/textures/badge.png");

/// The badge as raw RGBA, which `assets/textures/badge.png` encodes.
///
/// Kept so a test can prove the shipped PNG is exactly this image, rather than
/// trusting that swapping a generator for a file left the frame unchanged.
pub fn demo_badge_pixels() -> Vec<u8> {
    const SIZE: u32 = 64;
    let mut pixels =
        Vec::with_capacity(usize::try_from(SIZE * SIZE * 4).expect("badge fits usize"));
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = i64::from(x) - 31;
            let dy = i64::from(y) - 31;
            let distance_squared = dx * dx + dy * dy;
            let bolt = (30..=38).contains(&x) && (12..=31).contains(&y)
                || (24..=38).contains(&x) && (27..=36).contains(&y)
                || (24..=32).contains(&x) && (32..=51).contains(&y);
            let color = if distance_squared > 31 * 31 {
                [0, 0, 0, 0]
            } else if distance_squared > 27 * 27 {
                [240, 114, 43, 255]
            } else if bolt {
                [255, 244, 214, 255]
            } else {
                [18, 34, 55, 235]
            };
            pixels.extend_from_slice(&color);
        }
    }
    pixels
}

/// Decodes an embedded PNG into an upload-ready texture.
///
/// This is the whole bridge: `sindri-assets` turns bytes into a `TextureAsset`,
/// and `sindri-render` turns that into something drawable. A file read or an
/// HTTP fetch produces the same bytes and joins here.
pub fn decode_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    id: &str,
    bytes: &[u8],
) -> Texture2D {
    let asset_id = AssetId::new(id).expect("demo asset IDs are valid");
    let decoded = TextureAssetDecoder
        .decode(AssetBytes::new(asset_id, bytes.to_vec()))
        .expect("the embedded texture decodes");
    Texture2D::from_rgba8(
        device,
        queue,
        id,
        decoded.width(),
        decoded.height(),
        decoded.rgba8(),
    )
    .expect("a decoded texture has valid dimensions")
}

/// The renderers a frame is drawn with.
pub struct FrameRenderers<'a> {
    pub cube: &'a mut TexturedCubeRenderer,
    pub sprites: &'a mut SpriteBatchRenderer,
    pub textures: &'a TextureRegistry,
}

pub fn encode_prepared_frame(
    renderers: FrameRenderers<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: FrameTarget<'_>,
    frame: &PreparedFrame,
) -> Result<SpriteBatchStats, SpriteBatchError> {
    let FrameRenderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        textures,
    } = renderers;
    let mut sprite_stats = SpriteBatchStats::default();
    for pass in frame.passes() {
        match &pass.command {
            FrameCommand::TexturedCube { model, texture } => cube_renderer.encode_with_clear(
                DrawContext {
                    device,
                    queue,
                    textures,
                    texture: *texture,
                },
                encoder,
                target.color,
                target.depth,
                pass.camera.view_projection * *model,
                frame.clear(),
            ),
            FrameCommand::SpriteBatch { texture, instances } => {
                sprite_stats =
                    sprite_renderer.prepare(device, queue, textures, *texture, instances)?;
                sprite_renderer.encode(queue, encoder, target.color, pass.camera.view_projection);
            }
        }
    }
    Ok(sprite_stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped PNG must be exactly the image the demo used to generate, so
    /// moving the badge onto the asset pipeline cannot change what is drawn.
    #[test]
    fn the_badge_png_decodes_to_the_authored_image() {
        let id = AssetId::new("textures/badge.png").expect("a valid asset ID");
        let decoded = TextureAssetDecoder
            .decode(AssetBytes::new(id, BADGE_PNG.to_vec()))
            .expect("the badge PNG decodes");

        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);
        assert_eq!(
            decoded.rgba8(),
            demo_badge_pixels().as_slice(),
            "the PNG and the generated image must be the same pixels"
        );
    }

    /// Scene texture references are real asset IDs, not arbitrary strings.
    #[test]
    fn every_scene_texture_reference_is_a_loadable_asset_id() {
        let document = DemoScene::authored_document().expect("the demo scene parses");
        let mut checked = 0;
        for entity in &document.entities {
            for payload in entity.components.values() {
                let Some(reference) = payload.get("texture").and_then(|value| value.as_str())
                else {
                    continue;
                };
                checked += 1;
                if let Some(generated) = reference.strip_prefix("procedural:") {
                    assert!(!generated.is_empty(), "a generated texture needs a name");
                    continue;
                }
                AssetId::new(reference).unwrap_or_else(|error| {
                    panic!("scene texture reference {reference:?} is not loadable: {error}")
                });
            }
        }
        assert!(checked > 0, "the demo scene should reference textures");
    }
}
