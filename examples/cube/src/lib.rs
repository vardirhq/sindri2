use std::time::Duration;

use glam::Vec2;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::AssetId;
use sindri_core::FixedStepConfig;
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{EngineHost, FrameContext, Game, HostError, InputEvent, Key};
use sindri_render::{
    DepthTarget, FrameEncodeError, FrameRenderers, FrameTarget, GlyphRenderer, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, Texture2D, TextureRegistry, TexturedCubeRenderer, Viewport,
    encode_prepared_frame,
};
use thiserror::Error;

mod colors;
mod scene;

pub use colors::{
    AUTHORED_COLORS, CHANNEL_TOLERANCE, MINIMUM_SHARE_PER_THOUSAND, verify_authored_colors,
};
pub use scene::{DemoScene, DemoSceneError, spin_cube};
pub use sindri_scene::{CameraView, TextureBindings, WorldProjection};

/// The demo's gameplay: the half that only writes the world.
///
/// It never sees a device, a texture, or a frame. Rotating the cube is a
/// transform written into whatever world the engine hands it, and the next
/// extraction reads the world as it now is.
#[derive(Debug, Default)]
struct CubeGame {
    rotation: Vec2,
}

/// How fast the arrow keys turn the cube, in radians per second.
const ROTATION_RATE: f32 = 1.8;

impl Game for CubeGame {
    type Error = DemoSceneError;

    /// Runs at the fixed simulation rate, so the cube turns at the same speed
    /// whatever the frame rate is. The engine caps and accumulates the real
    /// delta; nothing here has to guard against a stalled frame.
    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let axis = Vec2::new(
            context.input.axis(Key::ArrowLeft, Key::ArrowRight),
            context.input.axis(Key::ArrowUp, Key::ArrowDown),
        );
        self.rotation += axis * context.time.delta.as_secs_f32() * ROTATION_RATE;
        spin_cube(context.world, self.rotation)
    }
}

/// The demo as the windowed host runs it.
///
/// The engine owns the world and the simulation; this owns the GPU resources
/// and the drawing. Neither reaches into the other.
struct CubeApp {
    engine: EngineHost<CubeGame>,
    scene: DemoScene,
    depth: DepthTarget,
    cube_renderer: TexturedCubeRenderer,
    sprite_renderer: SpriteBatchRenderer,
    text_renderer: TextRenderer,
    glyph_renderer: GlyphRenderer,
    shape_renderer: ShapeRenderer,
    textures: TextureRegistry,
    bindings: TextureBindings,
}

impl DesktopApp for CubeApp {
    type Error = CubeError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let (textures, bindings) = demo_textures(context.device(), context.queue());
        let (scene, world) = DemoScene::load()?;
        let mut engine = EngineHost::new(CubeGame::default(), FixedStepConfig::default())?;
        *engine.world_mut() = world;
        engine.start()?;
        Ok(Self {
            engine,
            scene,
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cube_renderer: TexturedCubeRenderer::new(context.device(), context.format()),
            sprite_renderer: SpriteBatchRenderer::new(context.device(), context.format()),
            text_renderer: TextRenderer::new(),
            glyph_renderer: GlyphRenderer::new(context.device(), context.format()),
            shape_renderer: ShapeRenderer::new(context.device(), context.format()),
            textures,
            bindings,
        })
    }

    fn input(&mut self, event: InputEvent) {
        self.engine.queue_input(event);
    }

    fn update(&mut self, delta: Duration) -> Result<Flow, Self::Error> {
        if self.engine.input().key_down(Key::Escape) {
            return Ok(Flow::Exit);
        }
        self.engine.advance(delta)?;
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
        let prepared = self
            .scene
            .extract_frame(self.engine.world(), viewport, &self.bindings)?;
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
                text: &mut self.text_renderer,
                glyphs: &mut self.glyph_renderer,
                shapes: &mut self.shape_renderer,
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
    Frame(#[from] FrameEncodeError),
    #[error(transparent)]
    Host(#[from] HostError<DemoSceneError>),
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

    for procedural in sindri_scene::PROCEDURAL_TEXTURES {
        let texture = registry.insert(
            Texture2D::checkerboard(
                device,
                queue,
                procedural.reference,
                procedural.size,
                procedural.cells,
                procedural.colors,
            )
            .expect("built-in procedural texture dimensions are valid"),
        );
        bindings.bind(procedural.reference, texture);
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the demo's gameplay for one simulated second, delivered as `frames`
    /// equal slices, and reports where the cube ended up.
    fn cube_rotation_after_a_second(frames: u32) -> [f32; 4] {
        let (_, world) = DemoScene::load().expect("the demo scene loads");
        let mut engine = EngineHost::new(CubeGame::default(), FixedStepConfig::default())
            .expect("the host starts");
        *engine.world_mut() = world;
        engine.start().expect("the engine starts");
        engine.queue_input(InputEvent::KeyPressed(Key::ArrowRight));

        let slice = Duration::from_secs(1) / frames;
        for _ in 0..frames {
            engine.advance(slice).expect("the demo never fails a frame");
        }

        let cube = engine
            .world()
            .entities()
            .find(|(_, data)| data.components.contains_key("sindri.mesh"))
            .map(|(entity, _)| entity)
            .expect("the demo scene keeps its cube");
        engine
            .world()
            .get(cube)
            .and_then(|data| data.transform_3d)
            .expect("the cube carries a 3D transform")
            .rotation
    }

    /// The reason gameplay moved onto the engine loop. The cube used to
    /// integrate whatever frame delta it was handed, so the same second of held
    /// input turned it further on a slow machine than a fast one. Fixed steps
    /// make a second of input a second of rotation.
    #[test]
    fn a_second_of_input_turns_the_cube_the_same_amount_at_any_frame_rate() {
        let at_60 = cube_rotation_after_a_second(60);
        let at_15 = cube_rotation_after_a_second(15);
        let at_144 = cube_rotation_after_a_second(144);

        for (fast, slow) in at_144.iter().zip(at_15) {
            assert!(
                (fast - slow).abs() < 1.0e-5,
                "frame rate changed the result: {at_144:?} against {at_15:?}"
            );
        }
        for (reference, other) in at_60.iter().zip(at_15) {
            assert!(
                (reference - other).abs() < 1.0e-5,
                "frame rate changed the result: {at_60:?} against {at_15:?}"
            );
        }
        assert!(
            at_60[3] < 0.999,
            "holding right for a second should have turned the cube, got {at_60:?}"
        );
    }

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
