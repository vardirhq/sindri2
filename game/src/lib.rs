//! Gather: the companion game.
//!
//! Five orbs on a floor, a thing you drive with the arrow keys, and a row of
//! lamps that fills as you collect them. That is the whole game, and it is the
//! first thing built with this engine that someone can lose interest in for the
//! right reasons rather than the wrong ones.
//!
//! **There are no game rules in this file.** Moving, gathering, counting, and
//! winning are Decay scripts in `assets/scripts/`; this is a window, a device,
//! and a loop. That split is the claim the game exists to test: if authoring
//! gameplay meant writing Rust here, the scripting layer would not be doing its
//! job.
//!
//! Native builds embed the project so the standalone binary has no working
//! directory requirement. Browser builds deliberately do not: `browser` loads
//! the same logical IDs through `FetchAssetSource` + `AssetLoader`, which proves
//! the static-hosting path rather than proving only that `include_bytes!` works
//! in WebAssembly.

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use sindri_assets::{
    AssetBytes, AssetDecoder, AudioAssetDecoder, FontAssetDecoder, TextureAssetDecoder,
};
use sindri_core::{ComponentSchemaRegistry, World};
#[cfg(not(target_arch = "wasm32"))]
use sindri_core::{AssetId, FixedStepConfig, SceneDocument, SpriteSheetDocument, sheet_id_for};
use sindri_decay::{AudioCommand, ScriptComponent, ScriptSources, Scripts};
use sindri_desktop::WindowConfig;
#[cfg(not(target_arch = "wasm32"))]
use sindri_desktop::{AppContext, DesktopApp, Flow};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::NativeAudioBackend;
use sindri_platform::{
    AudioBackend, AudioError, FrameContext, Game, HostError, InputState, PlaybackSettings,
};
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{AudioClip, EngineHost, InputEvent, Key};
use sindri_render::{FrameEncodeError, TextureError};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, SpriteBatchRenderer, TextRenderer, Texture2D,
    TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
use sindri_scene::{AudioSourceComponent, SceneExtractor, SheetBindError, SpriteAnimations};
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{CameraView, TextureBindings};
use thiserror::Error;

#[cfg(not(target_arch = "wasm32"))]
type GatherAudio = NativeAudioBackend;

#[cfg(target_arch = "wasm32")]
pub(crate) const SCENE_ID: &str = "gather.scene.json";
#[cfg(target_arch = "wasm32")]
pub(crate) const SCRIPT_IDS: &[&str] = &[
    "scripts/player.decay",
    "scripts/wisp.decay",
    "scripts/orb.decay",
    "scripts/pip.decay",
    "scripts/banner.decay",
];
#[cfg(target_arch = "wasm32")]
pub(crate) const TEXTURE_IDS: &[&str] = &[
    "textures/tiles.png",
    "textures/orb.png",
    "textures/player.png",
    "textures/pip.png",
    "textures/banner.png",
];
#[cfg(target_arch = "wasm32")]
pub(crate) const FONT_IDS: &[&str] = &["fonts/Inter.ttf"];
#[cfg(target_arch = "wasm32")]
pub(crate) const AUDIO_IDS: &[&str] = &[
    "audio/background.wav",
    "audio/pickup.wav",
    "audio/victory.wav",
];
#[cfg(target_arch = "wasm32")]
pub(crate) const SHEET_IDS: &[&str] = &[
    "textures/tiles.sheet.json",
    "textures/pip.sheet.json",
    "textures/player.sheet.json",
];

/// The scene and scripts are embedded only in native builds.
#[cfg(not(target_arch = "wasm32"))]
const SCENE: &str = include_str!("../assets/gather.scene.json");
#[cfg(not(target_arch = "wasm32"))]
const SCRIPTS: &[(&str, &str)] = &[
    (
        "scripts/player.decay",
        include_str!("../assets/scripts/player.decay"),
    ),
    (
        "scripts/wisp.decay",
        include_str!("../assets/scripts/wisp.decay"),
    ),
    (
        "scripts/orb.decay",
        include_str!("../assets/scripts/orb.decay"),
    ),
    (
        "scripts/pip.decay",
        include_str!("../assets/scripts/pip.decay"),
    ),
    (
        "scripts/banner.decay",
        include_str!("../assets/scripts/banner.decay"),
    ),
];

/// Native art bytes used by the standalone game and capture tests.
#[cfg(not(target_arch = "wasm32"))]
pub const TEXTURES: &[(&str, &[u8])] = &[
    (
        "textures/tiles.png",
        include_bytes!("../assets/textures/tiles.png"),
    ),
    (
        "textures/orb.png",
        include_bytes!("../assets/textures/orb.png"),
    ),
    (
        "textures/player.png",
        include_bytes!("../assets/textures/player.png"),
    ),
    (
        "textures/pip.png",
        include_bytes!("../assets/textures/pip.png"),
    ),
    (
        "textures/banner.png",
        include_bytes!("../assets/textures/banner.png"),
    ),
];

