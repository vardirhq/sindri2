//! Performing a call a script made.
//!
//! One function per host type, matching the lists in
//! [`crate::surface::call`]. A new call is an arm here and an entry
//! there.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{EntityId, SceneComponent};
use sindri_grid::GridPoint;

use crate::ScriptComponent;
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
            WorldCall::Spawn => self.spawn_call(path, args),
            WorldCall::SetParent => {
                let child = self.entity_argument(path, args, 0, "the entity to move")?;
                let parent = match args.get(1) {
                    Some(Value::Reference(_)) => {
                        Some(self.entity_argument(path, args, 1, "the new parent")?)
                    }
                    // `null` is how a script says "at the root", which is the
                    // only other place an entity can be.
                    Some(Value::Null) | None => None,
                    Some(other) => {
                        return Err(RuntimeError::Host(format!(
                            "{} takes an entity or null as the new parent, and the \
                             script gave {other:?}",
                            path.dotted()
                        )));
                    }
                };
                self.world
                    .set_parent(child, parent)
                    .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
                Ok(Value::Unit)
            }
            WorldCall::SetProperty => self.set_property_call(path, args),
        }
    }

    /// Creates what a prefab describes, and answers with its root.
    fn spawn_call(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let id = match args.first() {
            // A `Prefab` value is the asset ID the scene authored into an
            // `@export` field. Decay never sees it as text, and the script that
            // holds it cannot have built it.
            Some(Value::String(id)) => id,
            // The ordinary way to reach this: an exported prefab field the
            // scene never filled in. Named as what it is, rather than reported
            // as a missing asset called "null".
            Some(Value::Null) | None => {
                return Err(RuntimeError::Host(format!(
                    "{} was given no prefab; the entity's script has an exported \
                     prefab field that the scene has not authored",
                    path.dotted()
                )));
            }
            Some(other) => {
                return Err(RuntimeError::Host(format!(
                    "{} takes an authored prefab, and the script gave {other:?}",
                    path.dotted()
                )));
            }
        };
        let Some(prefab) = self.spawning.prefabs.get(id) else {
            // Named rather than answered with null. A spawn that silently
            // produced no entity is a bug report nobody can reproduce, and a
            // mistyped asset ID is the ordinary way to reach this.
            return Err(RuntimeError::Host(format!(
                "{} names the prefab '{id}', which this project has not loaded",
                path.dotted()
            )));
        };
        if self.spawning.spawned.len() + prefab.entities.len() > crate::SPAWN_LIMIT_PER_PASS {
            return Err(RuntimeError::Host(format!(
                "{} would take this pass past {} spawned entities",
                path.dotted(),
                crate::SPAWN_LIMIT_PER_PASS
            )));
        }

        let prefab = prefab.clone();
        let created = self
            .world
            .spawn_prefab(&prefab)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
        self.spawning
            .spawned
            .extend(created.entities.iter().copied());
        Ok(Value::Reference(created.root.to_bits()))
    }

    /// Authors one `@export` property on an entity whose script has not started.
    fn set_property_call(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let entity = self.entity_argument(path, args, 0, "the entity to author")?;
        let Some(Value::String(property)) = args.get(1) else {
            return Err(RuntimeError::Host(format!(
                "{} names the property, as text",
                path.dotted()
            )));
        };
        if self.spawning.started.contains(&entity) {
            return Err(RuntimeError::Host(format!(
                "{} cannot author '{property}': that entity's script has already \
                 started, and properties are applied when an instance is built",
                path.dotted()
            )));
        }
        let value = match args.get(2) {
            Some(Value::Number(number)) => serde_json::Value::from(*number),
            Some(Value::Bool(value)) => serde_json::Value::from(*value),
            Some(Value::String(text)) => serde_json::Value::from(text.clone()),
            other => {
                return Err(RuntimeError::Host(format!(
                    "{} authors a number, a truth, or text, and the script gave \
                     {other:?}",
                    path.dotted()
                )));
            }
        };

        let Some(data) = self.world.get_mut(entity) else {
            return Err(RuntimeError::Host(format!(
                "{}'s entity no longer exists",
                path.dotted()
            )));
        };
        let Some(payload) = data.components.get_mut(ScriptComponent::TYPE_NAME) else {
            return Err(RuntimeError::Host(format!(
                "{} needs a {} on that entity, and it has none",
                path.dotted(),
                ScriptComponent::TYPE_NAME
            )));
        };
        // The payload is written rather than the typed view, for the reason
        // every other component write here is: a view is `Deserialize`-only and
        // rebuilding one drops the fields it does not know about.
        let Some(properties) = payload
            .as_object_mut()
            .map(|object| {
                object
                    .entry("properties")
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
            })
            .and_then(serde_json::Value::as_object_mut)
        else {
            return Err(RuntimeError::Host(format!(
                "{}'s script component does not hold properties",
                path.dotted()
            )));
        };
        properties.insert(property.clone(), value);
        Ok(Value::Unit)
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
