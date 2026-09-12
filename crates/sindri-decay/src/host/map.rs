//! A tilemap as a script sees it: logical cells, and where they are in
//! the world.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{EntityId, Transform3D};
use sindri_grid::{
    GridCoord, GridPathfinder, GridPoint, GridSpace, PlanePoint, PlaneYAxis, Projection,
};
use sindri_scene::WorldGridNavigation;

use crate::surface::{GridCall, TILEMAP_COMPONENT};

use super::WorldHost;
use super::convert::{as_f32, number};

/// The renderer-independent grid and the entity transform placing it in world
/// XY. Kept together because a logical coordinate only means a world position
/// relative to both.
#[derive(Clone, Copy)]
pub(super) struct MapGrid {
    pub(super) space: GridSpace,
    pub(super) transform: Transform3D,
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
    /// Reads the same tilemap layout convention `sindri-scene` exposes as its
    /// `GridSpace`, without taking a dependency on that render-facing crate.
    /// An integration test in the companion game holds the two adapters to the
    /// same answer.
    pub(super) fn map_grid(&self, path: &Path, map: EntityId) -> Result<MapGrid, RuntimeError> {
        let data = self.world.get(map).ok_or_else(|| {
            RuntimeError::Host(format!("{}'s grid no longer exists", path.dotted()))
        })?;
        let payload = data.components.get(TILEMAP_COMPONENT).ok_or_else(|| {
            RuntimeError::Host(format!(
                "{} needs its grid entity to carry {TILEMAP_COMPONENT}",
                path.dotted()
            ))
        })?;
        let tile_size = payload
            .get("tile_size")
            .and_then(serde_json::Value::as_array)
            .map_or(Ok([1.0, 1.0]), |size| {
                let [width, height] = size.as_slice() else {
                    return Err(RuntimeError::Host(format!(
                        "{} found a tile_size that is not two numbers",
                        path.dotted()
                    )));
                };
                let Some(width) = width.as_f64() else {
                    return Err(RuntimeError::Host(format!(
                        "{} found a tile width that is not a number",
                        path.dotted()
                    )));
                };
                let Some(height) = height.as_f64() else {
                    return Err(RuntimeError::Host(format!(
                        "{} found a tile height that is not a number",
                        path.dotted()
                    )));
                };
                Ok([width, height])
            })?;
        let projection = match payload
            .get("projection")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("orthogonal")
        {
            "orthogonal" => Projection::Orthogonal,
            "isometric" => Projection::Isometric,
            other => {
                return Err(RuntimeError::Host(format!(
                    "{} found an unknown tile projection `{other}`",
                    path.dotted()
                )));
            }
        };
        let origin = match projection {
            Projection::Orthogonal => PlanePoint::new(tile_size[0] * 0.5, -tile_size[1] * 0.5),
            Projection::Isometric => PlanePoint::default(),
        };
        let space = GridSpace::with_origin_and_y_axis(
            projection,
            tile_size[0],
            tile_size[1],
            origin,
            PlaneYAxis::Up,
        )
        .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        let transform = data.transform_3d.unwrap_or_default();
        validate_planar_map(path, transform)?;
        Ok(MapGrid { space, transform })
    }

    pub(super) fn path_to_target(
        &self,
        path: &Path,
        entity: EntityId,
        map: EntityId,
        target: EntityId,
        grid: MapGrid,
    ) -> Result<Option<Vec<GridCoord>>, RuntimeError> {
        let navigation = WorldGridNavigation::from_world(self.world, map)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        let target_world = self
            .transform_of(target)
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{} needs the target to have a transform",
                    path.dotted()
                ))
            })?
            .position_2d();
        let local = world_to_map(grid.transform, target_world);
        let goal = navigation
            .space()
            .plane_to_grid(local)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
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
                    .position_2d();
                let local = world_to_map(grid.transform, world);
                let point = grid
                    .space
                    .unproject(local)
                    .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
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
                let local = grid
                    .space
                    .project(GridPoint::new(x, y))
                    .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
                let world = map_to_world(grid.transform, local);
                let Some(mut transform) = self.transform_of(entity) else {
                    return Err(RuntimeError::Host(format!(
                        "{} needs the positioned object to have a transform",
                        path.dotted()
                    )));
                };
                transform.set_position_2d(world);
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
                let local = grid
                    .space
                    .grid_to_plane(next)
                    .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
                let world = map_to_world(grid.transform, local);
                let Some(mut transform) = self.transform_of(entity) else {
                    return Err(RuntimeError::Host(format!(
                        "{} needs the moving occupant to have a transform",
                        path.dotted()
                    )));
                };
                transform.set_position_2d(world);
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
