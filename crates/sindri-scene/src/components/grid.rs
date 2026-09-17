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

/// Places an entity by naming a cell, rather than by naming a position.
///
/// The inverse of `sindri.grid.occupant`, which derives a cell *from* a
/// transform: a scene authors a world position and the runtime works out where
/// it landed. That direction is why a prop can straddle the boundary between
/// two cells, and why nothing moves when the ground beneath it does.
///
/// Here the cell is the authored fact and the transform is derived from it,
/// including the height of the ground in that column and the Z that orders it.
/// `docs/2d-depth-and-placement.md` carries the reasoning; the short version is
/// that an author who never writes a depth cannot write a wrong one.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct GridPlacementComponent {
    /// The stable scene ID of the grid this stands on.
    pub grid: String,
    /// The cell, or `None` for something that moves.
    ///
    /// With a cell, the whole transform is derived and the entity is pinned.
    /// Without one, only the depth is: a walker keeps whatever X and Y its
    /// script gives it, and its Z follows from where that leaves it. Both are
    /// the same rule — depth is a consequence of position — applied to
    /// something that stays put and something that does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[i32; 2]>,
    /// Where inside the cell, as a fraction of it from the centre.
    ///
    /// Snapping everything to a cell centre makes a grid look like a
    /// spreadsheet. An offset lets a rock sit off-centre without leaving the
    /// cell that owns it, and without reintroducing a position nobody can
    /// trace back to one.
    #[serde(default)]
    pub offset: [f32; 2],
}

impl SceneComponent for GridPlacementComponent {
    const TYPE_NAME: &'static str = "sindri.grid.placement";
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
