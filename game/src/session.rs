//! One run of the game: the world, its scripts, and a frame of both.
//!
//! **There are no game rules here.** Moving, gathering, counting, and
//! winning are Decay scripts in `assets/scripts/`; this advances them
//! and hands what they did to the engine.

use sindri_core::{ComponentSchemaRegistry, World};
use sindri_decay::{AudioCommand, PrefabSources, ScriptFrame, ScriptSources, Scripts};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::NativeAudioBackend;
use sindri_platform::{AudioBackend, AudioError, FrameContext, Game, InputState, PlaybackSettings};
use sindri_scene::{
    AudioSourceComponent, PointerFrame, ScenePhysics2d, ScreenExtent, ScreenUi, SpriteAnimations,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::assets::sources;
use crate::error::GatherError;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type GatherAudio = NativeAudioBackend;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn gather_audio_backend() -> Result<GatherAudio, GatherError> {
    Ok(NativeAudioBackend::new()?)
}

/// The gameplay, which is the scripts and nothing else.
pub struct Session {
    scripts: Scripts,
    sources: ScriptSources,
    /// The prefabs the scripts can spawn.
    ///
    /// Empty: Gather shows what the engine already does rather than reaching
    /// for what it has just grown, and nothing it runs spawns. A host that does
    /// fills this the same way it fills the sources.
    prefabs: PrefabSources,
    components: ComponentSchemaRegistry,
    animations: SpriteAnimations,
    /// The physics the scripts may drive.
    ///
    /// Stepped every fixed update whether or not the scene authors a collider,
    /// which costs nothing for a scene with none and means a scene that grows
    /// one needs no change here. No gravity: Gather is seen from above.
    physics: ScenePhysics2d,
    /// Where the screen elements are and what the pointer is doing to them.
    screen_ui: ScreenUi,
    /// The run's random stream.
    ///
    /// A fixed seed, because the engine has no entropy to offer and will not
    /// pretend otherwise. A game that wants a different run each time calls
    /// `Random.seed` with something it knows.
    random: sindri_core::Rng,
    /// What the game remembers, and how long since it was written out.
    ///
    /// Held in memory and written on a cadence rather than on every change: how
    /// often someone's storage is touched is a decision about their machine.
    saves: sindri_core::SaveStore,
    /// The live flecks a script has thrown.
    effects: sindri_scene::Effects2d,
    since_written: f32,
    /// Where the save actually goes.
    ///
    /// Memory unless a host says otherwise, so a headless run and a test have
    /// somewhere to write without choosing a path. The desktop host names a
    /// file; the browser host uses the page's own storage.
    save_backend: Box<dyn sindri_platform::SaveBackend>,
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
            prefabs: PrefabSources::new(),
            components,
            animations: SpriteAnimations::new(),
            physics: ScenePhysics2d::top_down().expect("zero gravity is finite"),
            screen_ui: ScreenUi::default(),
            random: sindri_core::Rng::default(),
            saves: sindri_core::SaveStore::default(),
            effects: sindri_scene::Effects2d::default(),
            since_written: 0.0,
            save_backend: Box::new(sindri_platform::MemorySaves::new()),
            pending_audio: Vec::new(),
            autoplay_started: false,
        }
    }

    /// One fixed step: the scripts run, then the animations move.
    pub fn step(
        &mut self,
        world: &mut World,
        input: &InputState,
        viewport: (f32, f32),
        delta_seconds: f32,
    ) -> Result<(), GatherError> {
        // Physics first, so a script observes the events of the step that just
        // happened and its writes take effect on the next one, which is the
        // order `docs/physics.md` fixes.
        self.physics.step(
            world,
            &self.components,
            std::time::Duration::from_secs_f32(delta_seconds),
        )?;
        // No safe area yet: reading a device's insets is the browser host's to
        // report, and it does not yet. The scene needs no change when it does.
        self.screen_ui.update(
            world,
            &self.components,
            ScreenExtent::new(viewport.0, viewport.1),
            PointerFrame {
                position: input.pointer_position(),
                pressed: input.pointer_pressed(sindri_platform::MouseButton::Left),
                released: input.pointer_released(sindri_platform::MouseButton::Left),
                down: input.pointer_down(sindri_platform::MouseButton::Left),
            },
        )?;
        // Before the scripts, so a fleck thrown this frame is drawn where it
        // was thrown rather than one frame along.
        self.effects
            .advance(std::time::Duration::from_secs_f32(delta_seconds));
        let (physics, events) = self.physics.for_scripts();
        let report = self.scripts.advance(
            world,
            &self.components,
            ScriptFrame::new(&self.sources, input, delta_seconds)
                .with_prefabs(&self.prefabs)
                .with_screen_ui(&self.screen_ui)
                .with_random(&mut self.random)
                .with_saves(&mut self.saves)
                .with_effects(&mut self.effects)
                .with_physics(sindri_decay::Physics2d {
                    world: physics,
                    events,
                }),
        );
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

    /// The live flecks, for whatever draws the frame.
    pub const fn effects(&self) -> &sindri_scene::Effects2d {
        &self.effects
    }
}

impl Session {
    /// Keeps this session's save somewhere the host chose, loading what is
    /// already there.
    ///
    /// Called before the first frame: a game that read its progress after
    /// starting would have already begun a run without it.
    pub fn keep_saves_in(&mut self, mut backend: Box<dyn sindri_platform::SaveBackend>) {
        self.saves = sindri_core::SaveStore::opened(backend.read());
        self.save_backend = backend;
        self.since_written = 0.0;
    }

    /// Writes the save out if anything changed and enough time has passed.
    ///
    /// A failure is reported and does not stop the frame: a disk that will not
    /// take a save is worth knowing about, and it is not a reason to end
    /// someone's run.
    fn write_saves(&mut self, elapsed: f32, force: bool) {
        self.since_written += elapsed;
        if !self.saves.is_dirty() || (!force && self.since_written < SAVE_INTERVAL_SECONDS) {
            return;
        }
        self.since_written = 0.0;
        match self.save_backend.write(&self.saves.to_document()) {
            Ok(()) => self.saves.mark_written(),
            // Left dirty on purpose, so the next attempt tries again rather
            // than believing a write that did not happen.
            Err(error) => log::error!("the save could not be written: {error}"),
        }
    }
}

/// How long a change waits before being written out.
///
/// Long enough that a value changing every frame does not keep a disk busy,
/// short enough that a browser tab closing loses almost nothing.
const SAVE_INTERVAL_SECONDS: f32 = 2.0;

impl Game for Session {
    type Error = GatherError;

    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.start_autoplay(context.world, context.audio)?;
        #[allow(clippy::cast_precision_loss)]
        let viewport = (context.viewport[0] as f32, context.viewport[1] as f32);
        self.step(
            context.world,
            context.input,
            viewport,
            context.time.delta.as_secs_f32(),
        )?;
        self.write_saves(context.time.delta.as_secs_f32(), false);
        self.flush_audio(context.audio)
    }

    /// The last chance to keep what a run earned.
    fn stop(&mut self, _context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.write_saves(0.0, true);
        Ok(())
    }
}
