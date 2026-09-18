//! Which component is the ground, and what shape that ground is.
//!
//! Two questions that only look like one. *Where* a cell is comes from whatever
//! grid component the entity carries, and the flat map and a tile volume answer
//! it identically. *Whether something may walk there* is a volume's question
//! alone: a flat map has one level and no holes, so it has nothing to say.
//!
//! Keeping both here leaves `WorldGridNavigation` to do what it is for --
//! resolving authored references into a snapshot -- rather than also knowing
//! how a stack of cells becomes a wall.

use sindri_core::{EntityData, EntityId, SceneComponent};
use sindri_grid::{GridBounds, GridCoord, GridSpace, GridWalls};

use crate::{
    TileGridComponent, TileSetBindings, TileSurfaces, TileVolumeComponent, TilemapComponent,
};

use super::GridNavigationError;

/// Turns a volume's shape into walls the pathfinder already understands.
///
/// Every edge between two columns is either a step something can take or one it
/// cannot, and `sindri-grid` already refuses to cross a blocked edge. So rather
/// than teach the pathfinder about levels, the volume is reduced to a surface
/// height per column and the edges that are too big a step -- or that lead into
/// a column with no floor at all -- become walls like any other.
///
/// Only cardinal edges exist, so only cardinal edges are considered. Each is
/// visited once, from its western and northern side, because a wall is
/// symmetric and blocking it twice would say the same thing twice.
pub(super) fn block_unwalkable_steps(
    grid: EntityId,
    data: &EntityData,
    tile_sets: &TileSetBindings,
    bounds: GridBounds,
    max_step: f32,
    walls: &mut GridWalls,
) -> Result<(), GridNavigationError> {
    let Some(payload) = data.components.get(TileVolumeComponent::TYPE_NAME) else {
        return Ok(());
    };
    let volume: TileVolumeComponent = serde_json::from_value(payload.clone())
        .map_err(|source| GridNavigationError::InvalidTileVolumePayload { grid, source })?;
    let Some(tile_set) = tile_sets.get(&volume.tileset) else {
        return Err(GridNavigationError::UnboundTileSet {
            grid,
            tile_set: volume.tileset.clone(),
        });
    };
    let surfaces = TileSurfaces::derive(&volume, tile_set)
        .map_err(|source| GridNavigationError::InvalidTileSurface { grid, source })?;

    for from in bounds.iter() {
        for to in [
            GridCoord::new(from.x.saturating_add(1), from.y),
            GridCoord::new(from.x, from.y.saturating_add(1)),
        ] {
            if !bounds.contains(to) || surfaces.step_is_walkable(from, to, max_step) {
                continue;
            }
            walls
                .block(from, to)
                .map_err(|source| GridNavigationError::DerivedWall { grid, source })?;
        }
    }
    Ok(())
}

/// Which component is the floor a walker walks on.
///
/// Only one of them can be, and a scene carrying both is mid-migration rather
/// than ambiguous. While the flat map is there it is the floor, so adding a
/// volume beside an existing floor never changes where anything may walk; the
/// volume becomes the floor in the change that removes the flat map.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GridFloor {
    FlatMap,
    Volume,
}

/// The grid's bounds and plane, from whichever component describes them.
///
/// `sindri.tilemap` and `sindri.tile_grid` place cells identically; they differ
/// in what a cell holds, which navigation does not ask about. Taking either
/// means a scene can move its floor onto a tile volume without its walls,
/// occupants, and pathfinding having to move in the same change.
///
/// The flat map is tried first, so a scene carrying both — a migration in
/// progress — navigates exactly as it did before the second component existed.
/// A grid's shape, and which plane its cells stand on.
///
/// `solid` is the cell's footprint when the grid holds boxes rather than a
/// picture of them, and `None` when it holds the picture. It decides which
/// world axes a walker's position is read from, which is the one thing about a
/// grid that a flat map and a volume genuinely disagree on.
pub(super) struct GridGeometry {
    pub(super) bounds: GridBounds,
    pub(super) space: GridSpace,
    pub(super) floor: GridFloor,
    pub(super) solid: Option<[f32; 2]>,
}

pub(super) fn grid_geometry(
    grid: EntityId,
    data: &EntityData,
) -> Result<GridGeometry, GridNavigationError> {
    if let Some(payload) = data.components.get(TilemapComponent::TYPE_NAME) {
        let tilemap: TilemapComponent = serde_json::from_value(payload.clone())
            .map_err(|source| GridNavigationError::InvalidTilemapPayload { grid, source })?;
        tilemap
            .validate()
            .map_err(|source| GridNavigationError::InvalidTilemap { grid, source })?;
        return Ok(GridGeometry {
            bounds: tilemap
                .grid_bounds()
                .map_err(|source| GridNavigationError::InvalidGridGeometry { grid, source })?,
            space: tilemap
                .grid_space()
                .map_err(|source| GridNavigationError::InvalidGridGeometry { grid, source })?,
            floor: GridFloor::FlatMap,
            // A flat map is a picture of ground, never boxes standing on it.
            solid: None,
        });
    }
    if let Some(payload) = data.components.get(TileGridComponent::TYPE_NAME) {
        let tile_grid: TileGridComponent = serde_json::from_value(payload.clone())
            .map_err(|source| GridNavigationError::InvalidTileGridPayload { grid, source })?;
        tile_grid
            .validate()
            .map_err(|source| GridNavigationError::InvalidTileGrid { grid, source })?;
        return Ok(GridGeometry {
            bounds: tile_grid
                .bounds()
                .map_err(|source| GridNavigationError::InvalidGridGeometry { grid, source })?,
            space: tile_grid
                .grid_space()
                .map_err(|source| GridNavigationError::InvalidGridGeometry { grid, source })?,
            floor: GridFloor::Volume,
            solid: tile_grid
                .solid_cell()
                .map(|[across, into, _up]| [across, into]),
        });
    }
    Err(GridNavigationError::MissingGrid(grid))
}
