//! One run of the game: the world, its scripts, and a frame of both.
//!
//! **There are no game rules here.** Moving, gathering, counting, and
//! winning are Decay scripts in `assets/scripts/`; this advances them
//! and hands what they did to the engine.

use sindri_core::{ComponentSchemaRegistry, World};
use sindri_decay::{AudioCommand, PrefabSources, ScriptSources, Scripts};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::NativeAudioBackend;
use sindri_platform::{AudioBackend, AudioError, FrameContext, Game, InputState, PlaybackSettings};
use sindri_scene::{AudioSourceComponent, SpriteAnimations};

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
        let report = self.scripts.advance(
            world,
            &self.components,
            &self.sources,
            &self.prefabs,
            input,
            delta_seconds,
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
