//! One run of the game: the world, its scripts, and a frame of both.
//!
//! **There are no game rules here.** Moving, gathering, counting, and
//! winning are Decay scripts in `assets/scripts/`; this advances them
//! and hands what they did to the engine.

use std::collections::BTreeMap;

use sindri_core::{ComponentSchemaRegistry, World};
use sindri_decay::{
    AudioCommand, PrefabSources, ProfileSources, ScriptFrame, ScriptSources, Scripts,
};

#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::NativeAudioBackend;
use sindri_platform::{AudioBackend, AudioError, FrameContext, Game, InputState, PlaybackSettings};
use sindri_scene::{
    AudioSourceComponent, ScenePhysics2d, ScreenExtent, ScreenUi, SpriteAnimations, TileSetBindings,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::assets::sources;
use crate::error::CausewayError;
use crate::streaming::TerrainStream;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type CausewayAudio = NativeAudioBackend;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn causeway_audio_backend() -> Result<CausewayAudio, CausewayError> {
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
    /// What the scenes' tile volumes are made of.
    ///
    /// Empty by default, which is right for a host with no volumes and wrong
    /// the moment one has a floor: an unbound tile set means a script is told
    /// so rather than quietly walking on water.
    tile_sets: TileSetBindings,
    profiles: ProfileSources,
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
    /// What the person just did, read from the presses each frame.
    ///
    /// Lives here rather than being made per frame because recognising a
    /// gesture is a judgement about a press's whole life: a tap is a tap
    /// because of where it started and how long ago, and a recogniser built
    /// fresh each frame would see every press as having just arrived and
    /// never finish recognising anything.
    gestures: sindri_core::Gestures,
    /// The ground under each grid, remembered between frames.
    ///
    /// Kept on the session rather than made each frame, because that is the
    /// whole of the saving: the derivation walks every cell of the volume, and
    /// a generated landscape has enough of them that doing it per frame costs
    /// more than everything else in a frame put together.
    surfaces: sindri_scene::GridSurfaces,
    /// Generated terrain materialized around the camera.
    terrain: TerrainStream,
    since_written: f32,
    /// Where the save actually goes.
    ///
    /// Memory unless a host says otherwise, so a headless run and a test have
    /// somewhere to write without choosing a path. The desktop host names a
    /// file; the browser host uses the page's own storage.
    save_backend: Box<dyn sindri_platform::SaveBackend>,
    pending_audio: Vec<AudioCommand>,
    autoplay_started: bool,
    /// Every scene the project can reach, by the ID a script names.
    scenes: BTreeMap<String, sindri_core::SceneDocument>,
    /// Which of them are in the world, and which one is being played.
    loaded: sindri_core::LoadedScenes,
    /// The scene being played, and where a script asked to go.
    ///
    /// `None` until the host says which scene it opened on. A session that
    /// never says is a session whose scripts are told `Scene.go` cannot work
    /// here, which is the truth rather than a request dropped in silence.
    channel: Option<sindri_decay::SceneChannel>,
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
            tile_sets: TileSetBindings::new(),
            profiles: ProfileSources::new(),
            components,
            animations: SpriteAnimations::new(),
            physics: ScenePhysics2d::top_down().expect("zero gravity is finite"),
            screen_ui: ScreenUi::default(),
            random: sindri_core::Rng::default(),
            saves: sindri_core::SaveStore::default(),
            effects: sindri_scene::Effects2d::default(),
            gestures: sindri_core::Gestures::new(sindri_core::GestureLimits::default()),
            surfaces: sindri_scene::GridSurfaces::default(),
            terrain: TerrainStream::default(),
            since_written: 0.0,
            save_backend: Box::new(sindri_platform::MemorySaves::new()),
            pending_audio: Vec::new(),
            autoplay_started: false,
            scenes: BTreeMap::new(),
            loaded: sindri_core::LoadedScenes::new(),
            channel: None,
        }
    }

    /// The prefabs this project's scripts can spawn.
    ///
    /// A build had no way to be given any, so `World.spawn` in a shipped game
    /// answered that the prefab was missing while the same scene spawned
    /// correctly in the editor. A project whose enemies are prefabs is every
    /// project that spawns anything.
    #[must_use]
    pub fn with_prefabs(mut self, prefabs: PrefabSources) -> Self {
        self.prefabs = prefabs;
        self
    }

    #[must_use]
    pub fn with_profiles(mut self, profiles: ProfileSources) -> Self {
        self.profiles = profiles;
        self
    }

    /// The tile sets the scenes' volumes name.
    ///
    /// Without these a script's pathfinding cannot tell a pond from a lawn:
    /// which cells are solid is the tile set's answer, and the session has no
    /// business guessing it. Gather's floor is a volume, so this is how the
    /// water stops being walkable.
    #[must_use]
    pub fn with_tile_sets(mut self, tile_sets: TileSetBindings) -> Self {
        self.tile_sets = tile_sets;
        self
    }

    /// The scenes this project can reach, and which of them is already open.
    ///
    /// The world arrives with its opening scene in it, so the session is told
    /// what that scene was rather than loading it again: `LoadedScenes` is the
    /// record of what is in the world, and two records of that would be one
    /// too many.
    #[must_use]
    pub fn with_scenes(
        mut self,
        scenes: Vec<(String, sindri_core::SceneDocument)>,
        loaded: sindri_core::LoadedScenes,
    ) -> Self {
        // The scene being played is the one the loader entered. Taken from it
        // rather than from the list's first entry, so the two cannot disagree.
        self.channel = loaded.active().map(sindri_decay::SceneChannel::playing);
        self.scenes = scenes.into_iter().collect();
        self.loaded = loaded;
        self
    }

    /// Which scene is being played, or `None` where the host runs just one.
    #[must_use]
    pub fn scene(&self) -> Option<&str> {
        self.loaded.active()
    }

    /// Performs a scene change a script asked for, if one did.
    ///
    /// Between frames, never inside one: the script that asked is running in
    /// the scene being left, from a world this rearranges underneath it. The
    /// scene it came from is switched off rather than unloaded, so walking back
    /// in finds it as it was.
    fn follow_scene_request(&mut self, world: &mut World) -> Result<(), CausewayError> {
        let Some(channel) = self.channel.as_mut() else {
            return Ok(());
        };
        let Some(wanted) = channel.take() else {
            return Ok(());
        };
        if self.loaded.active() == Some(wanted.as_str()) {
            // Already there. Not an error: two doors into one room, or a script
            // asking twice, should be a no-op rather than a reload that threw
            // the room's state away.
            return Ok(());
        }
        let document = self
            .scenes
            .get(&wanted)
            .ok_or_else(|| CausewayError::UnknownScene(wanted.clone()))?;
        self.loaded.enter(world, &wanted, document)?;
        channel.now_playing(wanted);
        Ok(())
    }

    /// How far this frame's drag asks the camera to move.
    ///
    /// Zero covers every way there is nothing to move: nobody dragging, no
    /// camera, a viewport with no area. They are one situation to a script --
    /// the camera stays where it is.
    fn camera_pan(
        world: &World,
        components: &ComponentSchemaRegistry,
        gestures: &sindri_core::Gestures,
        viewport: (f32, f32),
    ) -> [f32; 3] {
        let Some(drag) = gestures.drag() else {
            return [0.0; 3];
        };
        if viewport.0 <= 0.0 || viewport.1 <= 0.0 {
            return [0.0; 3];
        }
        let Ok(Some(camera)) =
            sindri_scene::world_camera_of(world, components, viewport.0 / viewport.1)
        else {
            return [0.0; 3];
        };
        sindri_scene::pan_for_drag(&camera, viewport, drag).to_array()
    }

    /// Which block the pointer is on, if it is on one.
    ///
    /// `None` covers every way there is nothing to answer -- the pointer
    /// outside the window, no camera, no solid grid, a ray that meets no
    /// block. They are one situation to a script: the person is not pointing
    /// at a block.
    fn aim(
        world: &World,
        components: &ComponentSchemaRegistry,
        input: &InputState,
        viewport: (f32, f32),
    ) -> Option<sindri_scene::voxel::VolumeAim> {
        let position = input.pointer_position()?;
        if viewport.0 <= 0.0 || viewport.1 <= 0.0 {
            return None;
        }
        let camera = sindri_scene::world_camera_of(world, components, viewport.0 / viewport.1)
            .ok()
            .flatten()?;
        sindri_scene::voxel::aim_at(
            world,
            components,
            camera.view_projection,
            [position[0] / viewport.0, position[1] / viewport.1],
        )
    }

    /// One fixed step: the scripts run, then the animations move.
    pub fn step(
        &mut self,
        world: &mut World,
        input: &InputState,
        viewport: (f32, f32),
        delta_seconds: f32,
    ) -> Result<(), CausewayError> {
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
            input.presses(),
        )?;
        // Before the scripts, so a fleck thrown this frame is drawn where it
        // was thrown rather than one frame along.
        self.effects
            .advance(std::time::Duration::from_secs_f32(delta_seconds));
        // Worked out here rather than by the scripts, because this is the
        // layer holding a camera and a viewport. A script asking which block
        // the pointer is on would otherwise have to invert the projection
        // itself, which is the renderer's business leaking into gameplay.
        // Read before the scripts, from the presses this frame already holds,
        // so that what a script is told the person did and where the pointer
        // is are the same instant.
        self.gestures.update(input.presses());
        let aim = Self::aim(world, &self.components, input, viewport);
        // Worked out here rather than by the scripts, for the same reason the
        // aim is: it needs the view matrix and the viewport, and a script has
        // neither. Zero when nothing is being dragged, so a camera script can
        // add it every frame without asking.
        let pan = Self::camera_pan(world, &self.components, &self.gestures, viewport);
        let (physics, events) = self.physics.for_scripts();
        let mut frame = ScriptFrame::new(&self.sources, input, delta_seconds)
            .with_prefabs(&self.prefabs)
            .with_profiles(&self.profiles)
            .with_screen_ui(&self.screen_ui)
            .with_random(&mut self.random)
            .with_saves(&mut self.saves)
            .with_effects(&mut self.effects)
            .with_physics(sindri_decay::Physics2d {
                world: physics,
                events,
            })
            .with_animations(&mut self.animations);
        frame = frame.with_gestures(&self.gestures).with_camera_pan(pan);
        if let Some(aim) = aim {
            frame = frame.with_aim(aim);
        }
        // Handed over only when this host actually binds any, the same way the
        // scene channel is. An empty set is not "no tile sets" to the host that
        // receives it — it is a host that binds tile sets and is missing the
        // one this volume names, which is a project someone broke and deserves
        // the error it gets.
        if !self.tile_sets.is_empty() {
            frame = frame.with_tile_sets(&self.tile_sets);
        }
        // Only when this session is actually playing one of several scenes.
        // Handed over conditionally rather than always, so a host running a
        // single scene has its scripts told `Scene.go` cannot work here instead
        // of having a request accepted and dropped.
        if let Some(channel) = self.channel.as_mut() {
            frame = frame.with_scenes(channel);
        }
        let report = self.scripts.advance(world, &self.components, frame);
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
        // The camera may have moved in Build or Play this frame. Materialize
        // its new neighbourhood before placement/navigation and rendering ask
        // about it; the renderer itself remains mode-agnostic.
        self.terrain.update(world, &self.components, viewport)?;
        // After the scripts, because a walker's depth is a consequence of where
        // this step left it, and before anything draws. Props settle on the
        // first pass and never move again; only what moved costs anything.
        let tile_sets = (!self.tile_sets.is_empty()).then_some(&self.tile_sets);
        if let Err(error) = sindri_scene::resolve_grid_placements(
            world,
            &self.components,
            tile_sets,
            &mut self.surfaces,
        ) {
            log::error!("{error}");
        }
        // Last, so a script's request is performed with no script mid-call in
        // the scene it is leaving.
        self.follow_scene_request(world)?;
        Ok(())
    }

    fn start_autoplay(
        &mut self,
        world: &World,
        audio: &mut dyn AudioBackend,
    ) -> Result<(), CausewayError> {
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

    fn flush_audio(&mut self, audio: &mut dyn AudioBackend) -> Result<(), CausewayError> {
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
    type Error = CausewayError;

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
