//! Gather's browser host.
//!
//! The old browser build proved only that `include_bytes!` survives WASM. This
//! one does not own a scene, script, texture, font, sheet, or sound until the
//! browser fetch source has returned it through `AssetLoader` and the manifest
//! has accepted the bytes. Native stays on the embedded path so changing web
//! delivery cannot quietly destabilise the desktop game.

mod loader;

use std::time::Duration;

use sindri_core::{AssetId, EngineState, World, sheet_id_for};
use sindri_desktop::{AppContext, DesktopApp, Flow};
use sindri_platform::{AudioBackend, AudioClip, BrowserAudioBackend, EngineHost, InputEvent, Key};
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, GlyphRenderer, SpriteBatchRenderer, TextRenderer,
    Texture2D, TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TextureBindings};

use self::loader::{BrowserProjectAssets, BrowserProjectLoader};
use crate::assets::{TEXTURE_IDS, extractor};
use crate::error::GatherError;
use crate::session::Session;

/// Gather's browser host deliberately starts empty.
///
/// The old browser build proved only that `include_bytes!` survives wasm. This
/// one does not own a scene, script, texture, font, sheet, or sound until the
/// browser fetch source has returned it through `AssetLoader` and the manifest
/// has accepted the bytes. Native stays on the embedded path so changing web
/// delivery cannot quietly destabilise the desktop game.
pub(super) struct BrowserGatherApp {
    loader: Option<BrowserProjectLoader>,
    pending: Option<BrowserProjectAssets>,
    audio: Option<BrowserAudioBackend>,
    engine: Option<EngineHost<Session, BrowserAudioBackend>>,
    scene: SceneExtractor,
    bindings: TextureBindings,
    textures: TextureRegistry,
    depth: DepthTarget,
    cubes: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
    glyphs: GlyphRenderer,
    /// The drawing surface's size, kept because the engine is built later.
    viewport: [u32; 2],
    page_visible: bool,
    platform_suspended: bool,
    paused_for_page: bool,
}

impl BrowserGatherApp {
    fn install(
        &mut self,
        context: &AppContext<'_>,
        project: BrowserProjectAssets,
    ) -> Result<(), GatherError> {
        for (id, asset) in project.textures {
            let texture = Texture2D::from_rgba8(
                context.device(),
                context.queue(),
                id.as_str(),
                asset.width(),
                asset.height(),
                asset.rgba8(),
            )?;
            self.bindings
                .bind(id.as_str(), self.textures.insert(texture));
        }

        for texture_id in TEXTURE_IDS {
            let texture_id = AssetId::new(*texture_id)?;
            let Some(sheet_id) = sheet_id_for(&texture_id) else {
                continue;
            };
            if let Some(sheet) = project.sheets.get(sheet_id.as_str()) {
                self.bindings.bind_sheet(texture_id.as_str(), sheet)?;
            }
        }

        for (id, asset) in project.fonts {
            self.text
                .bind_font(id.as_str(), asset.family(), asset.bytes().to_vec());
        }

        let mut audio = self.audio.take().ok_or_else(|| {
            GatherError::BrowserAsset("browser audio backend was already moved".into())
        })?;
        for (id, asset) in project.audio {
            audio.register(AudioClip::new(
                id.as_str(),
                asset.bytes().to_vec(),
                asset.format().mime_type(),
            ))?;
        }

        let mut session = Session::with_sources(self.scene.components().clone(), project.scripts)
            .with_prefabs(project.prefabs);
        // A named key, because two games sharing an origin must not share a
        // save.
        session.keep_saves_in(Box::new(sindri_platform::BrowserSaves::under(
            "sindri.gather.save",
        )));
        let mut engine =
            EngineHost::new_with_audio(session, sindri_core::FixedStepConfig::default(), audio)?;
        *engine.world_mut() = World::from_scene(&project.scene)?.world;
        engine.start()?;
        engine.set_viewport(self.viewport[0], self.viewport[1]);
        self.engine = Some(engine);
        self.sync_page_lifecycle()?;
        log::info!(
            "Gather loaded {} project assets through browser fetch",
            project.asset_count
        );
        Ok(())
    }

