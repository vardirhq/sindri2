//! Deriving a transform from a cell, and a depth from a position.
//!
//! `docs/2d-model.md` decided that Z is a position and that the camera sorts by
//! it. What it left open was how an author manages a Z that decides depth and
//! nothing else — a question Gather answered by not answering it, pinning every
//! prop at `Z = 0` and writing seventy-four render layers instead.
//!
//! The answer here is that nobody manages it. A thing standing on a grid says
//! which cell it stands on, and everything else about where it is — the plane
//! position, the height of the ground under it, and the depth that orders it —
//! follows. An author who never writes a depth cannot write a wrong one.
//!
//! Run after anything that moves the world and before it is drawn. Static props
//! settle on the first pass and never move again; a walker's X and Y come from
//! its script and only its depth is answered here.

use sindri_core::{ComponentSchemaRegistry, EntityId, World};
use thiserror::Error;

use crate::{
    GridPlacementComponent, TileGridComponent, TileSetBindings, TileSurfaceError, TileSurfaces,
    TileVolumeComponent,
};

/// Resolves every `sindri.grid.placement` in the world.
///
/// Tile sets are optional and only decide how high the ground is: without them
/// a volume cannot say which of its cells are solid, so placement puts things
/// on the grid's own plane rather than guessing a surface. That is the same
/// refusal `WorldGridNavigation::from_world` makes for the same reason.
pub fn resolve_grid_placements(
    world: &mut World,
    components: &ComponentSchemaRegistry,
    tile_sets: Option<&TileSetBindings>,
) -> Result<usize, GridPlacementError> {
    let placements = components
        .query::<GridPlacementComponent>(world)
        .map_err(|source| GridPlacementError::InvalidPayload { source })?;
    if placements.is_empty() {
        return Ok(0);
    }

    let mut resolved = 0;
    for (entity, placement) in placements {
        let grid_entity = world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == placement.grid)
            })
            .map(|(entity, _)| entity)
            .ok_or_else(|| GridPlacementError::MissingGrid {
                entity,
                grid: placement.grid.clone(),
            })?;
        let Some(grid) = components
            .get::<TileGridComponent>(world, grid_entity)
            .map_err(|source| GridPlacementError::InvalidPayload { source })?
        else {
            return Err(GridPlacementError::NotAGrid {
                entity,
                grid: placement.grid.clone(),
            });
        };
        let surfaces = surfaces_of(world, components, grid_entity, tile_sets)?;
        let origin = world
            .get(grid_entity)
            .and_then(|data| data.transform_3d)
            .unwrap_or_default()
            .position;

        place(world, entity, &placement, &grid, surfaces.as_ref(), origin)?;
        resolved += 1;
    }
    Ok(resolved)
}

/// The ground heights of the volume on the grid entity, when one is readable.
fn surfaces_of(
    world: &World,
    components: &ComponentSchemaRegistry,
    grid_entity: EntityId,
    tile_sets: Option<&TileSetBindings>,
) -> Result<Option<TileSurfaces>, GridPlacementError> {
    let Some(tile_sets) = tile_sets else {
        return Ok(None);
    };
    let Some(volume) = components
        .get::<TileVolumeComponent>(world, grid_entity)
        .map_err(|source| GridPlacementError::InvalidPayload { source })?
    else {
        return Ok(None);
    };
    let Some(tile_set) = tile_sets.get(&volume.tileset) else {
        return Ok(None);
    };
    TileSurfaces::derive(&volume, tile_set)
        .map(Some)
        .map_err(|source| GridPlacementError::InvalidSurface { source })
}

fn place(
    world: &mut World,
    entity: EntityId,
    placement: &GridPlacementComponent,
    grid: &TileGridComponent,
    surfaces: Option<&TileSurfaces>,
    origin: [f32; 3],
) -> Result<(), GridPlacementError> {
    let mut transform = world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default();

    // A named cell decides X and Y; without one they are whatever moved the
    // entity last, and only the depth below is answered here.
    let column_row = match placement.cell {
        Some([column, row]) => {
            // The offset is in cells, so the same two numbers place the entity
            // and decide its depth. In plane units they would be two different
            // quantities that happen to be written together, and a wall on a
            // cell edge would sit between two cells while sorting as though it
            // were in one of them.
            let at = (
                f64::from(column) + f64::from(placement.offset[0]),
                f64::from(row) + f64::from(placement.offset[1]),
            );
            let height = surfaces
                .and_then(|surfaces| surfaces.height(sindri_grid::GridCoord::new(column, row)))
                .unwrap_or(0.0);
            let [x, y] = grid
                .point_to_local_at_height(at.0, at.1, height)
                .ok_or(GridPlacementError::UnprojectableCell { entity })?;
            transform.position[0] = origin[0] + x;
            transform.position[1] = origin[1] + y;
            at
        }
        None => {
            let space = grid
                .grid_space()
                .map_err(|_| GridPlacementError::UnprojectableCell { entity })?;
            let point = space
                .unproject(sindri_grid::PlanePoint::new(
                    f64::from(transform.position[0] - origin[0]),
                    f64::from(transform.position[1] - origin[1]),
                ))
                .map_err(|_| GridPlacementError::UnprojectableCell { entity })?;
            (point.x, point.y)
        }
    };

    transform.position[2] = origin[2] + grid.depth_z(grid.depth_at(column_row.0, column_row.1));
    if let Some(data) = world.get_mut(entity) {
        data.transform_3d = Some(transform);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum GridPlacementError {
    #[error("a grid placement payload is invalid: {source}")]
    InvalidPayload {
        #[source]
        source: sindri_core::ComponentRegistryError,
    },
    #[error("entity {entity:?} is placed on grid `{grid}`, which no entity carries")]
    MissingGrid { entity: EntityId, grid: String },
    #[error("entity {entity:?} is placed on `{grid}`, which has no tile grid")]
    NotAGrid { entity: EntityId, grid: String },
    #[error("entity {entity:?} is placed on a cell the grid cannot project")]
    UnprojectableCell { entity: EntityId },
    #[error("a placed grid's volume cannot be read: {source}")]
    InvalidSurface {
        #[source]
        source: TileSurfaceError,
    },
}
