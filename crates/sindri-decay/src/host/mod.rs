//! What a Decay script can reach, and nothing else.
//!
//! Decay knows paths, not engine concepts: `this.transform.position.x` reaches
//! the runtime as four strings the IR never interprets. This is the only place
//! that gives those strings a meaning, which is what keeps the language
//! replaceable — swapping Decay for something else costs this directory and
//! the syntax, not the architecture.
//!
//! What the surface *is* lives in [`crate::surface`], and both this and the
//! analyzer's view of it are derived from there. Nothing here decides which
//! paths exist; it decides only how to reach one.

mod call;
mod convert;
mod map;

use std::collections::BTreeSet;

use decay_ir::Path;
use decay_runtime::{Host, RuntimeError, Value};
use sindri_core::{EntityId, Transform3D, World};
use sindri_platform::InputState;

use self::convert::{as_f32, describe, key, number};
use crate::{
    Blackboard, PrefabSources,
    surface::{
        FUNCTIONS, GAME, GAME_CALLS, GRID, GRID_CALLS, GameCall, Handle, HostFunction, INPUT,
        INPUT_QUERIES, InputQuery, Leaf, PRINT, TIME, TIME_VALUES, TimeValue, WORLD, WORLD_CALLS,
        follow_mut, handle, leaf, leaf_through_reference,
    },
};

/// What a script can know about the frame it is running in.
///
/// Passed per call rather than held, because none of it belongs to the script:
/// the input is the host's, and the clock is the host's.
#[derive(Clone, Copy)]
pub struct ScriptContext<'a> {
    pub input: &'a InputState,
    pub delta_seconds: f32,
    pub elapsed_seconds: f32,
}

/// What `World.spawn` needs, and where what it makes is recorded.
///
/// Grouped rather than three more parameters, because they are one thing: the
/// prefabs a spawn may name, the instances that already exist, and the list the
/// runner reads back to know what to start.
pub struct Spawning<'a> {
    /// What `World.spawn` can name.
    pub prefabs: &'a PrefabSources,
    /// Which entities already have a script instance.
    ///
    /// A snapshot taken before the pass rather than the live map, which is
    /// borrowed by the runner for the length of the call. Stale only in the
    /// safe direction: an entity spawned during the pass is genuinely not
    /// running yet, because instantiating one would mean executing Decay from
    /// inside a host call.
    pub started: &'a BTreeSet<EntityId>,
    /// What this pass has created, for the runner to start.
    pub spawned: &'a mut Vec<EntityId>,
}

/// The world, seen through one entity's script.
///
/// Borrowed for the length of a single script call rather than held, because a
/// script may write to the world and the world cannot be lent out twice.
pub struct WorldHost<'a> {
    world: &'a mut World,
    entity: EntityId,
    context: ScriptContext<'a>,
    /// The notes every script in the world shares.
    blackboard: &'a mut Blackboard,
    /// What spawning needs, and what it produced.
    spawning: Spawning<'a>,
    /// What the script said, in order. Drained by the caller after the call.
    printed: Vec<String>,
}

