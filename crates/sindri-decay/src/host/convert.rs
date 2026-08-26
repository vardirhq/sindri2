//! Turning a script value into something the engine can use, and back.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_platform::Key;

use crate::surface::Seg;

/// Decay's only numeric type is spelled `f32` and holds an `f64`; every engine
/// transform is `f32`. This is the one place the two meet, so it is the one
/// place that narrows, and narrowing is the intent rather than a hazard.
/// `docs/decay-direction.md` records that the numeric type is a decision still
/// owed an answer.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn as_f32(number: f64) -> f32 {
    number as f32
}

pub(super) fn number(path: &Path, value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Number(number) => Ok(*number),
        other => Err(RuntimeError::Host(format!(
            "{} takes a number, and the script gave {other:?}",
            path.dotted()
        ))),
    }
}

/// A key name from a script, resolved to the key it names.
pub(super) fn key(path: &Path, value: Option<&Value>) -> Result<Key, RuntimeError> {
    let Some(Value::String(name)) = value else {
        return Err(RuntimeError::Host(format!(
            "{} takes key names, and the script gave {value:?}",
            path.dotted()
        )));
    };
    // Refused rather than treated as never-held. A mistyped key name that
    // silently reads as "not pressed" is a control that does nothing for a
    // reason nobody can see.
    Key::from_name(name)
        .ok_or_else(|| RuntimeError::Host(format!("there is no key called `{name}`")))
}

pub(super) fn describe(pointer: &[Seg]) -> String {
    pointer
        .iter()
        .map(|step| match step {
            Seg::Field(name) => (*name).to_owned(),
            Seg::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}
