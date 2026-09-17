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
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GridNavigationComponent {
    #[serde(default)]
    pub walls: Vec<GridWallDocument>,
    /// The biggest height difference a walker crosses between two columns of a
    /// stacked volume, in cells.
    ///
    /// One by default: a walker steps up or down a block, the way it does in
    /// the voxel games this reads like, and a two-block wall stops it. A half
    /// stops it at anything but a slab. Zero makes every change of height a
    /// wall, which is the flat floor a scene had before volumes existed.
    ///
    /// Symmetric, because a wall is: `sindri-grid` blocks an edge rather than a
    /// direction, so a rule letting a walker drop further than it can climb has
    /// nowhere to be recorded. Separate climb and drop limits wait on
    /// directional edges.
    ///
    /// Read only where the volume is the floor. A grid still carrying
    /// `sindri.tilemap` is a migration in progress and keeps navigating by that
    /// flat map alone.
    #[serde(default = "one_cell")]
    pub max_step: f32,
}

fn one_cell() -> f32 {
    1.0
}

impl Default for GridNavigationComponent {
    fn default() -> Self {
        Self {
            walls: Vec::new(),
            max_step: one_cell(),
        }
    }
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