impl Host for WorldHost<'_> {
    fn load(&mut self, subject: Option<u64>, path: &Path) -> Result<Option<Value>, RuntimeError> {
        let parts: Vec<&str> = path.0.iter().map(String::as_str).collect();

        // `Time.delta` and the like: the frame, not the world. Never about a
        // subject, so a reference cannot be asked for the time.
        if subject.is_none()
            && let [namespace, name] = parts.as_slice()
            && *namespace == TIME
        {
            return Ok(TIME_VALUES
                .iter()
                .find(|(known, _)| known == name)
                .map(|(_, value)| {
                    Value::Number(f64::from(match value {
                        TimeValue::Delta => self.context.delta_seconds,
                        TimeValue::Elapsed => self.context.elapsed_seconds,
                    }))
                }));
        }

        let Some(under) = Self::addressed(subject, path) else {
            return Ok(None);
        };

        // A reference is fetched rather than read into, so it is answered
        // before any leaf lookup: `this.entity` is not a number.
        if subject.is_none()
            && let Some(handle) = handle(&under)
        {
            return Ok(Some(match handle {
                Handle::Own => Value::Reference(self.entity.to_bits()),
            }));
        }

        let found = if subject.is_some() {
            leaf_through_reference(&under)
        } else {
            leaf(&under)
        };
        let Some(leaf) = found else {
            return Ok(None);
        };
        let entity = self.subject(subject, path)?;
        let transform = self.transform_of(entity);
        let components = self
            .world
            .get(entity)
            .map_or(serde_json::Value::Null, |data| {
                serde_json::to_value(&data.components).unwrap_or(serde_json::Value::Null)
            });

        // `None` is how the runtime says "unknown path" with the name attached,
        // and it is the right answer for an entity that has no sprite: the
        // surface says a script *may* reach one, not that every entity has one.
        Ok(leaf
            .read(transform.as_ref(), &components)
            .map(Value::Number))
    }

    fn store(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        let Some(under) = Self::addressed(subject, path) else {
            return Ok(false);
        };
        let found = if subject.is_some() {
            leaf_through_reference(&under)
        } else {
            leaf(&under)
        };
        let Some(leaf) = found else {
            return Ok(false);
        };
        let entity = self.subject(subject, path)?;
        let number = number(path, &value)?;

        match leaf {
            Leaf::TransformAxis(..) | Leaf::TransformScalar(_) => {
                let Some(mut transform) = self.transform_of(entity) else {
                    return Ok(false);
                };
                match leaf {
                    Leaf::TransformAxis(vector, index) => {
                        let mut values = vector.get(&transform);
                        values[index] = as_f32(number);
                        vector.set(&mut transform, values);
                    }
                    Leaf::TransformScalar(scalar) => {
                        scalar.set(&mut transform, as_f32(number));
                    }
                    Leaf::Component { .. } => unreachable!("matched above"),
                }

                // The Z lock is an invariant of the transform, not a rule the
                // editor enforces on the way in. A script is a write path like
                // any other, and one that could ignore the lock would be the
                // hole that makes the lock worthless.
                if self
                    .transform_of(entity)
                    .is_some_and(|current| current.z_lock_rejects(Some(transform)))
                {
                    return Err(RuntimeError::Host(format!(
                        "{} would move a Z-locked transform off its layer",
                        path.dotted()
                    )));
                }
                let Some(data) = self.world.get_mut(entity) else {
                    return Ok(false);
                };
                data.transform_3d = Some(transform);
                Ok(true)
            }
            Leaf::Component { component, pointer } => {
                let Some(data) = self.world.get_mut(entity) else {
                    return Ok(false);
                };
                let Some(payload) = data.components.get_mut(component) else {
                    return Err(RuntimeError::Host(format!(
                        "{} needs a {component} on this entity, and it has none",
                        path.dotted()
                    )));
                };
                let Some(slot) = follow_mut(payload, pointer) else {
                    return Err(RuntimeError::Host(format!(
                        "{}'s {component} has nothing at {}",
                        path.dotted(),
                        describe(pointer)
                    )));
                };
                // Written as the number the payload already held it as, so a
                // layer stays an integer and a tint channel stays a float --
                // the scene round-trips byte for byte either way.
                *slot = if slot.is_i64() || slot.is_u64() {
                    // Rounded and narrowed on purpose: the payload held an
                    // integer, and a layer that came back as `7.0` would change
                    // a scene byte for byte because a script touched it. A
                    // number too large for an i64 saturates, which is a wrong
                    // layer rather than a wrong file.
                    #[allow(clippy::cast_possible_truncation)]
                    serde_json::Value::from(number.round() as i64)
                } else {
                    serde_json::Value::from(number)
                };
                Ok(true)
            }
        }
    }

    fn call(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        // Nothing on the surface is called *through* a reference: an entity is
        // a thing to read and write, not a thing with methods. Refusing here
        // keeps `target.axis("a", "b")` from reaching `Input`.
        if subject.is_some() {
            return Ok(None);
        }
        let parts: Vec<&str> = path.0.iter().map(String::as_str).collect();

        if let [name] = parts.as_slice() {
            if *name == PRINT {
                self.printed.push(match args.first() {
                    Some(Value::String(text)) => text.clone(),
                    Some(Value::Number(number)) => format!("{number}"),
                    Some(Value::Bool(value)) => format!("{value}"),
                    Some(Value::Null) | None => "null".to_owned(),
                    Some(Value::Unit) => "unit".to_owned(),
                    // Named by what it is rather than by the number inside:
                    // the packing is the host's business, and printing it
                    // would invite a script to depend on it.
                    Some(Value::Reference(_)) => "entity".to_owned(),
                });
                return Ok(Some(Value::Unit));
            }
            if let Some((_, function)) = FUNCTIONS.iter().find(|(known, _)| known == name) {
                let argument = |index: usize| -> Result<f64, RuntimeError> {
                    args.get(index)
                        .ok_or_else(|| {
                            RuntimeError::Host(format!("{} wants more arguments", path.dotted()))
                        })
                        .and_then(|value| number(path, value))
                };
                return Ok(Some(Value::Number(match function {
                    HostFunction::Unary(apply) => apply(argument(0)?),
                    HostFunction::Binary(apply) => apply(argument(0)?, argument(1)?),
                })));
            }
        }

        if let [namespace, name] = parts.as_slice()
            && *namespace == GAME
            && let Some((_, call)) = GAME_CALLS.iter().find(|(known, _)| known == name)
        {
            let note = match args.first() {
                Some(Value::String(note)) => note.clone(),
                other => {
                    return Err(RuntimeError::Host(format!(
                        "{} names its note with text, and the script gave {other:?}",
                        path.dotted()
                    )));
                }
            };
            let value = |index: usize| -> Result<f64, RuntimeError> {
                args.get(index)
                    .ok_or_else(|| {
                        RuntimeError::Host(format!("{} wants more arguments", path.dotted()))
                    })
                    .and_then(|value| number(path, value))
            };
            return Ok(Some(match call {
                GameCall::Get => Value::Number(self.blackboard.get(&note, value(1)?)),
                GameCall::Set => {
                    self.blackboard.set(note, value(1)?);
                    Value::Unit
                }
            }));
        }

        if let [namespace, name] = parts.as_slice()
            && *namespace == WORLD
            && let Some((_, call)) = WORLD_CALLS.iter().find(|(known, _)| known == name)
        {
            return self.world_call(*call, path, args).map(Some);
        }

        if let [namespace, name] = parts.as_slice()
            && *namespace == GRID
            && let Some((_, call)) = GRID_CALLS.iter().find(|(known, _)| known == name)
        {
            return self.grid_call(*call, path, args).map(Some);
        }

        if let [namespace, name] = parts.as_slice()
            && *namespace == INPUT
            && let Some((_, query)) = INPUT_QUERIES.iter().find(|(known, _)| known == name)
        {
            let input = self.context.input;
            return Ok(Some(match query {
                InputQuery::Axis => Value::Number(f64::from(
                    input.axis(key(path, args.first())?, key(path, args.get(1))?),
                )),
                InputQuery::Down => Value::Bool(input.key_down(key(path, args.first())?)),
                InputQuery::Pressed => Value::Bool(input.key_pressed(key(path, args.first())?)),
                InputQuery::Released => Value::Bool(input.key_released(key(path, args.first())?)),
            }));
        }

        Ok(None)
    }
}

