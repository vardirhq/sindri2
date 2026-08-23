use std::{collections::BTreeMap, time::Duration};

use sindri_assets::{
    AssetDecoder, AssetLoadOutcome, AssetLoadQueueConfig, AssetLoader, AssetManifest, AudioAsset,
    AudioAssetDecoder, FetchAssetSource, FontAsset, FontAssetDecoder, MANIFEST_FILE_NAME,
    SceneAssetDecoder, SpriteSheetAssetDecoder, TextAssetDecoder, TextureAsset,
    TextureAssetDecoder,
};
use sindri_core::{AssetId, SceneDocument, SpriteSheetDocument, World, sheet_id_for};
use sindri_decay::ScriptSources;
use sindri_desktop::{AppContext, DesktopApp, Flow};
use sindri_platform::{AudioBackend, AudioClip, BrowserAudioBackend, EngineHost, InputEvent, Key};
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, SpriteBatchRenderer, TextRenderer, Texture2D,
    TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings};

use crate::{
    AUDIO_IDS, FONT_IDS, GatherError, SCENE_ID, SCRIPT_IDS, SHEET_IDS, Session, TEXTURE_IDS,
    extractor,
};

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

        let mut engine = EngineHost::new_with_audio(
            Session::with_sources(self.scene.components().clone(), project.scripts),
            sindri_core::FixedStepConfig::default(),
            audio,
        )?;
        *engine.world_mut() = World::from_scene(&project.scene)?.world;
        engine.start()?;
        self.engine = Some(engine);
        log::info!(
            "Gather loaded {} project assets through browser fetch",
            project.asset_count
        );
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
            text: TextRenderer::new(context.device(), context.queue(), context.format()),
        })
    }

    fn input(&mut self, event: InputEvent) {
        if let Some(engine) = &mut self.engine {
            engine.queue_input(event);
            return;
        }
        if matches!(
            event,
            InputEvent::KeyPressed(_) | InputEvent::ButtonPressed(_)
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
        Ok(())
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
            engine.game().animations(),
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

struct BrowserProjectAssets {
    scene: SceneDocument,
    scripts: ScriptSources,
    textures: Vec<(AssetId, TextureAsset)>,
    fonts: Vec<(AssetId, FontAsset)>,
    audio: Vec<(AssetId, AudioAsset)>,
    sheets: BTreeMap<String, SpriteSheetDocument>,
    asset_count: usize,
}

struct BrowserProjectLoader {
    phase: Option<LoadPhase>,
}

enum LoadPhase {
    Manifest(AssetLoader<TextAssetDecoder>),
    Assets(Box<ProjectLoaders>),
}

impl BrowserProjectLoader {
    fn new() -> Result<Self, GatherError> {
        let source = FetchAssetSource::new("assets")?;
        let mut manifest =
            AssetLoader::new(source, AssetLoadQueueConfig::default(), TextAssetDecoder)?;
        manifest.request(AssetId::new(MANIFEST_FILE_NAME)?)?;
        Ok(Self {
            phase: Some(LoadPhase::Manifest(manifest)),
        })
    }

    fn poll(&mut self) -> Result<Option<BrowserProjectAssets>, GatherError> {
        let Some(phase) = self.phase.take() else {
            return Ok(None);
        };
        match phase {
            LoadPhase::Manifest(mut loader) => {
                poll_loader(&mut loader)?;
                let id = AssetId::new(MANIFEST_FILE_NAME)?;
                if let Some(text) = loader.get(&id) {
                    let manifest = AssetManifest::from_json(text)?;
                    self.phase = Some(LoadPhase::Assets(Box::new(ProjectLoaders::new(manifest)?)));
                } else {
                    self.phase = Some(LoadPhase::Manifest(loader));
                }
                Ok(None)
            }
            LoadPhase::Assets(mut loaders) => {
                if let Some(project) = loaders.poll()? {
                    Ok(Some(project))
                } else {
                    self.phase = Some(LoadPhase::Assets(loaders));
                    Ok(None)
                }
            }
        }
    }
}

struct ProjectLoaders {
    scene: AssetLoader<SceneAssetDecoder>,
    scripts: AssetLoader<TextAssetDecoder>,
    textures: AssetLoader<TextureAssetDecoder>,
    fonts: AssetLoader<FontAssetDecoder>,
    audio: AssetLoader<AudioAssetDecoder>,
    sheets: AssetLoader<SpriteSheetAssetDecoder>,
}

impl ProjectLoaders {
    fn new(manifest: AssetManifest) -> Result<Self, GatherError> {
        let source = FetchAssetSource::new("assets")?;
        let config = AssetLoadQueueConfig::default();
        let mut scene = AssetLoader::new(source.clone(), config, SceneAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut scripts = AssetLoader::new(source.clone(), config, TextAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut textures = AssetLoader::new(source.clone(), config, TextureAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut fonts = AssetLoader::new(source.clone(), config, FontAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut audio = AssetLoader::new(source.clone(), config, AudioAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut sheets =
            AssetLoader::new(source, config, SpriteSheetAssetDecoder)?.with_manifest(manifest);

        request(&mut scene, &[SCENE_ID])?;
        request(&mut scripts, SCRIPT_IDS)?;
        request(&mut textures, TEXTURE_IDS)?;
        request(&mut fonts, FONT_IDS)?;
        request(&mut audio, AUDIO_IDS)?;
        request(&mut sheets, SHEET_IDS)?;

        Ok(Self {
            scene,
            scripts,
            textures,
            fonts,
            audio,
            sheets,
        })
    }

    fn poll(&mut self) -> Result<Option<BrowserProjectAssets>, GatherError> {
        poll_loader(&mut self.scene)?;
        poll_loader(&mut self.scripts)?;
        poll_loader(&mut self.textures)?;
        poll_loader(&mut self.fonts)?;
        poll_loader(&mut self.audio)?;
        poll_loader(&mut self.sheets)?;

        if self.scene.outstanding()
            + self.scripts.outstanding()
            + self.textures.outstanding()
            + self.fonts.outstanding()
            + self.audio.outstanding()
            + self.sheets.outstanding()
            != 0
        {
            return Ok(None);
        }

        let scene = loaded(&self.scene, SCENE_ID)?;
        let mut scripts = ScriptSources::new();
        for id in SCRIPT_IDS {
            scripts.insert(*id, loaded(&self.scripts, id)?);
        }
        let textures = loaded_many(&self.textures, TEXTURE_IDS)?;
        let fonts = loaded_many(&self.fonts, FONT_IDS)?;
        let audio = loaded_many(&self.audio, AUDIO_IDS)?;
        let sheets = loaded_many(&self.sheets, SHEET_IDS)?
            .into_iter()
            .map(|(id, sheet)| (id.as_str().to_owned(), sheet))
            .collect();
        let asset_count = 1
            + SCRIPT_IDS.len()
            + TEXTURE_IDS.len()
            + FONT_IDS.len()
            + AUDIO_IDS.len()
            + SHEET_IDS.len();
        Ok(Some(BrowserProjectAssets {
            scene,
            scripts,
            textures,
            fonts,
            audio,
            sheets,
            asset_count,
        }))
    }
}

fn request<D: AssetDecoder>(loader: &mut AssetLoader<D>, ids: &[&str]) -> Result<(), GatherError> {
    for id in ids {
        loader.request(AssetId::new(*id)?)?;
    }
    Ok(())
}

fn poll_loader<D: AssetDecoder>(loader: &mut AssetLoader<D>) -> Result<(), GatherError> {
    for outcome in loader.poll() {
        if let AssetLoadOutcome::Failed(error) = outcome {
            return Err(error.into());
        }
    }
    Ok(())
}

fn loaded<D>(loader: &AssetLoader<D>, id: &str) -> Result<D::Asset, GatherError>
where
    D: AssetDecoder,
    D::Asset: Clone,
{
    let id = AssetId::new(id)?;
    loader
        .get(&id)
        .cloned()
        .ok_or_else(|| GatherError::BrowserAsset(format!("'{id}' completed without a value")))
}

fn loaded_many<D>(
    loader: &AssetLoader<D>,
    ids: &[&str],
) -> Result<Vec<(AssetId, D::Asset)>, GatherError>
where
    D: AssetDecoder,
    D::Asset: Clone,
{
    ids.iter()
        .map(|id| {
            let asset_id = AssetId::new(*id)?;
            let asset = loader.get(&asset_id).cloned().ok_or_else(|| {
                GatherError::BrowserAsset(format!("'{asset_id}' completed without a value"))
            })?;
            Ok((asset_id, asset))
        })
        .collect()
}
