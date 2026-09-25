//! Answering `Gamepad`, from the pads the host has read this frame.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_platform::{GamepadAxis, GamepadButton, SLOT_LIMIT};

use crate::surface::GamepadQuery;

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    pub(super) fn gamepad_query(
        &mut self,
        query: GamepadQuery,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let pads = self.context.input.gamepads();
        let slot = || slot(path, args.first());
        let named = |kind: &str| -> Result<&str, RuntimeError> {
            match args.get(1) {
                Some(Value::String(name)) => Ok(name),
                other => Err(RuntimeError::Host(format!(
                    "{} names its {kind} with text, and the script gave {other:?}",
                    path.dotted()
                ))),
            }
        };
        // Refused rather than read as untouched, for the reason a mistyped key
        // is: a control that silently does nothing is a bug nobody can see.
        let button = || -> Result<GamepadButton, RuntimeError> {
            let name = named("button")?;
            GamepadButton::from_name(name).ok_or_else(|| {
                RuntimeError::Host(format!("there is no pad button called `{name}`"))
            })
        };
        Ok(match query {
            GamepadQuery::Joined => Value::Number(pads.joined().map_or(0.0, f64::from)),
            GamepadQuery::Left => Value::Number(pads.left().map_or(0.0, f64::from)),
            GamepadQuery::Count => Value::Number(slot_count(pads.count())),
            GamepadQuery::IsConnected => {
                let slot = slot()?;
                Value::Bool(if slot == 0 {
                    pads.connected() > 0
                } else {
                    pads.is_claimed(slot)
                })
            }
            GamepadQuery::Down => Value::Bool(pads.down(slot()?, button()?)),
            GamepadQuery::Pressed => Value::Bool(pads.pressed(slot()?, button()?)),
            GamepadQuery::Released => Value::Bool(pads.released(slot()?, button()?)),
            GamepadQuery::Axis => {
                let name = named("axis")?;
                let axis = GamepadAxis::from_name(name).ok_or_else(|| {
                    RuntimeError::Host(format!("there is no pad axis called `{name}`"))
                })?;
                Value::Number(f64::from(pads.axis(slot()?, axis)))
            }
        })
    }
}

/// A player slot from a script: a whole number from 0, the any-pad slot, to
/// the most players a game can have.
fn slot(path: &Path, value: Option<&Value>) -> Result<u32, RuntimeError> {
    let refused = || {
        RuntimeError::Host(format!(
            "{} takes a player slot from 0 to {SLOT_LIMIT}, and the script gave {value:?}",
            path.dotted()
        ))
    };
    let number = number(path, value.ok_or_else(refused)?)?;
    if number.fract() != 0.0 || !(0.0..=slot_count(SLOT_LIMIT)).contains(&number) {
        return Err(refused());
    }
    // Checked just above to be a whole number no larger than the slot limit.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(number as u32)
}

/// A count of slots as a script's number; never more than the slot limit.
fn slot_count(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}
