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

use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::{AssetId, ComponentSchemaRegistry, FixedStepConfig, SceneDocument, World};
use sindri_decay::{ScriptComponent, ScriptSources, Scripts};
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{EngineHost, FrameContext, Game, HostError, InputEvent, InputState, Key};
use sindri_render::{
    DepthTarget, FrameRenderers, FrameTarget, SpriteBatchRenderer, Texture2D, TextureError,
    TextureRegistry, TexturedCubeRenderer, Viewport, encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneExtractor, SpriteAnimations, TextureBindings};
use thiserror::Error;

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
        "textures/tile.png",
        include_bytes!("../assets/textures/tile.png"),
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
        }
    }

    /// One fixed step: the scripts run, then the animations move.
    ///
    /// Animations last because they read the world the scripts just wrote —
    /// a script that switches clip should be showing the new one this frame,
    /// not next.
    pub fn step(
        &mut self,
        world: &mut World,
        input: &InputState,
        delta_seconds: f32,
    ) -> Result<(), GatherError> {
        let report =
            self.scripts
                .advance(world, &self.components, &self.sources, input, delta_seconds);
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
        self.step(
            context.world,
            context.input,
            context.time.delta.as_secs_f32(),
        )
    }
}

/// The game as the windowed host runs it.
struct GatherApp {
    engine: EngineHost<Session>,
    scene: SceneExtractor,
    bindings: TextureBindings,
    textures: TextureRegistry,
    depth: DepthTarget,
    cubes: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
}

impl DesktopApp for GatherApp {
    type Error = GatherError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let scene = extractor()?;
        let mut textures = TextureRegistry::new(context.device(), context.queue());
        let mut bindings = TextureBindings::new();
        for (id, bytes) in TEXTURES {
            let asset = TextureAssetDecoder.decode(AssetBytes::new(
                (*id).parse::<AssetId>()?,
                (*bytes).to_vec(),
            ))?;
            let texture = Texture2D::from_rgba8(
                context.device(),
                context.queue(),
                id,
                asset.width(),
                asset.height(),
                asset.rgba8(),
            )?;
            bindings.bind(*id, textures.insert(texture));
        }

        let mut engine = EngineHost::new(
            Session::new(scene.components().clone()),
            FixedStepConfig::default(),
        )?;
        *engine.world_mut() = world()?;
        engine.start()?;

        Ok(Self {
            engine,
            scene,
            bindings,
            textures,
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cubes: TexturedCubeRenderer::new(context.device(), context.format()),
            sprites: SpriteBatchRenderer::new(context.device(), context.format()),
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
    Asset(#[from] sindri_core::AssetIdError),
    #[error(transparent)]
    Decode(#[from] sindri_assets::AssetDecodeError),
    #[error(transparent)]
    Texture(#[from] TextureError),
    #[error(transparent)]
    Animation(#[from] sindri_scene::AnimationError),
    #[error(transparent)]
    Json(#[from] sindri_core::SceneJsonError),
    #[error(transparent)]
    Batch(#[from] sindri_render::SpriteBatchError),
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
