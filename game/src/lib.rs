//! Gather: the companion game.
//!
//! Five orbs on a floor, a thing you drive with the arrow keys, and a row of
//! lamps that fills as you collect them. That is the whole game, and it is the
//! first thing built with this engine that someone can lose interest in for the
//! right reasons rather than the wrong ones.
//!
//! **There are no game rules in this file.** Moving, gathering, counting, and
//! winning are four Decay scripts in `assets/scripts/`; this is a window, a
//! device, and a loop. That split is the claim the game exists to test: if
//! authoring gameplay meant writing Rust here, the scripting layer would not be
//! doing its job.
//!
//! Everything is embedded rather than loaded from disk. The game ships as one
//! binary that runs the same way on a desktop and in a browser, and a game that
//! needed a working directory would not.

use std::time::Duration;

use sindri_assets::{
    AssetBytes, AssetDecoder, AudioAssetDecoder, FontAssetDecoder, TextureAssetDecoder,
};
use sindri_core::{
    AssetId, ComponentSchemaRegistry, FixedStepConfig, SceneDocument, SpriteSheetDocument, World,
    sheet_id_for,
};
use sindri_decay::{AudioCommand, ScriptComponent, ScriptSources, Scripts};
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
#[cfg(target_arch = "wasm32")]
use sindri_platform::BrowserAudioBackend;
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::NativeAudioBackend;
use sindri_platform::{
    AudioBackend, AudioClip, AudioError, EngineHost, FrameContext, Game, HostError, InputEvent,
    InputState, Key, PlaybackSettings,
};
use sindri_render::{
    DepthTarget, FrameEncodeError, FrameRenderers, FrameTarget, SpriteBatchRenderer, TextRenderer,
    Texture2D, TextureError, TextureRegistry, TexturedCubeRenderer, Viewport,
    encode_prepared_frame,
};
use sindri_scene::{
    AudioSourceComponent, CameraView, SceneExtractor, SheetBindError, SpriteAnimations,
    TextureBindings,
};
use thiserror::Error;

#[cfg(target_arch = "wasm32")]
type GatherAudio = BrowserAudioBackend;
#[cfg(not(target_arch = "wasm32"))]
type GatherAudio = NativeAudioBackend;

/// The scene, and the four scripts that are the game.
const SCENE: &str = include_str!("../assets/gather.scene.json");
const SCRIPTS: &[(&str, &str)] = &[
    (
        "scripts/player.decay",
        include_str!("../assets/scripts/player.decay"),
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
/// The art, embedded so the game is one file that runs anywhere.
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

/// Project-owned typefaces, embedded for the same native/browser parity as art.
pub const FONTS: &[(&str, &[u8])] = &[(
    "fonts/Inter.ttf",
    include_bytes!("../assets/fonts/Inter.ttf"),
)];

/// The sounds proving the same asset -> platform path on desktop and web.
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

/// Decodes and binds the fonts the shipped scene names.
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

/// Registers embedded audio through the same decoder the asset pipeline exposes.
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

#[cfg(target_arch = "wasm32")]
fn gather_audio_backend() -> Result<GatherAudio, GatherError> {
    Ok(BrowserAudioBackend::new())
}

/// How each sliced texture is cut, shipped beside it.
///
/// A sheet is named after the texture it slices, so nothing here has to say
/// which goes with which — `sheet_id_for` derives it, the same way the editor
/// finds one on disk.
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

/// Every texture on the GPU, and every sheet bound to the texture it cuts.
///
/// One function rather than one per binary, because there are two — the window
/// and the capture — and they were built separately. Adding sheets to one and
/// not the other made the capture draw every sprite as its whole sheet, which
/// is the exact duplication this change removed from the scene format, living
/// one level up. Two callers of one function cannot disagree.
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

        // The sheet that slices it, found by the same derivation the editor
        // uses to look for one on disk.
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
///
/// `sindri.script` is registered by the host rather than by `sindri-scene`,
/// because the engine's scene crate must not learn about a language. The editor
/// does the same thing for the same reason.
pub fn extractor() -> Result<SceneExtractor, GatherError> {
    let mut extractor = SceneExtractor::new()?;
    extractor.register::<ScriptComponent>("Script")?;
    Ok(extractor)
}

/// The scene as a world, ready to run.
pub fn world() -> Result<World, GatherError> {
    let document = SceneDocument::from_json(SCENE)?;
    Ok(World::from_scene(&document)?.world)
}

/// The scripts, keyed by the ids the scene names them with.
#[must_use]
pub fn sources() -> ScriptSources {
    let mut sources = ScriptSources::new();
    for (id, text) in SCRIPTS {
        sources.insert(*id, *text);
    }
    sources
}

/// The gameplay, which is the scripts and nothing else.
///
/// Runs at the fixed step rather than per frame, so gathering an orb happens at
/// the same rate whatever the frame rate is — and so a stalled frame does not
/// teleport the player through the wall the script clamps them to.
///
/// Public because the window is not the only thing that plays the game: the
/// offscreen capture photographs a game part-way through a run, and it must be
/// the same run the window would have had rather than a second implementation
/// of one.
pub struct Session {
    scripts: Scripts,
    sources: ScriptSources,
    components: ComponentSchemaRegistry,
    animations: SpriteAnimations,
    pending_audio: Vec<AudioCommand>,
    autoplay_started: bool,
}

impl Session {
    /// A session that runs the shipped scripts against `components`, which is
    /// the schema set the scene was validated with.
    #[must_use]
    pub fn new(components: ComponentSchemaRegistry) -> Self {
        Self {
            scripts: Scripts::new(),
            sources: sources(),
            components,
            animations: SpriteAnimations::new(),
            pending_audio: Vec::new(),
            autoplay_started: false,
        }
    }

    /// One fixed step: the scripts run, then the animations move.
    ///
    /// Animations last because they read the world the scripts just wrote —
    /// a script that switches clip should be showing the new one this frame,
    /// not next. Audio intent is drained after the scripts so a headless caller
    /// can still run gameplay without owning a device.
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
        // A failing script says so once rather than being swallowed. Nothing
        // here stops the game: the others keep running, which is the same
        // arrangement the editor uses.
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
                // Browsers require a real key/pointer gesture. Keeping this
                // false makes the next fixed step retry after queue_input has
                // unlocked the backend, instead of silently losing music.
                Err(AudioError::Locked) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        self.autoplay_started = true;
        Ok(())
    }

    /// Performs what the scripts asked for, and keeps playing if a sound cannot.
    ///
    /// A clip nobody shipped, or a browser that has not seen a gesture yet, is
    /// reported rather than fatal — the same posture the renderer takes when a
    /// texture will not bind, where the frame still draws and the missing
    /// reference is named. A typo in a sound should not end the game.
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

    /// Where every animated sprite has got to, which is what extraction needs
    /// to draw the frame a clip is on rather than the whole sheet.
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

/// The game as the windowed host runs it.
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
        // Animated, because the session advanced those clips and a frame that
        // extracted without them would show every cell of each sheet at once.
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
    // Boxed because a host error carries the game's own error, and a type that
    // contains itself has no size.
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
    env_logger::init();
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Info);
    }
    if let Err(error) = sindri_desktop::run::<GatherApp>(WindowConfig {
        title: "Gather".to_owned(),
        ..WindowConfig::default()
    }) {
        log::error!("{error}");
    }
}
