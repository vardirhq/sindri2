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

use sindri_core::{ComponentSchemaRegistry, EntityId, Transform3D, World};
use sindri_grid::GridCoord;
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
                    .is_some_and(|id| id == &placement.grid)
            })
            .map(|(entity, _)| entity)
            .ok_or_else(|| GridPlacementError::MissingGrid {
                entity,
                grid: placement.grid.as_str().to_owned(),
            })?;
        let Some(grid) = components
            .get::<TileGridComponent>(world, grid_entity)
            .map_err(|source| GridPlacementError::InvalidPayload { source })?
        else {
            return Err(GridPlacementError::NotAGrid {
                entity,
                grid: placement.grid.as_str().to_owned(),
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

    // A grid whose cells are boxes places things in the world rather than on a
    // picture of it. There is no projection to apply and no depth to derive:
    // a column is a distance across, a row is a distance into the scene, and
    // the ground's height is a height.
    if let Some(cell_size) = grid.solid_cell() {
        return place_solid(
            world, entity, placement, cell_size, surfaces, origin, transform,
        );
    }

    // A named cell decides X and Y; without one they are whatever moved the
    // entity last, and only the depth below is answered here.
    let column_row = if let Some([column, row]) = placement.cell {
        {
            // The offset is in cells, so the same two numbers place the entity
            // and decide its depth. In plane units they would be two different
            // quantities that happen to be written together, and a wall on a
            // cell edge would sit between two cells while sorting as though it
            // were in one of them.
            let at = (
                f64::from(column) + f64::from(placement.offset[0]),
                f64::from(row) + f64::from(placement.offset[1]),
            );
            // Every cell it covers has to hold it up. Without a tile set
            // there is nothing to ask -- a volume that cannot say which cells
            // support anything gets the grid's own plane, as it always did --
            // but with one, resting on a hole is an error rather than a prop
            // quietly sitting at height zero, which is how three rocks came to
            // be standing off Gather's shore on nothing.
            let anchor = GridCoord::new(column, row);
            let height = if let Some(surfaces) = surfaces {
                for cell in footprint(placement, anchor) {
                    if surfaces.height(cell).is_none() {
                        return Err(GridPlacementError::UnsupportedCell {
                            entity,
                            x: cell.x,
                            y: cell.y,
                        });
                    }
                }
                surfaces.height(anchor).unwrap_or(0.0)
            } else {
                0.0
            };
            let [x, y] = grid
                .point_to_local_at_height(at.0, at.1, height)
                .ok_or(GridPlacementError::UnprojectableCell { entity })?;
            transform.position[0] = origin[0] + x;
            transform.position[1] = origin[1] + y;
            (at, height)
        }
    } else {
        {
            let space = grid
                .grid_space()
                .map_err(|_| GridPlacementError::UnprojectableCell { entity })?;
            let point = space
                .unproject(sindri_grid::PlanePoint::new(
                    f64::from(transform.position[0] - origin[0]),
                    f64::from(transform.position[1] - origin[1]),
                ))
                .map_err(|_| GridPlacementError::UnprojectableCell { entity })?;
            // A walker's cell is wherever its script has put it, so the ground
            // under it is read from the column it is standing in rather than
            // from an authored cell it does not have.
            //
            // The *walkable* surface, not the supporting one, and the
            // difference is a player standing on a pond. An authored occupant
            // rests on whatever holds it up -- that is what a pier or a lily on
            // water is -- but a walker stands only on what it can stand on, so
            // over water it is on the grid's own plane rather than lifted onto
            // the surface. Whether it should be over water at all is a question
            // for whatever moves it; `Grid.walkable` is how a script asks.
            let height = surfaces
                .and_then(|surfaces| surfaces.walkable_height(nearest_cell(point.x, point.y)))
                .unwrap_or(0.0);
            ((point.x, point.y), height)
        }
    };

    let (column_row, height) = column_row;
    let depth = standing_depth(grid, surfaces, column_row, height);
    transform.position[2] = origin[2] + grid.depth_z(depth);
    if let Some(data) = world.get_mut(entity) {
        data.transform_3d = Some(transform);
    }
    Ok(())
}

/// Where a thing stands on a grid of boxes.
///
/// The whole of what the projected path below computes -- a plane point, a
/// projected offset for height, a depth key, a Z derived from it -- collapses
/// to three coordinates, because in a world with boxes in it a position is a
/// position. What covers what is the depth buffer's answer, so nothing here
/// has an opinion about draw order.
fn place_solid(
    world: &mut World,
    entity: EntityId,
    placement: &GridPlacementComponent,
    cell_size: [f32; 3],
    surfaces: Option<&TileSurfaces>,
    origin: [f32; 3],
    mut transform: Transform3D,
) -> Result<(), GridPlacementError> {
    let [across, into, up] = cell_size;
    let (column, row) = if let Some([column, row]) = placement.cell {
        // Every cell it covers has to hold it up, exactly as on a flat grid:
        // resting on a hole is an error rather than a prop quietly sitting at
        // height zero.
        if let Some(surfaces) = surfaces {
            for cell in footprint(placement, GridCoord::new(column, row)) {
                if surfaces.height(cell).is_none() {
                    return Err(GridPlacementError::UnsupportedCell {
                        entity,
                        x: cell.x,
                        y: cell.y,
                    });
                }
            }
        }
        (
            f64::from(column) + f64::from(placement.offset[0]),
            f64::from(row) + f64::from(placement.offset[1]),
        )
    } else {
        // A walker's cell is wherever whatever moves it has put it, read back
        // out of the position it already holds.
        (
            f64::from(transform.position[0] - origin[0]) / f64::from(across),
            f64::from(transform.position[2] - origin[2]) / f64::from(into),
        )
    };

    let standing = nearest_cell(column, row);
    let height = match (surfaces, placement.cell.is_some()) {
        // An authored occupant rests on whatever holds it up; a walker stands
        // only on what it can stand on, which is what keeps it off a pond.
        (Some(surfaces), true) => surfaces.height(standing).unwrap_or(0.0),
        (Some(surfaces), false) => surfaces.walkable_height(standing).unwrap_or(0.0),
        (None, _) => 0.0,
    };

    transform.position = [
        origin[0] + narrow(column) * across,
        origin[1] + height * up,
        origin[2] + narrow(row) * into,
    ];
    if let Some(data) = world.get_mut(entity) {
        data.transform_3d = Some(transform);
    }
    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
fn narrow(value: f64) -> f32 {
    value as f32
}

/// The cell a continuous grid point stands in.
///
/// `GridSpace` puts the integer coordinate at the centre of its cell, so the
/// nearest whole column and row are the cell something is standing in.
///
/// Public because a script asking about the ground under it asks in the same
/// continuous coordinates a walker moves in, and two answers to "which cell is
/// that" is two disagreeing maps.
#[must_use]
pub fn nearest_cell(column: f64, row: f64) -> GridCoord {
    #[allow(clippy::cast_possible_truncation)]
    GridCoord::new(column.round() as i32, row.round() as i32)
}

/// How deep something standing here is drawn.
///
/// The counterpart of `TileGridComponent::face_depth`, and the other half of
/// the one rule that orders a world against the ground under it. Public because
/// a sweep that walks every standable surface has to ask the question the
/// renderer asks rather than a copy of it.
#[must_use]
pub fn standing_depth(
    grid: &TileGridComponent,
    surfaces: Option<&TileSurfaces>,
    (column, row): (f64, f64),
    height: f32,
) -> f64 {
    grid.depth_at(column, row) + clearance_ahead(grid, surfaces, (column, row), height)
}

/// How far forward something standing here has to sort to clear its own ground.
///
/// Depth alone put the ground in front of the player standing on it. The column
/// ahead of a walker is nearer than the one under their feet, so its top face --
/// flat, at exactly the height they stand at -- won on depth and drew over
/// their legs. Nothing is in front of a walker at the height they are standing
/// at, and a cell whose surface is no higher than their feet has no face that
/// can cover them: not its top, and not the walls holding that top up.
///
/// So a walker sorts forward, past the ground it is walking into. How far is
/// decided by what it must not overtake. A cell sits half a step back from its
/// own column, so the ground a step ahead is half a step in front of the walker
/// and has to be cleared; something *standing* on that ground is a whole step
/// in front and must not be, or a walker would draw through the tree it is
/// walking behind. Three quarters of a step is between them.
///
/// The cells that can reach it are the ones a step ahead -- a diagonal is two
/// steps of depth away and its top never rises far enough to touch the sprite --
/// so those are the only ones asked about.
///
/// A step ahead that *is* higher gets the walker's place back. A block raised
/// beside them is a wall rather than ground, it can cover them, and moving the
/// walker past it would be the original bug with the roles swapped. Standing in
/// the corner between a wall and open ground is the one case this cannot answer
/// for both at once, and it answers for the wall.
fn clearance_ahead(
    grid: &TileGridComponent,
    surfaces: Option<&TileSurfaces>,
    (column, row): (f64, f64),
    height: f32,
) -> f64 {
    if blocking_step_ahead(grid, surfaces, (column, row), height).is_some() {
        return 0.0;
    }
    if surfaces.is_none() {
        return 0.0;
    }
    CLEARANCE
}

/// The raised step ahead that takes a walker's clearance away, if there is one.
///
/// The one case the rule cannot answer for both sides of. A walker beside a
/// raised block needs the block to cover it and needs the open ground on its
/// other side not to, and one depth grants exactly one of those. It grants the
/// block, because a walker drawn through a wall is worse than a seam against
/// the ground.
///
/// Public because that is a fact about the scene rather than about the walker:
/// a sweep reports where it bites, and the editor can draw it.
#[must_use]
pub fn blocking_step_ahead(
    grid: &TileGridComponent,
    surfaces: Option<&TileSurfaces>,
    (column, row): (f64, f64),
    height: f32,
) -> Option<GridCoord> {
    let surfaces = surfaces?;
    let here = nearest_cell(column, row);
    let depth = grid.depth_at(f64::from(here.x), f64::from(here.y));
    [
        GridCoord::new(here.x + 1, here.y),
        GridCoord::new(here.x, here.y + 1),
        GridCoord::new(here.x - 1, here.y),
        GridCoord::new(here.x, here.y - 1),
    ]
    .into_iter()
    .filter(|cell| grid.depth_at(f64::from(cell.x), f64::from(cell.y)) > depth)
    // An empty column is a hole rather than a wall, and nothing in it can
    // cover anything.
    .find(|cell| surfaces.height(*cell).is_some_and(|top| top > height))
}

/// How far forward standing on the ground sorts something, in cells.
///
/// Above the half step a cell sits back from its column, and below the whole
/// step to whatever is standing on the next one.
const CLEARANCE: f64 = 0.75;

/// Every cell a placement covers, anchored on the one it names.
fn footprint(
    placement: &GridPlacementComponent,
    anchor: GridCoord,
) -> impl Iterator<Item = GridCoord> + '_ {
    placement
        .footprint
        .iter()
        .map(move |[x, y]| GridCoord::new(anchor.x + x, anchor.y + y))
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
    #[error("entity {entity:?} covers ({x}, {y}), where nothing holds it up")]
    UnsupportedCell { entity: EntityId, x: i32, y: i32 },
    #[error("a placed grid's volume cannot be read: {source}")]
    InvalidSurface {
        #[source]
        source: TileSurfaceError,
    },
}
