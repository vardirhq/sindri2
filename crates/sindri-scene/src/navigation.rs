use std::collections::BTreeMap;

use sindri_core::{EntityId, SceneComponent, Transform3D, World};
use sindri_grid::{
    FootprintError, GridBounds, GridCoord, GridError, GridFootprint, GridOccupancy, GridPath,
    GridPathError, GridPathfinder, GridPlacementError, GridSpace, GridWallError, GridWalls,
    PlanePoint,
};
use thiserror::Error;

use crate::{GridNavigationComponent, GridOccupantComponent, TilemapComponent, TilemapError};

/// One entity's derived placement on a world grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridPlacement {
    pub anchor: GridCoord,
    pub footprint: GridFootprint,
}

/// A validated navigation snapshot derived from the current world.
///
/// Authored components contain stable references, wall endpoints, and relative
/// footprints. This adapter resolves those references to runtime handles,
/// derives anchors from transforms through the tilemap's exact projection, and
/// refuses a partial or conflicting occupancy snapshot. Rebuild it after world
/// transforms or component payloads change.
#[derive(Clone, Debug)]
pub struct WorldGridNavigation {
    grid_entity: EntityId,
    bounds: GridBounds,
    space: GridSpace,
    walls: GridWalls,
    occupancy: GridOccupancy<EntityId>,
    placements: BTreeMap<EntityId, GridPlacement>,
}

impl WorldGridNavigation {
    /// Derives navigation for one explicitly selected tilemap entity.
    pub fn from_world(world: &World, grid_entity: EntityId) -> Result<Self, GridNavigationError> {
        let grid_data = world
            .get(grid_entity)
            .ok_or(GridNavigationError::MissingEntity(grid_entity))?;
        let grid_source = grid_data
            .source_id
            .as_ref()
            .ok_or(GridNavigationError::UnstableGrid(grid_entity))?;
        let tilemap_payload = grid_data
            .components
            .get(TilemapComponent::TYPE_NAME)
            .ok_or(GridNavigationError::MissingTilemap(grid_entity))?;
        let tilemap: TilemapComponent =
            serde_json::from_value(tilemap_payload.clone()).map_err(|source| {
                GridNavigationError::InvalidTilemapPayload {
                    grid: grid_entity,
                    source,
                }
            })?;
        tilemap
            .validate()
            .map_err(|source| GridNavigationError::InvalidTilemap {
                grid: grid_entity,
                source,
            })?;
        let grid_transform = grid_data.transform_3d.unwrap_or_default();
        validate_planar_grid(grid_entity, grid_transform)?;
        let bounds =
            tilemap
                .grid_bounds()
                .map_err(|source| GridNavigationError::InvalidGridGeometry {
                    grid: grid_entity,
                    source,
                })?;
        let space =
            tilemap
                .grid_space()
                .map_err(|source| GridNavigationError::InvalidGridGeometry {
                    grid: grid_entity,
                    source,
                })?;

        let mut walls = GridWalls::new(bounds);
        if let Some(payload) = grid_data.components.get(GridNavigationComponent::TYPE_NAME) {
            let navigation: GridNavigationComponent = serde_json::from_value(payload.clone())
                .map_err(|source| GridNavigationError::InvalidNavigationPayload {
                    grid: grid_entity,
                    source,
                })?;
            for (index, wall) in navigation.walls.into_iter().enumerate() {
                walls
                    .block(coord(wall.first), coord(wall.second))
                    .map_err(|source| GridNavigationError::InvalidWall {
                        grid: grid_entity,
                        index,
                        source,
                    })?;
            }
        }

        let mut occupancy = GridOccupancy::new(bounds);
        let mut placements = BTreeMap::new();
        for (entity, data) in world.entities() {
            let Some(payload) = data.components.get(GridOccupantComponent::TYPE_NAME) else {
                continue;
            };
            let occupant: GridOccupantComponent = serde_json::from_value(payload.clone())
                .map_err(|source| GridNavigationError::InvalidOccupantPayload { entity, source })?;
            if &occupant.grid != grid_source {
                continue;
            }
            let transform = data
                .transform_3d
                .ok_or(GridNavigationError::MissingOccupantTransform(entity))?;
            if !transform.position[0].is_finite() || !transform.position[1].is_finite() {
                return Err(GridNavigationError::InvalidOccupantPosition(entity));
            }
            let local = world_to_grid_plane(grid_transform, transform.position_2d());
            let anchor = space.plane_to_grid(local).map_err(|source| {
                GridNavigationError::InvalidOccupantCoordinate { entity, source }
            })?;
            let footprint = GridFootprint::new(occupant.footprint.into_iter().map(coord))
                .map_err(|source| GridNavigationError::InvalidFootprint { entity, source })?;
            occupancy
                .place(entity, anchor, &footprint)
                .map_err(|source| GridNavigationError::InvalidPlacement { entity, source })?;
            placements.insert(entity, GridPlacement { anchor, footprint });
        }

        Ok(Self {
            grid_entity,
            bounds,
            space,
            walls,
            occupancy,
            placements,
        })
    }

    #[must_use]
    pub const fn grid_entity(&self) -> EntityId {
        self.grid_entity
    }

    #[must_use]
    pub const fn bounds(&self) -> GridBounds {
        self.bounds
    }

    #[must_use]
    pub const fn space(&self) -> GridSpace {
        self.space
    }

    #[must_use]
    pub const fn walls(&self) -> &GridWalls {
        &self.walls
    }

    #[must_use]
    pub const fn occupancy(&self) -> &GridOccupancy<EntityId> {
        &self.occupancy
    }

