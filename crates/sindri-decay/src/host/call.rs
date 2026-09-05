//! Performing a call a script made.
//!
//! One function per host type, matching the lists in
//! [`crate::surface::call`]. A new call is an arm here and an entry
//! there.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{EntityId, SceneComponent};
use sindri_grid::GridPoint;
use sindri_scene::ShapeComponent;

use sindri_core::TagsComponent;

use crate::ScriptComponent;
use crate::surface::{GridCall, WorldCall};

/// How many entities one query may answer with.
///
/// A query walks the world, so its cost is the world's size whatever the
/// answer; this bounds what a script then holds and walks. It is far past a
/// dense combat frame and far short of a number that would make a per-entity
/// loop over the result a frame's worth of work on its own. Exceeding it is
/// refused rather than truncated: a query that quietly returned the first
/// eight thousand enemies would be a game that quietly stopped hitting some of
/// them.
const QUERY_LIMIT: usize = 8192;
const SHAPE_POINT_LIMIT: usize = 8;

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
            WorldCall::SetShapePoint => self.set_shape_point_call(path, args),
            WorldCall::SetProperty => self.set_property_call(path, args),
            WorldCall::PropertyNumber => self.property_number_call(path, args),
            WorldCall::WithTag => self.with_tag_call(path, args),
            WorldCall::HasTag => self.has_tag_call(path, args),
            WorldCall::SetActive => {
                let entity = self.entity_argument(path, args, 0, "the entity to switch")?;
                let Some(Value::Bool(active)) = args.get(1) else {
                    return Err(RuntimeError::Host(format!(
                        "{} takes an entity and whether it is on",
                        path.dotted()
                    )));
                };
                // Written on the entity itself rather than down through its
                // children, which is what makes switching a screen back on
                // restore the children that were off on their own account.
                let data = self.world.get_mut(entity).ok_or_else(|| {
                    RuntimeError::Host(format!("{}: that entity is gone", path.dotted()))
                })?;
                data.disabled = !*active;
                Ok(Value::Unit)
            }
            WorldCall::IsActive => {
                let entity = self.entity_argument(path, args, 0, "the entity to ask about")?;
                Ok(Value::Bool(self.world.is_active(entity)))
            }
        }
    }

    /// Every active entity carrying `tag`, in world order.
    fn with_tag_call(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let Some(Value::String(tag)) = args.first() else {
            return Err(RuntimeError::Host(format!(
                "{} names a tag, as text",
                path.dotted()
            )));
        };

        let mut found = Vec::new();
        for (entity, data) in self.world.entities() {
            // Active is the filter every other walk of the world uses: an
            // entity switched off — or whose parent is — takes no part in
            // rendering, stepping, scripting or picking, and a query that
            // answered with one would be the odd one out.
            if !self.world.is_active(entity) {
                continue;
            }
            let Some(payload) = data.components.get(TagsComponent::TYPE_NAME) else {
                continue;
            };
            // Decoded rather than pattern-matched against the raw JSON: a
            // payload that is not a set of tags is an authoring mistake worth
            // naming, and reading it by hand would silently skip it.
            let tags: TagsComponent = serde_json::from_value(payload.clone()).map_err(|error| {
                RuntimeError::Host(format!(
                    "{}: an entity's {} could not be read: {error}",
                    path.dotted(),
                    TagsComponent::TYPE_NAME
                ))
            })?;
            if !tags.has(tag) {
                continue;
            }
            if found.len() == QUERY_LIMIT {
                return Err(RuntimeError::Host(format!(
                    "{} found more than {QUERY_LIMIT} entities tagged '{tag}'",
                    path.dotted()
                )));
            }
            found.push(Value::Reference(entity.to_bits()));
        }
        Ok(Value::array(found))
    }

    fn has_tag_call(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let entity = self.entity_argument(path, args, 0, "the entity to inspect")?;
        let Some(Value::String(tag)) = args.get(1) else {
            return Err(RuntimeError::Host(format!(
                "{} names a tag, as text",
                path.dotted()
            )));
        };
        if !self.world.is_active(entity) {
            return Ok(Value::Bool(false));
        }
        let Some(data) = self.world.get(entity) else {
            return Ok(Value::Bool(false));
        };
        let Some(payload) = data.components.get(TagsComponent::TYPE_NAME) else {
            return Ok(Value::Bool(false));
        };
        let tags: TagsComponent = serde_json::from_value(payload.clone()).map_err(|error| {
            RuntimeError::Host(format!(
                "{}: the entity's {} could not be read: {error}",
                path.dotted(),
                TagsComponent::TYPE_NAME
            ))
        })?;
        Ok(Value::Bool(tags.has(tag)))
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

    /// Writes one vertex of the current script entity's world-space polygon.
    fn set_shape_point_call(
        &mut self,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let index = number(
            path,
            args.first().ok_or_else(|| {
                RuntimeError::Host(format!("{} needs a point index", path.dotted()))
            })?,
        )?;
        if index.fract() != 0.0 || !(0.0..SHAPE_POINT_LIMIT as f64).contains(&index) {
            return Err(RuntimeError::Host(format!(
                "{} needs a whole point index from 0 through {}",
                path.dotted(),
                SHAPE_POINT_LIMIT - 1
            )));
        }
        let x = number(
            path,
            args.get(1)
                .ok_or_else(|| RuntimeError::Host(format!("{} needs X", path.dotted())))?,
        )?;
        let y = number(
            path,
            args.get(2)
                .ok_or_else(|| RuntimeError::Host(format!("{} needs Y", path.dotted())))?,
        )?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = index as usize;
        let data = self.world.get_mut(self.entity).ok_or_else(|| {
            RuntimeError::Host(format!("{}'s entity no longer exists", path.dotted()))
        })?;
        let payload = data
            .components
            .get_mut(ShapeComponent::TYPE_NAME)
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{} needs a {} on the script entity",
                    path.dotted(),
                    ShapeComponent::TYPE_NAME
                ))
            })?;
        let object = payload.as_object_mut().ok_or_else(|| {
            RuntimeError::Host(format!(
                "{}'s {} is not an object",
                path.dotted(),
                ShapeComponent::TYPE_NAME
            ))
        })?;
        let points = object
            .entry("points")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{}'s {} points are not an array",
                    path.dotted(),
                    ShapeComponent::TYPE_NAME
                ))
            })?;
        while points.len() <= index {
            points.push(serde_json::json!([0.0, 0.0]));
        }
        points[index] = serde_json::json!([x, y]);
        Ok(Value::Unit)
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

    /// Reads one numeric value from a script component's authored properties.
    fn property_number_call(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let entity = self.entity_argument(path, args, 0, "the entity to inspect")?;
        let Some(Value::String(property)) = args.get(1) else {
            return Err(RuntimeError::Host(format!(
                "{} names the property, as text",
                path.dotted()
            )));
        };
        let Some(Value::Number(fallback)) = args.get(2) else {
            return Err(RuntimeError::Host(format!(
                "{} takes a numeric fallback",
                path.dotted()
            )));
        };
        let Some(data) = self.world.get(entity) else {
            return Err(RuntimeError::Host(format!(
                "{}'s entity no longer exists",
                path.dotted()
            )));
        };
        let value = data
            .components
            .get(ScriptComponent::TYPE_NAME)
            .and_then(|payload| payload.get("properties"))
            .and_then(|properties| properties.get(property))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(*fallback);
        Ok(Value::Number(value))
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
