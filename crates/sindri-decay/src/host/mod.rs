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
mod dispatch;
mod effects;
mod map;
mod physics;
mod random;
mod save;
mod ui;

use std::collections::BTreeSet;

use decay_ir::Path;
use decay_runtime::{Host, RuntimeError, Value};
use sindri_core::{EntityId, Transform3D, World};
use sindri_platform::InputState;

use self::convert::{as_f32, describe, number};
use crate::{
    Blackboard, PrefabSources,
    surface::{
        FUNCTIONS, Handle, HostFunction, Leaf, POINTER, POINTER_VALUES, PRINT, PointerValue, STICK,
        STICK_VALUES, StickValue, TIME, TIME_VALUES, TOUCH, TOUCH_COUNT, TimeValue, TouchCall,
        VIEWPORT, VIEWPORT_VALUES, ViewportValue, follow_mut, handle, leaf, leaf_through_reference,
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
    /// The physics a script may read and drive, when the host runs any.
    physics: Option<crate::Physics2d<'a>>,
    /// What the game remembers, when the host is keeping a save.
    saves: Option<&'a mut sindri_core::SaveStore>,
    /// The fleck pool, when the host is running one.
    effects: Option<&'a mut sindri_scene::Effects2d>,
    /// The run's random stream, when the host is running one.
    ///
    /// Mutable because drawing a number is what advances it: a stream a script
    /// could read without moving would hand out the same number for ever.
    random: Option<&'a mut sindri_core::Rng>,
    /// Where the screen elements are and what the pointer is doing to them.
    ///
    /// Read-only: hover and click are answers about this frame, computed by the
    /// host before scripts ran. A script that could change them would be
    /// deciding what the person did.
    screen_ui: Option<&'a sindri_scene::ScreenUi>,
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

        // Where the person is pointing, and how many fingers are down. Facts
        // about the frame like `Time.delta`, and never about a subject: a
        // reference cannot be asked where the mouse is.
        if subject.is_none()
            && let [namespace, name] = parts.as_slice()
        {
            if *namespace == POINTER
                && let Some((_, value)) = POINTER_VALUES.iter().find(|(known, _)| known == name)
            {
                return Ok(Some(self.pointer_value(*value)));
            }
            if *namespace == STICK
                && let Some((_, value)) = STICK_VALUES.iter().find(|(known, _)| known == name)
            {
                return Ok(Some(self.stick_value(*value)));
            }
            if *namespace == TOUCH && *name == TOUCH_COUNT {
                // `usize` to `f64` is exact for every count a hand can produce,
                // and the platform bounds it to ten regardless.
                #[allow(clippy::cast_precision_loss)]
                return Ok(Some(Value::Number(self.context.input.touch_count() as f64)));
            }
            if *namespace == VIEWPORT
                && let Some((_, value)) = VIEWPORT_VALUES.iter().find(|(known, _)| known == name)
            {
                return Ok(Some(Value::Number(f64::from(match value {
                    ViewportValue::Aspect => self
                        .screen_ui
                        .map_or(1.0, sindri_scene::ScreenUi::viewport_aspect),
                }))));
            }
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

        if let Some(value) = self.bare_call(&parts, path, args)? {
            return Ok(Some(value));
        }

        if let [namespace, name] = parts.as_slice()
            && let Some(result) = self.namespaced_call(namespace, name, path, args)
        {
            return result.map(Some);
        }

        Ok(None)
    }
}

/// Everything a script can reach beyond the world and the frame.
///
/// Bundled because the list had reached eight and every capability the scripting
/// surface grows adds another. `crate::HostServices` is this plus the audio
/// queue, which is the wrapper's business rather than this host's.
pub struct WorldServices<'a> {
    pub spawning: Spawning<'a>,
    /// What the game remembers, when the host is keeping a save.
    pub saves: Option<&'a mut sindri_core::SaveStore>,
    /// The fleck pool, when the host is running one.
    pub effects: Option<&'a mut sindri_scene::Effects2d>,
    pub physics: Option<crate::Physics2d<'a>>,
    pub screen_ui: Option<&'a sindri_scene::ScreenUi>,
    pub random: Option<&'a mut sindri_core::Rng>,
}