    #[must_use]
    pub fn placement(&self, entity: EntityId) -> Option<&GridPlacement> {
        self.placements.get(&entity)
    }

    /// Every placed occupant in stable runtime-handle order.
    pub fn placements(&self) -> impl Iterator<Item = (EntityId, &GridPlacement)> {
        self.placements
            .iter()
            .map(|(entity, placement)| (*entity, placement))
    }

    /// Checks an occupant's complete footprint at a prospective anchor.
    pub fn validate_placement(
        &self,
        entity: EntityId,
        anchor: GridCoord,
    ) -> Result<(), GridNavigationError> {
        let placement = self
            .placements
            .get(&entity)
            .ok_or(GridNavigationError::NotAnOccupant(entity))?;
        self.occupancy
            .validate(&entity, anchor, &placement.footprint)
            .map_err(|source| GridNavigationError::InvalidPlacement { entity, source })
    }

    /// Finds a path for an occupant's complete footprint through occupancy and
    /// authored walls.
    pub fn find_path(
        &self,
        pathfinder: GridPathfinder,
        entity: EntityId,
        goal: GridCoord,
    ) -> Result<Option<GridPath>, GridNavigationError> {
        let placement = self
            .placements
            .get(&entity)
            .ok_or(GridNavigationError::NotAnOccupant(entity))?;
        self.occupancy
            .find_path_with_walls(
                pathfinder,
                &self.walls,
                &entity,
                &placement.footprint,
                placement.anchor,
                goal,
            )
            .map_err(GridNavigationError::Path)
    }
}

const fn coord(value: [i32; 2]) -> GridCoord {
    GridCoord::new(value[0], value[1])
}

fn validate_planar_grid(grid: EntityId, transform: Transform3D) -> Result<(), GridNavigationError> {
    let finite = transform.position[0].is_finite()
        && transform.position[1].is_finite()
        && transform.rotation.into_iter().all(f32::is_finite);
    let flat =
        transform.rotation[0].abs() <= f32::EPSILON && transform.rotation[1].abs() <= f32::EPSILON;
    let usable_scale = transform.scale[0].is_finite()
        && transform.scale[1].is_finite()
        && transform.scale[0].abs() > f32::EPSILON
        && transform.scale[1].abs() > f32::EPSILON;
    if !finite || !flat || !usable_scale {
        return Err(GridNavigationError::InvalidGridTransform(grid));
    }
    Ok(())
}

fn world_to_grid_plane(transform: Transform3D, point: [f32; 2]) -> PlanePoint {
    let (sin, cos) = transform.rotation_z_radians().sin_cos();
    let x = point[0] - transform.position[0];
    let y = point[1] - transform.position[1];
    PlanePoint::new(
        f64::from((cos * x + sin * y) / transform.scale[0]),
        f64::from((-sin * x + cos * y) / transform.scale[1]),
    )
}

/// A world cannot be represented as one complete navigation snapshot.
#[derive(Debug, Error)]
pub enum GridNavigationError {
    #[error("grid entity {0:?} does not exist")]
    MissingEntity(EntityId),
    #[error("grid entity {0:?} has no stable scene ID for occupant references")]
    UnstableGrid(EntityId),
    #[error("grid entity {0:?} does not carry sindri.tilemap")]
    MissingTilemap(EntityId),
    #[error("grid entity {grid:?} has an invalid tilemap payload: {source}")]
    InvalidTilemapPayload {
        grid: EntityId,
        #[source]
        source: serde_json::Error,
    },
    #[error("grid entity {grid:?} has an invalid tilemap: {source}")]
    InvalidTilemap {
        grid: EntityId,
        #[source]
        source: TilemapError,
    },
    #[error("grid entity {0:?} needs a finite planar XY transform with non-zero XY scale")]
    InvalidGridTransform(EntityId),
    #[error("grid entity {grid:?} has invalid grid geometry: {source}")]
    InvalidGridGeometry {
        grid: EntityId,
        #[source]
        source: GridError,
    },
    #[error("grid entity {grid:?} has an invalid navigation payload: {source}")]
    InvalidNavigationPayload {
        grid: EntityId,
        #[source]
        source: serde_json::Error,
    },
    #[error("wall {index} on grid entity {grid:?} is invalid: {source}")]
    InvalidWall {
        grid: EntityId,
        index: usize,
        #[source]
        source: GridWallError,
    },
    #[error("grid occupant {entity:?} has an invalid component payload: {source}")]
    InvalidOccupantPayload {
        entity: EntityId,
        #[source]
        source: serde_json::Error,
    },
    #[error("grid occupant {0:?} needs a transform to derive its anchor cell")]
    MissingOccupantTransform(EntityId),
    #[error("grid occupant {0:?} has a non-finite world position")]
    InvalidOccupantPosition(EntityId),
    #[error("grid occupant {entity:?} cannot be converted into a grid cell: {source}")]
    InvalidOccupantCoordinate {
        entity: EntityId,
        #[source]
        source: GridError,
    },
    #[error("grid occupant {entity:?} has an invalid footprint: {source}")]
    InvalidFootprint {
        entity: EntityId,
        #[source]
        source: FootprintError,
    },
    #[error("grid occupant {entity:?} has an invalid placement: {source}")]
    InvalidPlacement {
        entity: EntityId,
        #[source]
        source: GridPlacementError,
    },
    #[error("entity {0:?} is not an occupant of this grid")]
    NotAnOccupant(EntityId),
    #[error(transparent)]
    Path(#[from] GridPathError),
}
