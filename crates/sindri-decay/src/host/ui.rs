//! Performing a `Ui.*` call.
//!
//! Every call here writes into the component payload the entity already
//! carries, so what a script changes is what the scene holds and what the
//! renderer reads. There is no side table of runtime UI state to keep in step
//! with the world, because there is no second copy of anything.
//!
//! The numbers go into `values` rather than into `text`. A script that wrote a
//! finished string would consume the template on its first call, and the words
//! would stop being something the scene owns.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use serde_json::json;
use sindri_core::{EntityId, SceneComponent};
use sindri_scene::{UiImageComponent, UiTextComponent};

use crate::surface::UiCall;

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    pub(super) fn ui_call(
        &mut self,
        call: UiCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let entity = self.entity_argument(path, args, 0, "the element")?;
        if call.is_query() {
            // A menu whose buttons never respond because nothing is laying them
            // out should be heard about on the first frame, not mistaken for a
            // person who has not clicked yet.
            let Some(screen) = self.screen_ui else {
                return Err(RuntimeError::Host(format!(
                    "{}: this host is not laying out any screen UI",
                    path.dotted()
                )));
            };
            return Ok(Value::Bool(match call {
                UiCall::Hovered => screen.is_hovered(entity),
                UiCall::Pressed => screen.is_pressed(entity),
                _ => screen.is_held(entity),
            }));
        }
        match call {
            UiCall::Text => {
                let Some(Value::String(text)) = args.get(1) else {
                    return Err(RuntimeError::Host(format!(
                        "{}: set_text needs the words to show",
                        path.dotted()
                    )));
                };
                let text = text.clone();
                self.write_text(entity, path, |payload| {
                    payload["text"] = json!(text);
                })
            }
            UiCall::Number | UiCall::Numbers => {
                let count = if matches!(call, UiCall::Number) { 1 } else { 2 };
                let mut values = Vec::with_capacity(count);
                for index in 0..count {
                    // Narrowed for the reason every number crossing into the
                    // engine is: Decay holds an `f64` and a scene holds `f32`.
                    #[allow(clippy::cast_possible_truncation)]
                    values.push(number(path, args.get(index + 1).unwrap_or(&Value::Null))? as f32);
                }
                self.write_text(entity, path, |payload| {
                    payload["values"] = json!(values);
                })
            }
            UiCall::Fill => {
                #[allow(clippy::cast_possible_truncation)]
                let amount = number(path, args.get(1).unwrap_or(&Value::Null))? as f32;
                let data = self
                    .world
                    .get_mut(entity)
                    .ok_or_else(|| gone(path, entity))?;
                let payload = data
                    .components
                    .get_mut(UiImageComponent::TYPE_NAME)
                    .ok_or_else(|| not_a(path, entity, "an image"))?;
                // Only the fraction: which edge a bar empties towards is a
                // designer's decision about how the bar reads, not a per-frame
                // gameplay value, so a script setting one cannot flip it.
                payload["fill"]["amount"] = json!(amount);
                Ok(Value::Unit)
            }
            // Answered above, before the entity was even resolved to a payload.
            UiCall::Hovered | UiCall::Pressed | UiCall::Held => unreachable!("handled as a query"),
        }
    }

    /// Edits an entity's text payload, or says why it could not.
    fn write_text(
        &mut self,
        entity: EntityId,
        path: &Path,
        edit: impl FnOnce(&mut serde_json::Value),
    ) -> Result<Value, RuntimeError> {
        let data = self
            .world
            .get_mut(entity)
            .ok_or_else(|| gone(path, entity))?;
        let payload = data
            .components
            .get_mut(UiTextComponent::TYPE_NAME)
            .ok_or_else(|| not_a(path, entity, "text"))?;
        edit(payload);
        Ok(Value::Unit)
    }
}

/// A reference to something that has been despawned.
fn gone(path: &Path, entity: EntityId) -> RuntimeError {
    RuntimeError::Host(format!(
        "{}: entity {} is no longer in the world",
        path.dotted(),
        entity.index()
    ))
}

/// Naming the entity rather than doing nothing: a HUD that silently stops
/// updating because a script points at the wrong element is the failure that
/// survives a play-test.
fn not_a(path: &Path, entity: EntityId, kind: &str) -> RuntimeError {
    RuntimeError::Host(format!(
        "{}: entity {} is not {kind} element",
        path.dotted(),
        entity.index()
    ))
}
