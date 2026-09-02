//! Deciding which namespace a call belongs to, and answering three of them.
//!
//! The dispatch is a table rather than a chain of near-identical blocks: every
//! namespace the surface grows is one line here, and a chain was six — which is
//! how the caller reached a length nobody reads.
//!
//! `Game`, `Pointer` and `Input` are answered here rather than in files of
//! their own because each is a handful of lines over state the host already
//! holds. A namespace earns its own file when it has something to own.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};

use crate::surface::{
    EFFECTS, EFFECTS_CALLS, GAME, GAME_CALLS, GRID, GRID_CALLS, GameCall, INPUT, INPUT_QUERIES,
    InputQuery, PHYSICS, PHYSICS_CALLS, POINTER, POINTER_QUERIES, PointerQuery, RANDOM,
    RANDOM_CALLS, SAVE, SAVE_CALLS, TOUCH, TOUCH_CALLS, UI, UI_CALLS, WORLD, WORLD_CALLS,
};

use super::WorldHost;
use super::convert::{button, key, number};

impl WorldHost<'_> {
    /// Performs a call that names a namespace and a call inside it.
    ///
    /// One table rather than a chain of near-identical blocks: every namespace
    /// the surface grows is a line here, and a chain was six — which is how
    /// `call` reached a length nobody reads.
    ///
    /// `None` means no namespace claimed it, which the caller reports as an
    /// unknown call rather than a failed one.
    pub(super) fn namespaced_call(
        &mut self,
        namespace: &str,
        name: &str,
        path: &Path,
        args: &[Value],
    ) -> Option<Result<Value, RuntimeError>> {
        match namespace {
            GAME => named(GAME_CALLS, name).map(|call| self.game_call(call, path, args)),
            WORLD => named(WORLD_CALLS, name).map(|call| self.world_call(call, path, args)),
            PHYSICS => named(PHYSICS_CALLS, name).map(|call| self.physics_call(call, path, args)),
            UI => named(UI_CALLS, name).map(|call| self.ui_call(call, path, args)),
            RANDOM => named(RANDOM_CALLS, name).map(|call| self.random_call(call, path, args)),
            SAVE => named(SAVE_CALLS, name).map(|call| self.save_call(call, path, args)),
            EFFECTS => named(EFFECTS_CALLS, name).map(|call| self.effects_call(call, path, args)),
            GRID => named(GRID_CALLS, name).map(|call| self.grid_call(call, path, args)),
            TOUCH => named(TOUCH_CALLS, name).map(|call| self.touch_call(call, path, args)),
            POINTER => {
                named(POINTER_QUERIES, name).map(|query| self.pointer_query(query, path, args))
            }
            INPUT => named(INPUT_QUERIES, name).map(|query| self.input_query(query, path, args)),
            _ => None,
        }
    }

    /// A note on the blackboard every script shares.
    fn game_call(
        &mut self,
        call: GameCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
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
        Ok(match call {
            GameCall::Get => Value::Number(self.blackboard.get(&note, value(1)?)),
            GameCall::Set => {
                self.blackboard.set(note, value(1)?);
                Value::Unit
            }
        })
    }

    /// Whether the pointer is doing something with this button.
    fn pointer_query(
        &mut self,
        query: PointerQuery,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let input = self.context.input;
        let button = button(path, args.first())?;
        Ok(Value::Bool(match query {
            PointerQuery::Down => input.pointer_down(button),
            PointerQuery::Pressed => input.pointer_pressed(button),
            PointerQuery::Released => input.pointer_released(button),
        }))
    }

    /// What the keyboard is doing.
    fn input_query(
        &mut self,
        query: InputQuery,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let input = self.context.input;
        Ok(match query {
            InputQuery::Axis => Value::Number(f64::from(
                input.axis(key(path, args.first())?, key(path, args.get(1))?),
            )),
            InputQuery::Down => Value::Bool(input.key_down(key(path, args.first())?)),
            InputQuery::Pressed => Value::Bool(input.key_pressed(key(path, args.first())?)),
            InputQuery::Released => Value::Bool(input.key_released(key(path, args.first())?)),
        })
    }
}

/// Finds the typed call a name stands for in one namespace's table.
///
/// The tables are short and read once per call; a map would be more machinery
/// than the lookup is worth, and this keeps each table a plain list anyone can
/// read down.
fn named<T: Copy>(table: &[(&str, T)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, call)| *call)
}
