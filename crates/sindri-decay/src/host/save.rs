//! Performing a `Save.*` call.
//!
//! Everything here touches an in-memory store. Nothing here touches a disk: how
//! often someone's storage is written is a decision about their machine, and a
//! script asking for a write every frame would be making that decision badly on
//! everyone's behalf. The host writes out what is dirty, on its own schedule and
//! before it stops.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{SaveState, SaveValue};

use crate::surface::SaveCall;

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    pub(super) fn save_call(
        &mut self,
        call: SaveCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        // A game whose progress silently never persists is worth hearing about
        // on the first frame rather than after someone has played for an hour.
        let Some(saves) = self.saves.as_deref_mut() else {
            return Err(RuntimeError::Host(format!(
                "{}: this host is not keeping a save",
                path.dotted()
            )));
        };
        match call {
            SaveCall::Clear => {
                saves.clear();
                return Ok(Value::Unit);
            }
            SaveCall::IsNew => return Ok(Value::Bool(saves.state() == SaveState::New)),
            SaveCall::IsDamaged => return Ok(Value::Bool(saves.state() == SaveState::Damaged)),
            SaveCall::IsFromNewer => {
                return Ok(Value::Bool(matches!(
                    saves.state(),
                    SaveState::FromNewer { .. }
                )));
            }
            _ => {}
        }

        let Some(Value::String(key)) = args.first() else {
            return Err(RuntimeError::Host(format!(
                "{}: needs the name of what is being remembered",
                path.dotted()
            )));
        };
        match call {
            SaveCall::Has => Ok(Value::Bool(saves.has(key))),
            SaveCall::Number => {
                let fallback = number(path, args.get(1).unwrap_or(&Value::Null))?;
                Ok(Value::Number(saves.number(key, fallback)))
            }
            SaveCall::Flag => {
                let fallback = matches!(args.get(1), Some(Value::Bool(true)));
                Ok(Value::Bool(saves.flag(key, fallback)))
            }
            SaveCall::SetNumber => {
                let value = number(path, args.get(1).unwrap_or(&Value::Null))?;
                if !value.is_finite() {
                    // A NaN written to a save is one that comes back next run
                    // and poisons whatever reads it, long after the frame that
                    // produced it has gone.
                    return Err(RuntimeError::Host(format!(
                        "{}: {value} is not worth remembering",
                        path.dotted()
                    )));
                }
                saves.set(key.clone(), SaveValue::Number(value));
                Ok(Value::Unit)
            }
            SaveCall::SetFlag => {
                let Some(Value::Bool(value)) = args.get(1) else {
                    return Err(RuntimeError::Host(format!(
                        "{}: set_flag needs a truth to remember",
                        path.dotted()
                    )));
                };
                saves.set(key.clone(), SaveValue::Flag(*value));
                Ok(Value::Unit)
            }
            // Answered above, before a key was even looked for.
            SaveCall::Clear | SaveCall::IsNew | SaveCall::IsDamaged | SaveCall::IsFromNewer => {
                unreachable!("handled without a key")
            }
        }
    }
}
