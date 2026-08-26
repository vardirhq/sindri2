//! Performing a call a script made.
//!
//! One function per host type, matching the lists in
//! [`crate::surface::call`]. A new call is an arm here and an entry
//! there.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::EntityId;
use sindri_grid::GridPoint;

use crate::surface::{GridCall, WorldCall};

use super::WorldHost;
use super::convert::number;
use super::map::{map_to_world, world_to_map};

impl WorldHost<'_> {
    /// Performs one of the `World.*` calls.
    ///
    /// Its own method rather than another arm of [`Host::call`], because it is
    /// the only namespace whose arguments are references and so the only one
    /// with anything to say about them.
    pub(super) fn world_call(
        &mut self,
        call: WorldCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        match call {
            WorldCall::Find => {
                let Some(Value::String(name)) = args.first() else {
                    return Err(RuntimeError::Host(format!(
                        "{} looks an entity up by name, as text",
                        path.dotted()
                    )));
                };
                Ok(self
                    .find_named(name)
                    .map_or(Value::Null, |entity| Value::Reference(entity.to_bits())))
            }
            WorldCall::Exists => Ok(Value::Bool(match args.first() {
                Some(Value::Reference(bits)) => {
                    self.world.get(EntityId::from_bits(*bits)).is_some()
                }
                // `null` names nothing, and asking whether nothing exists is a
                // fair question with a plain answer.
                Some(Value::Null) | None => false,
                Some(other) => {
                    return Err(RuntimeError::Host(format!(
                        "{} asks about an entity, and the script gave {other:?}",
                        path.dotted()
                    )));
                }
            })),
            WorldCall::Despawn => {
                let entity = match args.first() {
                    Some(Value::Reference(bits)) => EntityId::from_bits(*bits),
                    // Despawning nothing is a no-op rather than an error:
                    // `World.despawn(World.find("gone"))` is a reasonable thing
                    // to write.
                    Some(Value::Null) | None => return Ok(Value::Unit),
                    Some(other) => {
                        return Err(RuntimeError::Host(format!(
                            "{} removes an entity, and the script gave {other:?}",
                            path.dotted()
                        )));
                    }
                };
                self.despawn(entity, path)?;
                Ok(Value::Unit)
            }
        }
    }

    /// Removes `entity` and everything under it.
    ///
    /// Not through `WorldCommand`, and deliberately not: no write a script
    /// makes goes through one. A script's transform writes do not produce undo
    /// entries either, and play mode restores the world from the snapshot it
    /// took when Play was pressed, so a despawn that alone was undoable would
    /// be an inconsistency rather than a feature. `World::despawn_recursive` is
    /// the same removal the command performs; what is missing is the captured
    /// inverse, which nothing would consume. `ROADMAP.md` keeps the item open.
    pub(super) fn despawn(&mut self, entity: EntityId, path: &Path) -> Result<(), RuntimeError> {
        self.world.despawn_recursive(entity).map_err(|error| {
            RuntimeError::Host(format!("{} could not remove it: {error}", path.dotted()))
        })?;
        Ok(())
    }

    /// The entity a scene named `name`, or `None`.
    ///
    /// First match in world order, and the surface says so: two entities with
    /// one name is an authoring mistake the editor should catch, not something
    /// to invent a rule for here.
    pub(super) fn find_named(&self, name: &str) -> Option<EntityId> {
        self.world
            .entities()
            .find(|(_, data)| data.name.as_deref() == Some(name))
            .map(|(entity, _)| entity)
    }

    pub(super) fn entity_argument(
        &self,
        path: &Path,
        args: &[Value],
        index: usize,
        role: &str,
    ) -> Result<EntityId, RuntimeError> {
        let Some(value) = args.get(index) else {
            return Err(RuntimeError::Host(format!(
                "{} needs an entity for {role}",
                path.dotted()
            )));
        };
        let Value::Reference(bits) = value else {
            return Err(RuntimeError::Host(format!(
                "{} needs an entity for {role}, and the script gave {value:?}",
                path.dotted()
            )));
        };
        let entity = EntityId::from_bits(*bits);
        self.world.get(entity).ok_or_else(|| {
            RuntimeError::Host(format!(
                "{} was given a {role} entity that no longer exists",
                path.dotted()
            ))
        })?;
        Ok(entity)
    }

    pub(super) fn grid_call(
        &mut self,
        call: GridCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
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
                    GridCall::Place | GridCall::CanReach | GridCall::StepToward => {
                        unreachable!("matched above")
                    }
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
        }
    }
}
