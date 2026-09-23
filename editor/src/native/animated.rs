//! Keeping the viewport moving while something in the scene moves on its own.
//!
//! The editor redraws when something happens, which is right for a still scene
//! and wrong for a lake: its ripple is a function of time, and without a redraw
//! it freezes on whichever frame was showing when the pointer last moved.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use sindri_core::{SceneComponent, World};
use sindri_scene::{TileSetBindings, VoxelWorldComponent};

/// How often a moving scene is redrawn: often enough that a ripple reads as
/// motion, and no more, since a still editor should not spin a core.
pub(super) const FRAME: Duration = Duration::from_millis(33);

/// Seconds since the editor started, the clock animated faces run on.
pub(super) fn seconds() -> f32 {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    STARTED.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Whether anything in the world animates by itself: a voxel world built from
/// a block set in which some block's face has frames.
pub(super) fn moves(world: &World, tile_sets: &TileSetBindings) -> bool {
    world
        .entities()
        .filter(|(entity, _)| world.is_active(*entity))
        .filter_map(|(_, data)| data.components.get(VoxelWorldComponent::TYPE_NAME))
        .filter_map(|payload| payload.get("blocks")?.as_str())
        .filter_map(|reference| tile_sets.get(reference))
        .any(|set| {
            set.tiles.values().any(|block| {
                block
                    .faces
                    .iter()
                    .any(|(_, visual)| visual.animation.is_some())
            })
        })
}
