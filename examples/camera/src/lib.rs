use std::time::Duration;

use serde_json::Value;
use sindri_core::{FixedStepConfig, SceneComponent, SceneDocument, SceneEntityId, World};
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{EngineHost, FrameContext, Game, InputEvent, InputState, Key};
use sindri_render::{
    DepthTarget, FrameEncodeError, FrameRenderers, FrameTarget, GlyphRenderer, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, TextureRegistry, TexturedCubeRenderer, Viewport,
    encode_prepared_frame,
};
use sindri_scene::{
    CameraBehaviorComponent, SceneExtractError, SceneExtractor, TextureBindings,
    update_camera_behaviors,
};
use thiserror::Error;

const SCENE_JSON: &str = include_str!("../assets/demo.scene.json");
const TARGET_SPEED: f32 = 5.0;

#[derive(Default)]
struct CameraGame;

impl Game for CameraGame {
    type Error = DemoError;

    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let dt = context.time.delta.as_secs_f32();
        move_target(context.world, context.input, dt)?;
        if context.input.key_pressed(Key::Space) {
            add_trauma(context.world, 1.0)?;
        }
        update_camera_behaviors(context.world, dt);
        Ok(())
    }
}

fn move_target(world: &mut World, input: &InputState, dt: f32) -> Result<(), DemoError> {
    let target = world
        .entity_for_source_id(&scene_id("target"))
        .ok_or(DemoError::Missing("target"))?;
    let data = world.get_mut(target).ok_or(DemoError::Missing("target"))?;
    let mut transform = data.transform_3d.unwrap_or_default();
    transform.position[0] += input.axis(Key::ArrowLeft, Key::ArrowRight) * TARGET_SPEED * dt;
    transform.position[1] += input.axis(Key::ArrowDown, Key::ArrowUp) * TARGET_SPEED * dt;
    data.transform_3d = Some(transform);
    Ok(())
}

fn add_trauma(world: &mut World, amount: f32) -> Result<(), DemoError> {
    let camera = world
        .entity_for_source_id(&scene_id("camera"))
        .ok_or(DemoError::Missing("camera"))?;
    let data = world.get_mut(camera).ok_or(DemoError::Missing("camera"))?;
    let payload = data
        .components
        .get_mut(CameraBehaviorComponent::TYPE_NAME)
        .ok_or(DemoError::Missing("camera behavior"))?;
    let trauma = payload
        .get_mut("shake")
        .and_then(Value::as_object_mut)
        .and_then(|shake| shake.get_mut("trauma"))
        .ok_or(DemoError::Missing("shake trauma"))?;
    *trauma = Value::from((trauma.as_f64().unwrap_or(0.0) + f64::from(amount)).min(1.0));
    Ok(())
}

fn scene_id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("demo scene IDs are valid")
}

struct CameraApp {
    engine: EngineHost<CameraGame>,
    extractor: SceneExtractor,
    depth: DepthTarget,
    cube: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
    glyphs: GlyphRenderer,
    shapes: ShapeRenderer,
    textures: TextureRegistry,
}

impl DesktopApp for CameraApp {
    type Error = DemoError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let extractor = SceneExtractor::new()?;
        let document = SceneDocument::from_json(SCENE_JSON)?;
        extractor.validate(&document, sindri_core::UnknownComponentPolicy::Reject)?;
        let mut engine = EngineHost::new(CameraGame, FixedStepConfig::default())
            .map_err(|error| DemoError::Host(error.to_string()))?;
        *engine.world_mut() = World::from_scene(&document)?.world;
        engine
            .start()
            .map_err(|error| DemoError::Host(error.to_string()))?;
        Ok(Self {
            engine,
            extractor,
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cube: TexturedCubeRenderer::new(context.device(), context.format()),
            sprites: SpriteBatchRenderer::new(context.device(), context.format()),
            text: TextRenderer::new(),
            glyphs: GlyphRenderer::new(context.device(), context.format()),
            shapes: ShapeRenderer::new(context.device(), context.format()),
            textures: TextureRegistry::new(context.device(), context.queue()),
        })
    }

    fn input(&mut self, event: InputEvent) {
        self.engine.queue_input(event);
    }

    fn update(&mut self, delta: Duration) -> Result<Flow, Self::Error> {
        if self.engine.input().key_down(Key::Escape) {
            return Ok(Flow::Exit);
        }
        self.engine
            .advance(delta)
            .map_err(|error| DemoError::Host(error.to_string()))?;
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
        let prepared = self.extractor.extract(
            self.engine.world(),
            viewport,
            sindri_scene::CameraView::default(),
            &TextureBindings::new(),
        )?;
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri camera demo encoder"),
                });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut self.cube,
                sprites: &mut self.sprites,
                text: &mut self.text,
                glyphs: &mut self.glyphs,
                shapes: &mut self.shapes,
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
enum DemoError {
    #[error(transparent)]
    SceneJson(#[from] sindri_core::SceneJsonError),
    #[error(transparent)]
    World(#[from] sindri_core::WorldError),
    #[error(transparent)]
    Extract(#[from] SceneExtractError),
    #[error(transparent)]
    Frame(#[from] FrameEncodeError),
    #[error("engine host failed: {0}")]
    Host(String),
    #[error(transparent)]
    Registry(#[from] sindri_core::ComponentRegistryError),
    #[error("camera demo is missing its {0}")]
    Missing(&'static str),
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

    if let Err(error) =
        sindri_desktop::run::<CameraApp>(WindowConfig::new("Sindri - camera behavior demo"))
    {
        log::error!("{error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_follows_the_authored_target_on_the_fixed_step_path() {
        let document = SceneDocument::from_json(SCENE_JSON).unwrap();
        let mut engine = EngineHost::new(CameraGame, FixedStepConfig::default()).unwrap();
        *engine.world_mut() = World::from_scene(&document).unwrap().world;
        engine.start().unwrap();
        engine.queue_input(InputEvent::KeyPressed(Key::ArrowRight));
        for _ in 0..60 {
            engine.advance(Duration::from_secs_f32(1.0 / 60.0)).unwrap();
        }
        let camera = engine
            .world()
            .entity_for_source_id(&scene_id("camera"))
            .unwrap();
        let x = engine
            .world()
            .get(camera)
            .unwrap()
            .transform_3d
            .unwrap()
            .position[0];
        assert!(x > 0.5, "camera should have followed right, got {x}");
    }

    #[test]
    fn space_adds_trauma_and_the_behavior_consumes_it() {
        let document = SceneDocument::from_json(SCENE_JSON).unwrap();
        let mut world = World::from_scene(&document).unwrap().world;
        add_trauma(&mut world, 1.0).unwrap();
        let camera = world.entity_for_source_id(&scene_id("camera")).unwrap();
        let before = world.get(camera).unwrap().components[CameraBehaviorComponent::TYPE_NAME]
            ["shake"]["trauma"].as_f64().unwrap();
        assert!((before - 1.0).abs() < f64::EPSILON);
        update_camera_behaviors(&mut world, 1.0 / 60.0);
        let after = world.get(camera).unwrap().components[CameraBehaviorComponent::TYPE_NAME]
            ["shake"]["trauma"].as_f64().unwrap();
        assert!(after < before);
    }
}
