//! The grid components: what a cell costs to cross, and what occupies one.

use serde::Deserialize;
use sindri_core::{SceneComponent, SceneEntityId};

/// One authored wall between two edge-sharing cells.
///
/// The renderer-free grid normalizes the direction and validates the bounds;
/// the document keeps the two endpoints because they are the smallest honest
/// representation an editor can paint and save.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub struct GridWallDocument {
    pub first: [i32; 2],
    pub second: [i32; 2],
}

/// Navigation data authored on the same entity as a tilemap.
///
/// Bounds and projection deliberately remain the tilemap's authority. Keeping
/// a second copy here would let rendering and gameplay describe different
/// grids while both payloads remained individually valid.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct GridNavigationComponent {
    #[serde(default)]
    pub walls: Vec<GridWallDocument>,
}

impl SceneComponent for GridNavigationComponent {
    const TYPE_NAME: &'static str = "sindri.grid.navigation";
}

/// Marks an entity as occupying cells on one authored grid.
///
/// `grid` is a stable scene ID rather than a runtime handle. The runtime
/// navigation adapter resolves it every time it derives occupancy, so saving a
/// scene never persists allocation details. The entity's world transform says
/// which cell is its anchor; `footprint` only says which cells it covers
/// relative to that anchor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GridOccupantComponent {
    pub grid: SceneEntityId,
    #[serde(default = "single_cell_footprint")]
    pub footprint: Vec<[i32; 2]>,
}

fn single_cell_footprint() -> Vec<[i32; 2]> {
    vec![[0, 0]]
}

impl SceneComponent for GridOccupantComponent {
    const TYPE_NAME: &'static str = "sindri.grid.occupant";
}
