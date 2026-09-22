use std::time::Duration;

use sindri_core::{ComponentSchemaRegistry, FixedStepConfig, SceneComponent, SceneDocument, World};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{EngineHost, FrameContext, Game, InputEvent, Key};
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
const DEMO_SCRIPT: &str = include_str!("../assets/camera-demo.decay");

struct CameraGame {
    scripts: Scripts,
    sources: ScriptSources,
    components: ComponentSchemaRegistry,
}

impl CameraGame {
    fn new(components: ComponentSchemaRegistry) -> Self {
        let mut sources = ScriptSources::new();
        sources.insert("camera-demo.decay", DEMO_SCRIPT);
        Self {
            scripts: Scripts::new(),
            sources,
            components,
        }
    }
}

impl Game for CameraGame {
    type Error = DemoError;

    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let dt = context.time.delta.as_secs_f32();
        let report = self.scripts.advance(
            context.world,
            &self.components,
            ScriptFrame::new(&self.sources, context.input, dt),
        );
        if !report.failures.is_empty() {
            return Err(DemoError::Script(format!("{:?}", report.failures)));
        }
        update_camera_behaviors(context.world, dt);
        Ok(())
    }
}

fn demo_extractor() -> Result<SceneExtractor, DemoError> {
    let mut extractor = SceneExtractor::new()?;
    extractor.register::<ScriptComponent>("Script")?;
    Ok(extractor)
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
        let extractor = demo_extractor()?;
        let components = extractor.components().clone();
        let document = SceneDocument::from_json(SCENE_JSON)?;
        extractor.validate(&document, sindri_core::UnknownComponentPolicy::Reject)?;
        let mut engine = EngineHost::new(CameraGame::new(components), FixedStepConfig::default())
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
    #[error("camera demo Decay failed: {0}")]
    Script(String),
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
        let extractor = demo_extractor().unwrap();
        let mut engine = EngineHost::new(
            CameraGame::new(extractor.components().clone()),
            FixedStepConfig::default(),
        )
        .unwrap();
        *engine.world_mut() = World::from_scene(&document).unwrap().world;
        engine.start().unwrap();
        engine.queue_input(InputEvent::KeyPressed(Key::ArrowRight));
        for _ in 0..60 {
            engine.advance(Duration::from_secs_f32(1.0 / 60.0)).unwrap();
        }
        let camera = engine
            .world()
            .entity_for_source_id(&sindri_core::SceneEntityId::new("camera").unwrap())
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
    fn space_adds_trauma_through_decay_and_the_behavior_consumes_it() {
        let document = SceneDocument::from_json(SCENE_JSON).unwrap();
        let extractor = demo_extractor().unwrap();
        let mut engine = EngineHost::new(
            CameraGame::new(extractor.components().clone()),
            FixedStepConfig::default(),
        )
        .unwrap();
        *engine.world_mut() = World::from_scene(&document).unwrap().world;
        engine.start().unwrap();
        engine.queue_input(InputEvent::KeyPressed(Key::Space));
        engine.advance(Duration::from_secs_f32(1.0 / 60.0)).unwrap();
        let camera = engine
            .world()
            .entity_for_source_id(&sindri_core::SceneEntityId::new("camera").unwrap())
            .unwrap();
        let trauma = engine.world().get(camera).unwrap().components
            [CameraBehaviorComponent::TYPE_NAME]["shake"]["trauma"]
            .as_f64()
            .unwrap();
        assert!(
            trauma > 0.0 && trauma < 1.0,
            "Decay should add trauma before camera decay, got {trauma}"
        );
    }
}
