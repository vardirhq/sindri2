//! The window, the device, and the loop between them.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use sindri_core::FixedStepConfig;
#[cfg(not(target_arch = "wasm32"))]
use sindri_desktop::{AppContext, DesktopApp, Flow};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{EngineHost, InputEvent, Key};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, SpriteBatchRenderer, TextRenderer, TextureRegistry,
    TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
use sindri_scene::SceneExtractor;
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{CameraView, TextureBindings};

use crate::assets::{bind_audio, bind_fonts, bind_textures, extractor, world};
use crate::error::GatherError;
use crate::session::{GatherAudio, Session, gather_audio_backend};

/// Native Gather keeps the standalone embedded-project path.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct GatherApp {
    engine: EngineHost<Session, GatherAudio>,
    scene: SceneExtractor,
    bindings: TextureBindings,
    textures: TextureRegistry,
    depth: DepthTarget,
    cubes: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
}

#[cfg(not(target_arch = "wasm32"))]
impl DesktopApp for GatherApp {
    type Error = GatherError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let scene = extractor()?;
        let (textures, bindings) = bind_textures(context.device(), context.queue())?;
        let mut audio = gather_audio_backend()?;
        bind_audio(&mut audio)?;

        let mut engine = EngineHost::new_with_audio(
            Session::new(scene.components().clone()),
            FixedStepConfig::default(),
            audio,
        )?;
        *engine.world_mut() = world()?;
        engine.start()?;

        let mut text = TextRenderer::new(context.device(), context.queue(), context.format());
        bind_fonts(&mut text)?;
        Ok(Self {
            engine,
            scene,
            bindings,
            textures,
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cubes: TexturedCubeRenderer::new(context.device(), context.format()),
            sprites: SpriteBatchRenderer::new(context.device(), context.format()),
            text,
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
        // The screen UI is laid out against this, so a window that changes
        // shape moves the HUD with it rather than a frame later.
        self.engine.set_viewport(context.width(), context.height());
        Ok(())
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        let prepared = self.scene.extract_animated(
            self.engine.world(),
            Viewport::new(context.width(), context.height()),
            CameraView::default(),
            &self.bindings,
            self.engine.game().animations(),
        )?;
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri gather encoder"),
                });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut self.cubes,
                sprites: &mut self.sprites,
                text: &mut self.text,
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