    fn sync_page_lifecycle(&mut self) -> Result<(), GatherError> {
        let should_pause = !self.page_visible || self.platform_suspended;
        let Some(engine) = &mut self.engine else {
            return Ok(());
        };

        if should_pause && !self.paused_for_page && engine.state() == EngineState::Running {
            engine.pause()?;
            self.paused_for_page = true;
        } else if !should_pause && self.paused_for_page && engine.state() == EngineState::Paused {
            engine.resume()?;
            self.paused_for_page = false;
        }
        Ok(())
    }

    fn clear_loading(&self, context: &AppContext<'_>, view: &wgpu::TextureView) {
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri Gather loading encoder"),
                });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sindri Gather loading pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.012,
                            g: 0.018,
                            b: 0.03,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        context.queue().submit([encoder.finish()]);
    }
}

impl DesktopApp for BrowserGatherApp {
    type Error = GatherError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            loader: Some(BrowserProjectLoader::new()?),
            pending: None,
            audio: Some(BrowserAudioBackend::new()),
            engine: None,
            scene: extractor()?,
            bindings: TextureBindings::new(),
            textures: TextureRegistry::new(context.device(), context.queue()),
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cubes: TexturedCubeRenderer::new(context.device(), context.format()),
            sprites: SpriteBatchRenderer::new(context.device(), context.format()),
            text: TextRenderer::new(),
            glyphs: GlyphRenderer::new(context.device(), context.format()),
            // Remembered because the engine does not exist yet: the project
            // loads asynchronously, and a resize that arrives before it must
            // not be the one size nobody ever hears about.
            viewport: [context.width(), context.height()],
            page_visible: true,
            platform_suspended: false,
            paused_for_page: false,
        })
    }

    fn input(&mut self, event: InputEvent) {
        if let Some(engine) = &mut self.engine {
            engine.queue_input(event);
            return;
        }
        // A finger counts as the gesture that unlocks audio. Browsers require
        // one before a sound may play, and a phone is the machine most likely
        // to be asking — a list that named only keys and mouse buttons would
        // leave a touch-only device silent for the whole run.
        if matches!(
            event,
            InputEvent::KeyPressed(_)
                | InputEvent::ButtonPressed(_)
                | InputEvent::TouchStarted { .. }
        ) && let Some(audio) = &mut self.audio
        {
            let _ = audio.unlock();
        }
    }

    fn update(&mut self, delta: Duration) -> Result<Flow, Self::Error> {
        if self.engine.is_none() {
            if self.pending.is_none()
                && let Some(loader) = &mut self.loader
                && let Some(project) = loader.poll()?
            {
                self.pending = Some(project);
                self.loader = None;
            }
            return Ok(Flow::Continue);
        }

        let engine = self.engine.as_mut().expect("checked above");
        if engine.input().key_down(Key::Escape) {
            return Ok(Flow::Exit);
        }
        engine.advance(delta)?;
        Ok(Flow::Continue)
    }

    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> {
        self.depth
            .resize(context.device(), context.width(), context.height());
        // A browser window changes shape constantly — a phone rotating, a tab
        // resizing — and the screen UI is laid out against this.
        self.viewport = [context.width(), context.height()];
        if let Some(engine) = self.engine.as_mut() {
            engine.set_viewport(context.width(), context.height());
        }
        Ok(())
    }

    fn suspend(&mut self) -> Result<(), Self::Error> {
        self.platform_suspended = true;
        self.sync_page_lifecycle()
    }

    fn resume(&mut self) -> Result<(), Self::Error> {
        self.platform_suspended = false;
        self.sync_page_lifecycle()
    }

    fn visibility_changed(&mut self, visible: bool) -> Result<(), Self::Error> {
        self.page_visible = visible;
        self.sync_page_lifecycle()
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        if self.engine.is_none()
            && let Some(project) = self.pending.take()
        {
            self.install(context, project)?;
        }

        let Some(engine) = &self.engine else {
            self.clear_loading(context, view);
            return Ok(());
        };

        let prepared = self.scene.extract_animated(
            engine.world(),
            Viewport::new(context.width(), context.height()),
            CameraView::default(),
            &self.bindings,
            SceneRuntime::default()
                .with_animations(engine.game().animations())
                .with_effects(engine.game().effects()),
        )?;
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri gather browser encoder"),
                });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut self.cubes,
                sprites: &mut self.sprites,
                text: &mut self.text,
                glyphs: &mut self.glyphs,
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
