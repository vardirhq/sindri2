//! What one frame's pointer resolved in a viewport.
//!
//! Separate from drawing it. A frame first works out what the pointer is over --
//! a tile, a block, the cell an armed prefab would land on -- and only then
//! paints something over the picture; keeping the answer in its own type is
//! what lets the painting take a shared borrow of an editor the resolving
//! needed mutably.

use sindri_scene::CameraView;

use super::block_pointer::TileVolumeHover;
use super::pointer::TilemapHover;
use super::prefab_pointer::PrefabTarget;

pub(super) enum PaintHover<'a> {
    Tilemap(&'a TilemapHover),
    TileVolume(&'a TileVolumeHover),
}

pub(super) struct ViewInteraction {
    pub(super) editing: bool,
    pub(super) painting: bool,
    pub(super) camera: CameraView,
    pub(super) tilemap_hover: Option<TilemapHover>,
    pub(super) volume_hover: Option<TileVolumeHover>,
    /// The cell an armed prefab would land on.
    pub(super) prefab_target: Option<PrefabTarget>,
}

impl ViewInteraction {
    pub(super) fn hover(&self) -> Option<PaintHover<'_>> {
        self.tilemap_hover
            .as_ref()
            .map(PaintHover::Tilemap)
            .or_else(|| self.volume_hover.as_ref().map(PaintHover::TileVolume))
    }
}
