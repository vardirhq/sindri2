//! A gameplay grid as a script sees it: logical cells, and where they are in
//! the world.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{EntityId, Transform3D};
use sindri_grid::{GridCoord, GridPathfinder, GridPoint, GridSpace, PlanePoint};
use sindri_scene::WorldGridNavigation;

use crate::surface::GridCall;

use super::WorldHost;
use super::convert::{as_f32, number};
use super::geometry;

/// The renderer-independent grid and the entity transform placing it in world
/// XY. Kept together because a logical coordinate only means a world position
/// relative to both.
#[derive(Clone, Copy)]
pub(super) struct MapGrid {
    pub(super) space: GridSpace,
    pub(super) transform: Transform3D,
    /// How wide and how deep a cell is, when the grid's cells are boxes.
    ///
    /// Present or absent decides which plane a logical coordinate means. A
    /// projected grid draws its map on world XY and derives a depth; a solid
    /// grid stands its cells on world XZ and leaves height to whatever holds
    /// the occupant up. Nothing else about a `Grid.*` call differs.
    pub(super) solid: Option<[f64; 2]>,
}

impl MapGrid {
    /// Where a logical point is in the world, keeping whatever the grid does
    /// not own.
    ///
    /// A projected grid owns X and Y and leaves Z to the depth resolver; a
    /// solid grid owns X and Z and leaves height to the ground. `was` supplies
    /// the axis that is not this grid's to answer.
    pub(super) fn to_world(
        self,
        path: &Path,
        point: GridPoint,
        was: [f32; 3],
    ) -> Result<[f32; 3], RuntimeError> {
        if let Some([across, into]) = self.solid {
            return Ok([
                self.transform.position[0] + as_f32(point.x * across) * self.transform.scale[0],
                was[1],
                self.transform.position[2] + as_f32(point.y * into) * self.transform.scale[2],
            ]);
        }
        let local = self
            .space
            .project(point)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        let flat = map_to_world(self.transform, local);
        Ok([flat[0], flat[1], was[2]])
    }

    /// Which logical point a world position stands on.
    pub(super) fn to_grid(self, path: &Path, world: [f32; 3]) -> Result<GridPoint, RuntimeError> {
        if let Some([across, into]) = self.solid {
            return Ok(GridPoint::new(
                f64::from((world[0] - self.transform.position[0]) / self.transform.scale[0])
                    / across,
                f64::from((world[2] - self.transform.position[2]) / self.transform.scale[2]) / into,
            ));
        }
        let local = world_to_map(self.transform, [world[0], world[1]]);
        self.space
            .unproject(local)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))
    }

    /// The cell a world position stands in.
    ///
    /// Half-open on both axes, exactly as `GridSpace::plane_to_grid` is, so a
    /// boundary belongs to the same cell whichever grid asked.
    pub(super) fn cell_at(self, path: &Path, world: [f32; 3]) -> Result<GridCoord, RuntimeError> {
        if self.solid.is_none() {
            let local = world_to_map(self.transform, [world[0], world[1]]);
            return self
                .space
                .plane_to_grid(local)
                .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())));
        }
        let point = self.to_grid(path, world)?;
        let cell = |value: f64| {
            let cell = (value + 0.5).floor();
            (cell.is_finite() && cell >= f64::from(i32::MIN) && cell <= f64::from(i32::MAX))
                .then_some(cell)
                .ok_or_else(|| {
                    RuntimeError::Host(format!(
                        "{} was asked about a cell too far away",
                        path.dotted()
                    ))
                })
        };
        #[allow(clippy::cast_possible_truncation)]
        Ok(GridCoord::new(cell(point.x)? as i32, cell(point.y)? as i32))
    }
}

/// What a solid grid's transform is allowed to be.
///
/// A projected grid may be turned within the picture it draws, because its Z
/// rotation turns the map on the surface it is painted on. A solid grid has no
/// picture: its cells are boxes standing on world XZ, and a rotation would
/// leave a cell's box and a script's idea of where that cell is pointing in
/// different directions. Rather than quietly place things into the gap, a
/// turned solid grid is refused.
pub(super) fn validate_solid_map(path: &Path, transform: Transform3D) -> Result<(), RuntimeError> {
    let finite = transform.position.into_iter().all(f32::is_finite);
    let square = transform.rotation[0].abs() <= f32::EPSILON
        && transform.rotation[1].abs() <= f32::EPSILON
        && transform.rotation[2].abs() <= f32::EPSILON;
    let usable_scale = [transform.scale[0], transform.scale[2]]
        .into_iter()
        .all(|value| value.is_finite() && value.abs() > f32::EPSILON);
    if !finite || !square || !usable_scale {
        return Err(RuntimeError::Host(format!(
            "{} needs a solid grid that is not turned and has non-zero X and Z scale",
            path.dotted()
        )));
    }
    Ok(())
}

pub(super) fn validate_planar_map(path: &Path, transform: Transform3D) -> Result<(), RuntimeError> {
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
        return Err(RuntimeError::Host(format!(
            "{} needs a grid transform that stays in world XY and has non-zero XY scale",
            path.dotted()
        )));
    }
    Ok(())
}

pub(super) fn map_to_world(transform: Transform3D, point: PlanePoint) -> [f32; 2] {
    let (sin, cos) = transform.rotation_z_radians().sin_cos();
    let x = as_f32(point.x) * transform.scale[0];
    let y = as_f32(point.y) * transform.scale[1];
    [
        transform.position[0] + cos * x - sin * y,
        transform.position[1] + sin * x + cos * y,
    ]
}