impl<'a> WorldHost<'a> {
    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
        services: WorldServices<'a>,
    ) -> Self {
        let WorldServices {
            spawning,
            saves,
            effects,
            physics,
            screen_ui,
            random,
        } = services;
        Self {
            world,
            entity,
            context,
            blackboard,
            spawning,
            physics,
            screen_ui,
            random,
            saves,
            effects,
            printed: Vec::new(),
        }
    }

    /// Everything the script printed during the call.
    pub fn take_printed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.printed)
    }

    /// `print` and the maths, which are the only calls with no namespace.
    ///
    /// Their own function because the dispatch that follows them is one arm per
    /// namespace, and a reader looking for `World.spawn` should not have to
    /// scroll past the argument handling for `min`.
    fn bare_call(
        &mut self,
        parts: &[&str],
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        let [name] = parts else {
            return Ok(None);
        };
        {
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
                    // How many, not what: printing a collection of two
                    // thousand entities into the console is not what anyone
                    // reaching for `print` wanted, and the elements are
                    // reachable one at a time anyway.
                    Some(Value::Array(values)) => format!("{} entries", values.len()),
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
        Ok(None)
    }

    /// One of the numbers a script reads about the pointer.
    ///
    /// A pointer that is not there reads as zero rather than as an error,
    /// because "the mouse left the window" is an ordinary thing that happens
    /// mid-frame and not a mistake in a script. `Pointer.inside` is how a
    /// script that cares asks, and it has to be asked *before* the position is
    /// believed.
    /// What the steering finger is asking for.
    ///
    /// The host computes it rather than the script, because anchoring, the
    /// clamp past the radius and the dead zone are the same three decisions in
    /// every game that has ever needed a stick -- and a script doing the
    /// subtraction itself gets a slightly different feel and its own bugs.
    fn stick_value(&self, value: StickValue) -> Value {
        let stick = self.context.input.stick();
        let pushed = stick.value();
        match value {
            StickValue::X => Value::Number(f64::from(pushed[0])),
            StickValue::Y => Value::Number(f64::from(pushed[1])),
            StickValue::Held => Value::Bool(stick.is_engaged()),
            // Zero when nothing is holding it, like a pointer position read
            // from outside the window: a script that cares asks `held` first.
            StickValue::AnchorX => Value::Number(f64::from(
                stick
                    .anchor(self.context.input.presses())
                    .unwrap_or([0.0, 0.0])[0],
            )),
            StickValue::AnchorY => Value::Number(f64::from(
                stick
                    .anchor(self.context.input.presses())
                    .unwrap_or([0.0, 0.0])[1],
            )),
        }
    }

    fn pointer_value(&self, value: PointerValue) -> Value {
        let position = self.context.input.pointer_position();
        match value {
            PointerValue::Inside => Value::Bool(position.is_some()),
            // False with no screen UI running, rather than an error: a host
            // with no UI has no element to take the pointer, which is a true
            // answer rather than a missing one.
            PointerValue::OverUi => Value::Bool(
                self.screen_ui
                    .is_some_and(sindri_scene::ScreenUi::captures_pointer),
            ),
            PointerValue::X => Value::Number(f64::from(position.unwrap_or([0.0, 0.0])[0])),
            PointerValue::Y => Value::Number(f64::from(position.unwrap_or([0.0, 0.0])[1])),
            // Zero with no screen UI running, for the same reason a position
            // read while the pointer is outside reads zero: the overlay is
            // where the UI is laid out, and a host laying out none has no
            // overlay to answer about. A script that cares asks `inside`.
            PointerValue::OverlayX => Value::Number(f64::from(
                self.screen_ui
                    .and_then(sindri_scene::ScreenUi::pointer_overlay)
                    .unwrap_or([0.0, 0.0])[0],
            )),
            PointerValue::OverlayY => Value::Number(f64::from(
                self.screen_ui
                    .and_then(sindri_scene::ScreenUi::pointer_overlay)
                    .unwrap_or([0.0, 0.0])[1],
            )),
        }
    }

    /// Where one finger is.
    fn touch_call(
        &self,
        call: TouchCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let index = number(path, args.first().unwrap_or(&Value::Null))?;
        if !index.is_finite() || index.fract() != 0.0 || index < 0.0 {
            return Err(RuntimeError::Host(format!(
                "{} takes which finger, counting from zero, and the script gave {index}",
                path.dotted()
            )));
        }
        // Guarded above: finite, non-negative, and whole.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let position = self.context.input.touch_at(index as usize).ok_or_else(|| {
            // Named rather than answered with zero: a script reading finger
            // three when two are down has a bound that is wrong, and a zero
            // would read as a finger in the corner of the screen.
            RuntimeError::Host(format!(
                "{} was asked for finger {index}, and {} are down",
                path.dotted(),
                self.context.input.touch_count()
            ))
        })?;
        Ok(Value::Number(f64::from(match call {
            TouchCall::X => position[0],
            TouchCall::Y => position[1],
        })))
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
