//! What a Decay script can reach, and nothing else.
//!
//! Decay knows paths, not engine concepts: `this.transform.position.x` reaches
//! the runtime as four strings the IR never interprets. This module is the only
//! place that gives those strings a meaning, which is what keeps the language
//! replaceable — swapping Decay for something else costs this file and the
//! syntax, not the architecture.
//!
//! The surface is deliberately small. Everything a script can touch is listed
//! here in one table each for loads, stores, and calls, so "what can a script
//! do" is answerable by reading rather than by grepping. Widening it is a
//! decision, and it should look like one.

use decay_ir::Path;
use decay_runtime::{Host, RuntimeError, Value};
use sindri_core::{EntityId, Transform3D, World};

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

impl Host for WorldHost<'_> {
    fn load(&mut self, path: &Path) -> Result<Option<Value>, RuntimeError> {
        let parts: Vec<&str> = path.0.iter().map(String::as_str).collect();
        let Some(transform) = self.transform() else {
            // Not an error: a script on an entity with no transform simply
            // cannot see one, and `load` returning None is how the runtime
            // says "unknown path" with the name attached.
            return Ok(None);
        };
        Ok(match parts.as_slice() {
            ["this", "transform", "position", axis] => {
                axis_index(axis).map(|index| Value::Number(f64::from(transform.position[index])))
            }
            ["this", "transform", "scale", axis] => {
                axis_index(axis).map(|index| Value::Number(f64::from(transform.scale[index])))
            }
            // The 2D rotation, because a quaternion is not something a gameplay
            // script should be asked to assemble by hand. There is deliberately
            // no way to read or write the other two axes yet: offering a third
            // of a 3D rotation API is worse than offering none.
            ["this", "transform", "rotation_z"] => {
                Some(Value::Number(f64::from(transform.rotation_z_radians())))
            }
            _ => None,
        })
    }

    fn store(&mut self, path: &Path, value: Value) -> Result<bool, RuntimeError> {
        let parts: Vec<&str> = path.0.iter().map(String::as_str).collect();
        // Decay's only numeric type is spelled `f32` and holds an `f64`; every
        // engine transform is `f32`. This is the one place the two meet, so it
        // is the one place that narrows -- and narrowing is the intent rather
        // than a hazard. `docs/decay-direction.md` records that the language's
        // numeric type is a decision still owed an answer.
        #[allow(clippy::cast_possible_truncation)]
        let number = |value: &Value| match value {
            Value::Number(number) => Ok(*number as f32),
            other => Err(RuntimeError::Host(format!(
                "{} takes a number, and the script gave {other:?}",
                path.dotted()
            ))),
        };

        let Some(mut transform) = self.transform() else {
            return Ok(false);
        };
        let written = match parts.as_slice() {
            ["this", "transform", "position", axis] => axis_index(axis)
                .map(|index| {
                    transform.position[index] = number(&value)?;
                    Ok::<_, RuntimeError>(())
                })
                .transpose()?
                .is_some(),
            ["this", "transform", "scale", axis] => axis_index(axis)
                .map(|index| {
                    transform.scale[index] = number(&value)?;
                    Ok::<_, RuntimeError>(())
                })
                .transpose()?
                .is_some(),
            ["this", "transform", "rotation_z"] => {
                transform.set_rotation_z_radians(number(&value)?);
                true
            }
            _ => false,
        };
        if !written {
            return Ok(false);
        }

        // The Z lock is an invariant of the transform, not a rule the editor
        // enforces on the way in. A script is a write path like any other, and
        // one that could ignore the lock would be the hole that makes the lock
        // worthless.
        let current = self.transform();
        if current.is_some_and(|current| current.z_lock_rejects(Some(transform))) {
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
        let number = |index: usize| -> Result<f64, RuntimeError> {
            match args.get(index) {
                Some(Value::Number(number)) => Ok(*number),
                other => Err(RuntimeError::Host(format!(
                    "{} takes numbers, and argument {index} was {other:?}",
                    path.dotted()
                ))),
            }
        };
        // A deliberately tiny standard library: enough for a script to move
        // something along a curve, and no more. Decay has no `math` module and
        // no imports, so every one of these is a bare global name, and each one
        // added is a name a script can no longer use for its own.
        let value = match path.dotted().as_str() {
            "abs" => number(0)?.abs(),
            "sqrt" => number(0)?.sqrt(),
            "sin" => number(0)?.sin(),
            "cos" => number(0)?.cos(),
            "min" => number(0)?.min(number(1)?),
            "max" => number(0)?.max(number(1)?),
            _ => return Ok(None),
        };
        Ok(Some(Value::Number(value)))
    }
}

fn axis_index(axis: &str) -> Option<usize> {
    match axis {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    }
}
