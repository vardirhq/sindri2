//! Performing a `Random.*` call.
//!
//! The host owns the stream, and a script draws from it. That ownership is the
//! point: a run's numbers come from its seed, so a run can be replayed from one,
//! and no script can reach a source of entropy the engine deliberately does not
//! have.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};

use crate::surface::RandomCall;

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    pub(super) fn random_call(
        &mut self,
        call: RandomCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        // A game whose waves never vary because nothing is seeding them should
        // hear about it on the first frame, rather than being handed the same
        // number for ever.
        let Some(rng) = self.random.as_deref_mut() else {
            return Err(RuntimeError::Host(format!(
                "{}: this host is not running a random stream",
                path.dotted()
            )));
        };
        match call {
            RandomCall::Value => Ok(Value::Number(f64::from(rng.next_f32()))),
            RandomCall::Range => {
                let low = number(path, args.first().unwrap_or(&Value::Null))?;
                let high = number(path, args.get(1).unwrap_or(&Value::Null))?;
                if high < low {
                    return Err(backwards(path, low, high));
                }
                // Interpolated rather than scaled-and-added, so that a fraction
                // of nearly one cannot land outside a span whose ends are far
                // apart.
                Ok(Value::Number(
                    f64::from(rng.next_f32()).mul_add(high - low, low),
                ))
            }
            RandomCall::Int => {
                let low = number(path, args.first().unwrap_or(&Value::Null))?;
                let high = number(path, args.get(1).unwrap_or(&Value::Null))?;
                if high < low {
                    return Err(backwards(path, low, high));
                }
                // Both ends included, so `int(1, 6)` is a die.
                let span = (high.floor() - low.ceil()) + 1.0;
                if span < 1.0 {
                    // No whole number lies between them: `int(1.2, 1.8)` has no
                    // answer, and inventing one would be a lie about the range.
                    return Err(RuntimeError::Host(format!(
                        "{}: no whole number lies between {low} and {high}",
                        path.dotted()
                    )));
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let bound = span.min(f64::from(u32::MAX)) as u32;
                Ok(Value::Number(low.ceil() + f64::from(rng.below(bound))))
            }
            RandomCall::Pick => {
                let Some(Value::Array(items)) = args.first() else {
                    return Err(RuntimeError::Host(format!(
                        "{}: pick needs a collection to choose from",
                        path.dotted()
                    )));
                };
                if items.is_empty() {
                    // A script that might be choosing from nothing guards with
                    // `if group.len > 0`, which is one line and says what it
                    // means. Answering with an entity that is not there would
                    // move the same problem one call further away.
                    return Err(RuntimeError::Host(format!(
                        "{}: nothing to pick from",
                        path.dotted()
                    )));
                }
                // A collection too long to count in 32 bits cannot happen —
                // `World.with_tag` is bounded far below that — but choosing the
                // first is a better answer than a panic if one ever does.
                let index = u32::try_from(items.len()).map_or(0, |len| rng.below(len));
                Ok(items[index as usize].clone())
            }
            RandomCall::Seed => {
                let seed = number(path, args.first().unwrap_or(&Value::Null))?;
                if !seed.is_finite() {
                    return Err(RuntimeError::Host(format!(
                        "{}: {seed} is not a seed",
                        path.dotted()
                    )));
                }
                // Through `i64` so a negative seed is a seed rather than zero:
                // a run counter that went below nothing is still a run.
                #[allow(clippy::cast_possible_truncation)]
                let bits = (seed.trunc() as i64).cast_unsigned();
                *rng = rng.reseeded(bits);
                Ok(Value::Unit)
            }
        }
    }
}

/// A span whose ends are the wrong way round is an authoring mistake, and one
/// that would otherwise show up as a spawner that never spawns anywhere.
fn backwards(path: &Path, low: f64, high: f64) -> RuntimeError {
    RuntimeError::Host(format!("{}: {low} to {high} runs backwards", path.dotted()))
}
