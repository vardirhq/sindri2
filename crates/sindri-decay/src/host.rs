//! What a Decay script can reach, and nothing else.
//!
//! Decay knows paths, not engine concepts: `this.transform.position.x` reaches
//! the runtime as four strings the IR never interprets. This is the only place
//! that gives those strings a meaning, which is what keeps the language
//! replaceable — swapping Decay for something else costs this file and the
//! syntax, not the architecture.
//!
//! What the surface *is* lives in [`crate::surface`], and both this and the
//! analyzer's view of it are derived from there. Nothing in this file decides
//! which paths exist; it decides only how to reach one.

use decay_ir::Path;
use decay_runtime::{Host, RuntimeError, Value};
use sindri_core::{EntityId, Transform3D, World};

use crate::surface::{AXES, FUNCTIONS, HostFunction, SCALARS, TRANSFORM_MEMBER, VECTORS, Vector};

/// The world, seen through one entity's script.
///
/// Borrowed for the length of a single script call rather than held, because a
/// script may write to the transform and the world cannot be lent out twice.
pub struct WorldHost<'a> {
    world: &'a mut World,
    entity: EntityId,
}

impl<'a> WorldHost<'a> {
    pub fn new(world: &'a mut World, entity: EntityId) -> Self {
        Self { world, entity }
    }

    fn transform(&self) -> Option<Transform3D> {
        self.world.get(self.entity)?.transform_3d
    }
}

/// Which part of a transform a path names, or `None` if it names none.
enum Target {
    Axis(Vector, usize),
    Scalar(crate::surface::Scalar),
}

fn target(path: &Path) -> Option<Target> {
    let parts: Vec<&str> = path.0.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["this", member, name, axis] if *member == TRANSFORM_MEMBER => {
            let vector = VECTORS.iter().find(|(known, _)| known == name)?.1;
            let index = AXES.iter().find(|(known, _)| known == axis)?.1;
            Some(Target::Axis(vector, index))
        }
        ["this", member, name] if *member == TRANSFORM_MEMBER => SCALARS
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, scalar)| Target::Scalar(*scalar)),
        _ => None,
    }
}

/// Decay's only numeric type is spelled `f32` and holds an `f64`; every engine
/// transform is `f32`. This is the one place the two meet, so it is the one
/// place that narrows, and narrowing is the intent rather than a hazard.
/// `docs/decay-direction.md` records that the numeric type is a decision still
/// owed an answer.
#[allow(clippy::cast_possible_truncation)]
fn number(path: &Path, value: &Value) -> Result<f32, RuntimeError> {
    match value {
        Value::Number(number) => Ok(*number as f32),
        other => Err(RuntimeError::Host(format!(
            "{} takes a number, and the script gave {other:?}",
            path.dotted()
        ))),
    }
}

impl Host for WorldHost<'_> {
    fn load(&mut self, path: &Path) -> Result<Option<Value>, RuntimeError> {
        // Not an error when there is no transform: a script on an entity
        // without one simply cannot see it, and `None` is how the runtime says
        // "unknown path" with the name attached.
        let Some(transform) = self.transform() else {
            return Ok(None);
        };
        Ok(target(path).map(|target| {
            Value::Number(f64::from(match target {
                Target::Axis(vector, index) => vector.get(&transform)[index],
                Target::Scalar(scalar) => scalar.get(&transform),
            }))
        }))
    }

    fn store(&mut self, path: &Path, value: Value) -> Result<bool, RuntimeError> {
        let (Some(mut transform), Some(target)) = (self.transform(), target(path)) else {
            return Ok(false);
        };
        let number = number(path, &value)?;
        match target {
            Target::Axis(vector, index) => {
                let mut values = vector.get(&transform);
                values[index] = number;
                vector.set(&mut transform, values);
            }
            Target::Scalar(scalar) => scalar.set(&mut transform, number),
        }

        // The Z lock is an invariant of the transform, not a rule the editor
        // enforces on the way in. A script is a write path like any other, and
        // one that could ignore the lock would be the hole that makes the lock
        // worthless.
        if self
            .transform()
            .is_some_and(|current| current.z_lock_rejects(Some(transform)))
        {
            return Err(RuntimeError::Host(format!(
                "{} would move a Z-locked transform off its layer",
                path.dotted()
            )));
        }

        let Some(data) = self.world.get_mut(self.entity) else {
            return Ok(false);
        };
        data.transform_3d = Some(transform);
        Ok(true)
    }

    fn call(&mut self, path: &Path, args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let Some((_, function)) = FUNCTIONS.iter().find(|(name, _)| *name == path.dotted()) else {
            return Ok(None);
        };
        let argument = |index: usize| -> Result<f64, RuntimeError> {
            match args.get(index) {
                Some(Value::Number(number)) => Ok(*number),
                other => Err(RuntimeError::Host(format!(
                    "{} takes numbers, and argument {index} was {other:?}",
                    path.dotted()
                ))),
            }
        };
        Ok(Some(Value::Number(match function {
            HostFunction::Unary(apply) => apply(argument(0)?),
            HostFunction::Binary(apply) => apply(argument(0)?, argument(1)?),
        })))
    }
}