impl<'a> WorldHost<'a> {
    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        spawning: Spawning<'a>,
    ) -> Self {
        Self {
            world,
            entity,
            context,
            blackboard,
            spawning,
            printed: Vec::new(),
        }
    }

    /// Everything the script printed during the call.
    pub fn take_printed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.printed)
    }

    pub(super) fn transform_of(&self, entity: EntityId) -> Option<Transform3D> {
        self.world.get(entity)?.transform_3d
    }

    /// Which entity a call is about: the one the script runs on, or the one a
    /// reference names.
    ///
    /// A reference that no longer resolves is an error naming the path rather
    /// than a silent no-op, because a script holding a stale handle is a bug in
    /// the script and the whole point of generation checking is to say so.
    pub(super) fn subject(
        &self,
        subject: Option<u64>,
        path: &Path,
    ) -> Result<EntityId, RuntimeError> {
        let Some(bits) = subject else {
            return Ok(self.entity);
        };
        let entity = EntityId::from_bits(bits);
        if self.world.get(entity).is_some() {
            Ok(entity)
        } else {
            Err(RuntimeError::Host(format!(
                "{} is about an entity that no longer exists",
                path.dotted()
            )))
        }
    }

    /// The parts of a path that address an entity's members.
    ///
    /// With a subject the path is already rooted at it, so every part counts.
    /// Without one the path is the script's own and starts with `this`.
    pub(super) fn addressed(subject: Option<u64>, path: &Path) -> Option<Vec<&str>> {
        if subject.is_some() {
            return Some(path.0.iter().map(String::as_str).collect());
        }
        Self::under_this(path)
    }

    /// The parts of a path after `this`, when it starts with `this`.
    pub(super) fn under_this(path: &Path) -> Option<Vec<&str>> {
        let mut parts = path.0.iter().map(String::as_str);
        (parts.next()? == "this").then(|| parts.collect())
    }
}