/// Native project-owned typefaces.
#[cfg(not(target_arch = "wasm32"))]
pub const FONTS: &[(&str, &[u8])] = &[(
    "fonts/Inter.ttf",
    include_bytes!("../assets/fonts/Inter.ttf"),
)];

/// Native sounds. Browser builds fetch the same IDs instead.
#[cfg(not(target_arch = "wasm32"))]
pub const AUDIO: &[(&str, &[u8])] = &[
    (
        "audio/background.wav",
        include_bytes!("../assets/audio/background.wav"),
    ),
    (
        "audio/pickup.wav",
        include_bytes!("../assets/audio/pickup.wav"),
    ),
    (
        "audio/victory.wav",
        include_bytes!("../assets/audio/victory.wav"),
    ),
];

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_fonts(renderer: &mut TextRenderer) -> Result<(), GatherError> {
    for (id, bytes) in FONTS {
        let asset = FontAssetDecoder.decode(AssetBytes::new(
            (*id).parse::<AssetId>()?,
            (*bytes).to_vec(),
        ))?;
        renderer.bind_font(*id, asset.family(), asset.bytes().to_vec());
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_audio(audio: &mut dyn AudioBackend) -> Result<(), GatherError> {
    for (id, bytes) in AUDIO {
        let asset = AudioAssetDecoder.decode(AssetBytes::new(
            (*id).parse::<AssetId>()?,
            (*bytes).to_vec(),
        ))?;
        let mime = asset.format().mime_type();
        audio.register(AudioClip::new(*id, asset.into_bytes(), mime))?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn gather_audio_backend() -> Result<GatherAudio, GatherError> {
    Ok(NativeAudioBackend::new()?)
}

/// How each sliced native texture is cut, shipped beside it.
#[cfg(not(target_arch = "wasm32"))]
pub const SHEETS: &[(&str, &str)] = &[
    (
        "textures/tiles.sheet.json",
        include_str!("../assets/textures/tiles.sheet.json"),
    ),
    (
        "textures/pip.sheet.json",
        include_str!("../assets/textures/pip.sheet.json"),
    ),
    (
        "textures/player.sheet.json",
        include_str!("../assets/textures/player.sheet.json"),
    ),
];

/// Every native texture on the GPU, and every sheet bound to what it cuts.
#[cfg(not(target_arch = "wasm32"))]
pub fn bind_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(TextureRegistry, TextureBindings), GatherError> {
    let mut textures = TextureRegistry::new(device, queue);
    let mut bindings = TextureBindings::new();
    for (id, bytes) in TEXTURES {
        let asset = TextureAssetDecoder.decode(AssetBytes::new(
            (*id).parse::<AssetId>()?,
            (*bytes).to_vec(),
        ))?;
        let texture = Texture2D::from_rgba8(
            device,
            queue,
            id,
            asset.width(),
            asset.height(),
            asset.rgba8(),
        )?;
        bindings.bind(*id, textures.insert(texture));

        let Some(sheet) = (*id)
            .parse::<AssetId>()
            .ok()
            .and_then(|id| sheet_id_for(&id))
        else {
            continue;
        };
        let Some((_, json)) = SHEETS.iter().find(|(name, _)| *name == sheet.as_str()) else {
            continue;
        };
        bindings.bind_sheet(*id, &SpriteSheetDocument::from_json(json)?)?;
    }
    Ok((textures, bindings))
}

/// The scene's component schemas, including the one the engine does not know.
pub fn extractor() -> Result<SceneExtractor, GatherError> {
    let mut extractor = SceneExtractor::new()?;
    extractor.register::<ScriptComponent>("Script")?;
    Ok(extractor)
}

/// The embedded native scene as a world, ready to run.
#[cfg(not(target_arch = "wasm32"))]
pub fn world() -> Result<World, GatherError> {
    let document = SceneDocument::from_json(SCENE)?;
    Ok(World::from_scene(&document)?.world)
}

/// The embedded native scripts, keyed by the IDs the scene names.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn sources() -> ScriptSources {
    let mut sources = ScriptSources::new();
    for (id, text) in SCRIPTS {
        sources.insert(*id, *text);
    }
    sources
}

/// The gameplay, which is the scripts and nothing else.
pub struct Session {
    scripts: Scripts,
    sources: ScriptSources,
    components: ComponentSchemaRegistry,
    animations: SpriteAnimations,
    pending_audio: Vec<AudioCommand>,
    autoplay_started: bool,
}

impl Session {
    /// A native session backed by the embedded project scripts.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn new(components: ComponentSchemaRegistry) -> Self {
        Self::with_sources(components, sources())
    }

    /// A session backed by sources supplied by the host.
    ///
    /// Browser delivery uses this after the script assets arrive through the
    /// real fetch pipeline; native `new` supplies the embedded equivalent.
    #[must_use]
    pub fn with_sources(components: ComponentSchemaRegistry, sources: ScriptSources) -> Self {
        Self {
            scripts: Scripts::new(),
            sources,
            components,
            animations: SpriteAnimations::new(),
            pending_audio: Vec::new(),
            autoplay_started: false,
        }
    }

    /// One fixed step: the scripts run, then the animations move.
    pub fn step(
        &mut self,
        world: &mut World,
        input: &InputState,
        delta_seconds: f32,
    ) -> Result<(), GatherError> {
        let report =
            self.scripts
                .advance(world, &self.components, &self.sources, input, delta_seconds);
        self.pending_audio
            .extend(self.scripts.take_audio_commands());
        for failure in &report.failures {
            log::error!("{failure}");
        }
        for message in &report.printed {
            log::info!("{}", message.message);
        }
        self.animations
            .advance(world, &self.components, delta_seconds)?;
        Ok(())
    }

    fn start_autoplay(
        &mut self,
        world: &World,
        audio: &mut dyn AudioBackend,
    ) -> Result<(), GatherError> {
        if self.autoplay_started {
            return Ok(());
        }
        for (_, source) in self.components.query::<AudioSourceComponent>(world)? {
            if !source.autoplay {
                continue;
            }
            let settings = if source.looping {
                PlaybackSettings::looping(source.normalized_volume())
            } else {
                PlaybackSettings::once(source.normalized_volume())
            };
            match audio.play(&source.clip, settings) {
                Ok(_) => {}
                Err(AudioError::Locked) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        self.autoplay_started = true;
        Ok(())
    }

    fn flush_audio(&mut self, audio: &mut dyn AudioBackend) -> Result<(), GatherError> {
        fn survivable(error: &AudioError) -> bool {
            matches!(error, AudioError::MissingClip(_) | AudioError::Locked)
        }

        for command in std::mem::take(&mut self.pending_audio) {
            match command {
                AudioCommand::Play { clip, volume } => {
                    match audio.play(&clip, PlaybackSettings::once(volume)) {
                        Ok(_) => {}
                        Err(error) if survivable(&error) => log::warn!("{error}"),
                        Err(error) => return Err(error.into()),
                    }
                }
                AudioCommand::Loop { clip, volume } => {
                    match audio.play(&clip, PlaybackSettings::looping(volume)) {
                        Ok(_) => {}
                        Err(error) if survivable(&error) => log::warn!("{error}"),
                        Err(error) => return Err(error.into()),
                    }
                }
                AudioCommand::StopAll => audio.stop_all(),
                AudioCommand::PauseAll => audio.pause_all(),
                AudioCommand::ResumeAll => audio.resume_all(),
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn animations(&self) -> &SpriteAnimations {
        &self.animations
    }
}

impl Game for Session {
    type Error = GatherError;

    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.start_autoplay(context.world, context.audio)?;
        self.step(
            context.world,
            context.input,
            context.time.delta.as_secs_f32(),
        )?;
        self.flush_audio(context.audio)
    }
}

/// Native Gather keeps the standalone embedded-project path.
#[cfg(not(target_arch = "wasm32"))]
struct GatherApp {
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

#[derive(Debug, Error)]
pub enum GatherError {
    #[error(transparent)]
    Scene(#[from] sindri_scene::SceneExtractError),
    #[error(transparent)]
    Document(#[from] sindri_core::SceneError),
    #[error(transparent)]
    World(#[from] sindri_core::WorldError),
    #[error(transparent)]
    Component(#[from] sindri_core::ComponentRegistryError),
    #[error(transparent)]
    Asset(#[from] sindri_core::AssetIdError),
    #[error(transparent)]
    Sheet(#[from] sindri_core::SheetError),
    #[error(transparent)]
    SheetBind(#[from] SheetBindError),
    #[error(transparent)]
    Decode(#[from] sindri_assets::AssetDecodeError),
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error(transparent)]
    Texture(#[from] TextureError),
    #[error(transparent)]
    Animation(#[from] sindri_scene::AnimationError),
    #[error(transparent)]
    Json(#[from] sindri_core::SceneJsonError),
    #[error(transparent)]
    Frame(#[from] FrameEncodeError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    AssetQueue(#[from] sindri_assets::AssetLoadQueueCreateError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    AssetLoader(#[from] sindri_assets::AssetLoaderError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    AssetLoad(#[from] sindri_core::AssetLoadError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    Manifest(#[from] sindri_assets::ManifestError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    UrlRoot(#[from] sindri_assets::UrlRootError),
    #[cfg(target_arch = "wasm32")]
    #[error("browser project asset error: {0}")]
    BrowserAsset(String),
    #[error(transparent)]
    Host(#[from] Box<HostError<GatherError>>),
}

impl From<HostError<GatherError>> for GatherError {
    fn from(error: HostError<GatherError>) -> Self {
        Self::Host(Box::new(error))
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn run() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        if let Err(error) = sindri_desktop::run::<GatherApp>(WindowConfig {
            title: "Gather".to_owned(),
            ..WindowConfig::default()
        }) {
            log::error!("{error}");
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Info);
        if let Err(error) = sindri_desktop::run::<browser::BrowserGatherApp>(WindowConfig {
            title: "Gather".to_owned(),
            ..WindowConfig::default()
        }) {
            log::error!("{error}");
        }
    }
}
