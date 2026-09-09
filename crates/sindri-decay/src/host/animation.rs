//! Playing an authored clip, and asking where it has got to.
//!
//! Two halves that live in two places, deliberately. *Which* clip an entity
//! plays is authored state and belongs in the world, so `play` and `stop` write
//! the component and the cursor follows on the next advance. *Where* that clip
//! has got to is derived, and lives beside the world in `SpriteAnimations`,
//! because advancing an animation must not dirty the scene it came from.
//!
//! Scripts run before animations advance, which is what makes this ordering
//! work: a script names a clip, and the advance in the same fixed step is the
//! one that starts it. Reads answer for the step that has already happened,
//! which is what `is_finished` has to mean — a clip cannot finish during the
//! frame a script asks about it.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{EntityId, SceneComponent};
use sindri_scene::SpriteAnimationComponent;

use crate::surface::AnimationCall;

use super::WorldHost;
use super::convert::number;

/// The component the clips are authored in.
const COMPONENT: &str = SpriteAnimationComponent::TYPE_NAME;

/// The field naming the clip that plays, or nothing.
const PLAYING: &str = "playing";

/// The field scaling how fast it plays.
const SPEED: &str = "speed";

impl WorldHost<'_> {
    pub(super) fn animation_call(
        &mut self,
        call: AnimationCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let target = self.entity_argument(path, args, 0, "the animation")?;
        match call {
            AnimationCall::Play => {
                let Some(Value::String(clip)) = args.get(1) else {
                    return Err(RuntimeError::Host(format!(
                        "{} names its clip with text",
                        path.dotted()
                    )));
                };
                self.write_playing(target, serde_json::Value::String(clip.clone()), path)
            }
            AnimationCall::Restart => {
                if let Some(animations) = self.animations.as_mut() {
                    animations.restart(target);
                }
                Ok(Value::Unit)
            }
            AnimationCall::Stop => self.write_playing(target, serde_json::Value::Null, path),
            AnimationCall::Speed => {
                let speed = number(
                    path,
                    args.get(1).ok_or_else(|| {
                        RuntimeError::Host(format!("{} wants more arguments", path.dotted()))
                    })?,
                )?;
                self.write_field(target, SPEED, serde_json::Value::from(speed), path)
            }
            AnimationCall::Finished => Ok(Value::Bool(
                self.animations
                    .as_ref()
                    .is_some_and(|animations| animations.is_finished(target)),
            )),
            // Zero for an entity playing nothing, which is the frame anything
            // that is playing starts on: a script asking where a stopped clip
            // has got to is asking about its beginning.
            AnimationCall::Frame => Ok(Value::Number(
                self.animations
                    .as_ref()
                    .and_then(|animations| animations.frame(target))
                    // Through `u32` so the conversion is exact rather than
                    // nearly so. A clip with more frames than that is not a
                    // clip, and saturating keeps "not playing" as its own
                    // answer instead of folding it in with an absurd index.
                    .map_or(0.0, |frame| {
                        f64::from(u32::try_from(frame).unwrap_or(u32::MAX))
                    }),
            )),
            AnimationCall::Clip => Ok(Value::String(self.playing_clip(target))),
        }
    }

    /// The clip an entity's component says is playing, or an empty string.
    ///
    /// Read from the world rather than from the cursor beside it, so a clip
    /// named this step answers as playing before the advance that starts it.
    fn playing_clip(&self, target: EntityId) -> String {
        self.world
            .get(target)
            .and_then(|data| data.components.get(COMPONENT))
            .and_then(|payload| payload.get(PLAYING))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    fn write_playing(
        &mut self,
        target: EntityId,
        clip: serde_json::Value,
        path: &Path,
    ) -> Result<Value, RuntimeError> {
        self.write_field(target, PLAYING, clip, path)
    }

    /// Writes one field of the animation component, or says why it could not.
    ///
    /// An entity with no animation component is an error rather than a silent
    /// nothing, for the reason the rest of this surface gives: a script telling
    /// something to play a clip it cannot hold is a mistake worth hearing about
    /// on the frame it happens.
    fn write_field(
        &mut self,
        target: EntityId,
        field: &str,
        value: serde_json::Value,
        path: &Path,
    ) -> Result<Value, RuntimeError> {
        let Some(data) = self.world.get_mut(target) else {
            return Ok(Value::Unit);
        };
        let Some(payload) = data.components.get_mut(COMPONENT) else {
            return Err(RuntimeError::Host(format!(
                "{} needs a {COMPONENT} on that entity, and it has none",
                path.dotted()
            )));
        };
        let Some(fields) = payload.as_object_mut() else {
            return Err(RuntimeError::Host(format!(
                "{}'s {COMPONENT} is not an object",
                path.dotted()
            )));
        };
        // Writing the value it already holds is not a write. A script says what
        // state it is in every frame, so most of these calls are the same
        // answer as last frame, and a scene should not be touched for that.
        if fields.get(field) == Some(&value) {
            return Ok(Value::Unit);
        }
        fields.insert(field.to_owned(), value);
        Ok(Value::Unit)
    }
}
