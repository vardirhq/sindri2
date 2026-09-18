//! The frame a caller assembles, and what it may offer a pass of scripts.
//!
//! Split from the pass that runs it because they change for different reasons.
//! This grows a field every time the scripting surface gains a capability the
//! host has to supply -- the physics it may drive, the screen it may read, the
//! block the pointer is on. How a pass compiles, ticks and settles its spawns
//! does not.

use sindri_platform::InputState;

use super::ScriptSources;
use crate::{Physics2d, PrefabSources, ProfileSources};

/// Everything one pass of scripts needs from the frame around it.
///
/// A struct rather than parameters, because the list had reached six and every
/// capability the scripting surface grows adds another: the input, the prefabs
/// it may spawn, the physics it may drive. A caller that does not offer one of
/// them says so by leaving a field out of the literal rather than by passing
/// something empty in the right position.
pub struct ScriptFrame<'a> {
    pub sources: &'a ScriptSources,
    pub prefabs: &'a PrefabSources,
    /// Which scene is being played, and where a script asked to go.
    ///
    /// `None` for a host that plays exactly one scene, and then `Scene.go`
    /// says so rather than accepting a request nothing will perform.
    pub scenes: Option<&'a mut crate::SceneChannel>,
    pub profiles: &'a ProfileSources,
    pub input: &'a InputState,
    /// The physics a script may read and drive, when the host runs any.
    ///
    /// `None` for a host with no physics — a headless test, a scene that never
    /// authored a collider — and then a script calling `Physics.*` is told so
    /// rather than quietly doing nothing.
    pub physics: Option<Physics2d<'a>>,
    /// Where the screen elements are and what the pointer is doing to them.
    ///
    /// `None` for a host that draws no UI, and then `Ui.is_pressed` says so
    /// rather than answering that nothing was clicked — a menu whose buttons
    /// never respond because nothing is laying them out should be heard about
    /// on the first frame, not mistaken for a person who has not clicked yet.
    pub screen_ui: Option<&'a sindri_scene::ScreenUi>,
    /// Which block the pointer is on, when the host picked one this frame.
    pub aim: Option<sindri_scene::voxel::VolumeAim>,
    /// The run's random stream, when the host is running one.
    ///
    /// `None` for a host that seeds nothing, and then `Random.value` says so
    /// rather than handing out the same number for ever — a game whose waves
    /// never vary should hear about it on the first frame.
    pub random: Option<&'a mut sindri_core::Rng>,
    /// What the game remembers, when the host is keeping a save.
    ///
    /// `None` for a host that keeps none, and then `Save.*` says so rather than
    /// accepting writes that go nowhere — a game whose progress silently never
    /// persists should be heard about on the first frame, not after someone has
    /// played for an hour.
    pub saves: Option<&'a mut sindri_core::SaveStore>,
    /// The fleck pool, when the host is running one.
    ///
    /// `None` for a host that draws none, and then `Effects.burst` says so
    /// rather than throwing flecks nobody will ever see.
    pub effects: Option<&'a mut sindri_scene::Effects2d>,
    /// Where each animated sprite has got to, when the host advances any.
    ///
    /// `None` for a host that advances none, and then `Animation.is_finished`
    /// answers that nothing has finished — which is true of a host where
    /// nothing is playing. Naming a clip still writes the world, because which
    /// clip plays is authored state and is the scene's whether or not anything
    /// is drawing it.
    pub animations: Option<&'a mut sindri_scene::SpriteAnimations>,
    /// The tile sets a stacked volume's cells name, when the host has loaded
    /// any.
    ///
    /// `None` for a host that binds none, and then a volume says nothing about
    /// where a walker may go and `Grid.set_block` refuses rather than writing a
    /// tile name nothing can draw. What a cell holds is the tile set's answer,
    /// and a host without one has no business guessing it.
    pub tile_sets: Option<&'a sindri_scene::TileSetBindings>,
    pub delta_seconds: f32,
}

impl<'a> ScriptFrame<'a> {
    /// A frame with sources and nothing else, for a caller that only runs
    /// scripts.
    #[must_use]
    pub fn new(sources: &'a ScriptSources, input: &'a InputState, delta_seconds: f32) -> Self {
        Self {
            sources,
            prefabs: PrefabSources::none(),
            scenes: None,
            profiles: ProfileSources::none(),
            input,
            physics: None,
            screen_ui: None,
            aim: None,
            random: None,
            saves: None,
            effects: None,
            animations: None,
            tile_sets: None,
            delta_seconds,
        }
    }

    /// The same frame, with the tile sets a volume's cells name.
    #[must_use]
    pub fn with_tile_sets(mut self, tile_sets: &'a sindri_scene::TileSetBindings) -> Self {
        self.tile_sets = Some(tile_sets);
        self
    }

    /// The same frame, with prefabs a script may spawn.
    #[must_use]
    pub fn with_prefabs(mut self, prefabs: &'a PrefabSources) -> Self {
        self.prefabs = prefabs;
        self
    }

    /// The same frame, with the scene it is being played in.
    ///
    /// Handing this over is what makes `Scene.go` available: without it a
    /// script asking to move is told the host plays one scene, rather than
    /// having its request quietly dropped.
    #[must_use]
    pub fn with_scenes(mut self, scenes: &'a mut crate::SceneChannel) -> Self {
        self.scenes = Some(scenes);
        self
    }

    /// The same frame, with reusable profiles a script may read.
    #[must_use]
    pub fn with_profiles(mut self, profiles: &'a ProfileSources) -> Self {
        self.profiles = profiles;
        self
    }

    /// The same frame, with the screen elements a script may ask about.
    #[must_use]
    pub const fn with_screen_ui(mut self, screen_ui: &'a sindri_scene::ScreenUi) -> Self {
        self.screen_ui = Some(screen_ui);
        self
    }

    /// The same frame, with the block the pointer is on.
    ///
    /// The host works this out because it is the one holding a camera and a
    /// viewport. Left unset, `Aim.hit` is false and a builder script simply
    /// finds the person pointing at nothing -- which is the right answer for a
    /// host with no pointer rather than an error to report.
    #[must_use]
    pub const fn with_aim(mut self, aim: sindri_scene::voxel::VolumeAim) -> Self {
        self.aim = Some(aim);
        self
    }

    /// The same frame, with the run's random stream.
    #[must_use]
    pub fn with_random(mut self, random: &'a mut sindri_core::Rng) -> Self {
        self.random = Some(random);
        self
    }

    /// The same frame, with what the game remembers.
    #[must_use]
    pub fn with_saves(mut self, saves: &'a mut sindri_core::SaveStore) -> Self {
        self.saves = Some(saves);
        self
    }

    /// The same frame, with the fleck pool a script may throw into.
    #[must_use]
    pub fn with_effects(mut self, effects: &'a mut sindri_scene::Effects2d) -> Self {
        self.effects = Some(effects);
        self
    }

    /// The same frame, with physics a script may read and drive.
    #[must_use]
    pub fn with_physics(mut self, physics: Physics2d<'a>) -> Self {
        self.physics = Some(physics);
        self
    }

    /// The same frame, with the animation cursors a script may read and reset.
    #[must_use]
    pub fn with_animations(mut self, animations: &'a mut sindri_scene::SpriteAnimations) -> Self {
        self.animations = Some(animations);
        self
    }
}