pub(super) fn world_to_map(transform: Transform3D, point: [f32; 2]) -> PlanePoint {
    let (sin, cos) = transform.rotation_z_radians().sin_cos();
    let x = point[0] - transform.position[0];
    let y = point[1] - transform.position[1];
    PlanePoint::new(
        f64::from((cos * x + sin * y) / transform.scale[0]),
        f64::from((-sin * x + cos * y) / transform.scale[1]),
    )
}

impl WorldHost<'_> {
    /// Where a cell is, read from whichever grid component the entity carries.
    ///
    /// The layout convention matches the `GridSpace` `sindri-scene` exposes,
    /// arrived at without taking a dependency on that render-facing crate. An
    /// integration test in the companion game holds the two adapters to the
    /// same answer.
    pub(super) fn map_grid(&self, path: &Path, map: EntityId) -> Result<MapGrid, RuntimeError> {
        let data = self.world.get(map).ok_or_else(|| {
            RuntimeError::Host(format!("{}'s grid no longer exists", path.dotted()))
        })?;
        let (geometry, _) = geometry::read(path, data)?;
        let transform = data.transform_3d.unwrap_or_default();
        match geometry.solid {
            Some(_) => validate_solid_map(path, transform)?,
            None => validate_planar_map(path, transform)?,
        }
        Ok(MapGrid {
            space: geometry.space,
            transform,
            solid: geometry.solid,
        })
    }

    pub(super) fn path_to_target(
        &self,
        path: &Path,
        entity: EntityId,
        map: EntityId,
        target: EntityId,
        grid: MapGrid,
    ) -> Result<Option<Vec<GridCoord>>, RuntimeError> {
        // With tile sets, a stacked floor's holes and walls are part of the
        // answer; without them, only what the scene authored is. A host that
        // binds none is one where nothing could draw the volume either.
        let navigation = match self.tile_sets {
            Some(tile_sets) => {
                WorldGridNavigation::from_world_with_tile_sets(self.world, map, tile_sets)
            }
            None => WorldGridNavigation::from_world(self.world, map),
        }
        .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        let target_world = self
            .transform_of(target)
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{} needs the target to have a transform",
                    path.dotted()
                ))
            })?
            .position;
        let goal = grid.cell_at(path, target_world)?;
        navigation
            .find_path(GridPathfinder::default(), entity, goal)
            .map(|route| route.map(sindri_grid::GridPath::into_nodes))
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))
    }

    /// The `Grid.*` calls that name an entity and the map it stands on.
    ///
    /// Here rather than beside the world calls because everything it needs --
    /// the projection, the transform, the pathfinder -- is in this file, and it
    /// was the one thing in `call.rs` that had nothing to do with the rest of
    /// it.
    pub(super) fn grid_call(
        &mut self,
        call: GridCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        debug_assert!(
            !call.is_about_a_cell(),
            "cell calls are dispatched to `tile_call` before this"
        );
        let entity = self.entity_argument(path, args, 0, "the positioned object")?;
        let map = self.entity_argument(path, args, 1, "the grid")?;
        let grid = self.map_grid(path, map)?;

        match call {
            GridCall::PositionX | GridCall::PositionY => {
                let world = self
                    .transform_of(entity)
                    .ok_or_else(|| {
                        RuntimeError::Host(format!(
                            "{} needs the positioned object to have a transform",
                            path.dotted()
                        ))
                    })?
                    .position;
                let point = grid.to_grid(path, world)?;
                Ok(Value::Number(match call {
                    GridCall::PositionX => point.x,
                    GridCall::PositionY => point.y,
                    _ => unreachable!("matched above"),
                }))
            }
            GridCall::Place => {
                let x = number(
                    path,
                    args.get(2).ok_or_else(|| {
                        RuntimeError::Host(format!("{} needs a grid X", path.dotted()))
                    })?,
                )?;
                let y = number(
                    path,
                    args.get(3).ok_or_else(|| {
                        RuntimeError::Host(format!("{} needs a grid Y", path.dotted()))
                    })?,
                )?;
                let Some(mut transform) = self.transform_of(entity) else {
                    return Err(RuntimeError::Host(format!(
                        "{} needs the positioned object to have a transform",
                        path.dotted()
                    )));
                };
                transform.position =
                    grid.to_world(path, GridPoint::new(x, y), transform.position)?;
                let Some(data) = self.world.get_mut(entity) else {
                    return Err(RuntimeError::Host(format!(
                        "{}'s positioned object no longer exists",
                        path.dotted()
                    )));
                };
                data.transform_3d = Some(transform);
                Ok(Value::Unit)
            }
            GridCall::CanReach | GridCall::StepToward => {
                let target = self.entity_argument(path, args, 2, "the path target")?;
                let route = self.path_to_target(path, entity, map, target, grid)?;
                if call == GridCall::CanReach {
                    return Ok(Value::Bool(route.is_some()));
                }
                let Some(next) = route.as_deref().and_then(|nodes| nodes.get(1)).copied() else {
                    return Ok(Value::Bool(false));
                };
                let Some(mut transform) = self.transform_of(entity) else {
                    return Err(RuntimeError::Host(format!(
                        "{} needs the moving occupant to have a transform",
                        path.dotted()
                    )));
                };
                transform.position = grid.to_world(path, next.into(), transform.position)?;
                self.world
                    .get_mut(entity)
                    .expect("entity argument was validated above")
                    .transform_3d = Some(transform);
                Ok(Value::Bool(true))
            }
            // Dispatched to `tile_call`, which is the only other reader of
            // this namespace and takes a map and a cell instead.
            _ => unreachable!("a cell call never reaches here"),
        }
    }
}
