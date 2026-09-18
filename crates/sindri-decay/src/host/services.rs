//! What a script can reach beyond the world, and how the host is handed it.
//!
//! Its own file because the list is the part that keeps growing: every
//! capability the scripting surface gains -- physics, saves, effects, a random
//! stream, tile sets -- adds a field here and nothing anywhere else. Keeping it
//! beside the dispatch made the file that changes most often the one hardest to
//! read.

use sindri_core::{EntityId, World};

use super::{ProfileSources, ScriptContext, Spawning, WorldHost};
use crate::Blackboard;

/// Everything a script can reach beyond the world and the frame.
///
/// Bundled because the list had reached eight and every capability the scripting
/// surface grows adds another. `crate::HostServices` is this plus the audio
/// queue, which is the wrapper's business rather than this host's.
pub struct WorldServices<'a> {
    pub spawning: Spawning<'a>,
    pub profiles: &'a ProfileSources,
    /// What the game remembers, when the host is keeping a save.
    pub saves: Option<&'a mut sindri_core::SaveStore>,
    /// The fleck pool, when the host is running one.
    pub effects: Option<&'a mut sindri_scene::Effects2d>,
    pub physics: Option<crate::Physics2d<'a>>,
    pub screen_ui: Option<&'a sindri_scene::ScreenUi>,
    /// Which block the pointer is on, when the host picked one this frame.
    pub aim: Option<sindri_scene::voxel::VolumeAim>,
    pub random: Option<&'a mut sindri_core::Rng>,
    /// Where each animated sprite has got to, when the host advances any.
    pub animations: Option<&'a mut sindri_scene::SpriteAnimations>,
    /// Which scene is being played, and where a script asked to go.
    ///
    /// `None` for a host that plays exactly one scene, and then `Scene.go`
    /// says so rather than accepting a request nothing will perform.
    pub scenes: Option<&'a mut crate::SceneChannel>,
    /// The tile sets a stacked volume's cells name, when the host binds any.
    pub tile_sets: Option<&'a sindri_scene::TileSetBindings>,
}

impl<'a> WorldHost<'a> {
    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        services: WorldServices<'a>,
    ) -> Self {
        let WorldServices {
            spawning,
            profiles,
            saves,
            effects,
            physics,
            screen_ui,
            aim,
            random,
            animations,
            scenes,
            tile_sets,
        } = services;
        Self {
            world,
            entity,
            context,
            blackboard,
            spawning,
            profiles,
            physics,
            screen_ui,
            aim,
            random,
            saves,
            effects,
            animations,
            scenes,
            tile_sets,
            walkable: None,
            printed: Vec::new(),
        }
    }
}
