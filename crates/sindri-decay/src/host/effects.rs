//! Performing an `Effects.*` call.
//!
//! A fleck is not an entity, so nothing here answers with one. A burst goes into
//! the pool and is never heard from again — which is the whole reason it is
//! affordable at the densities `docs/effect-scaling.md` measured.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::SceneComponent;
use sindri_scene::EffectBurstComponent;

use crate::surface::EffectsCall;

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    pub(super) fn effects_call(
        &mut self,
        call: EffectsCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        // A game whose explosions never appear because nothing is drawing them
        // should hear about it rather than watch nothing happen.
        let Some(effects) = self.effects.as_deref_mut() else {
            return Err(RuntimeError::Host(format!(
                "{}: this host is not running any effects",
                path.dotted()
            )));
        };
        if matches!(call, EffectsCall::Live) {
            #[allow(clippy::cast_precision_loss)]
            return Ok(Value::Number(effects.live() as f64));
        }

        let entity = self.entity_argument(path, args, 0, "the burst")?;
        let Some(payload) = self
            .world
            .get(entity)
            .and_then(|data| data.components.get(EffectBurstComponent::TYPE_NAME))
        else {
            return Err(RuntimeError::Host(format!(
                "{}: entity {} authors no burst",
                path.dotted(),
                entity.index()
            )));
        };
        let burst: EffectBurstComponent =
            serde_json::from_value(payload.clone()).map_err(|error| {
                RuntimeError::Host(format!(
                    "{}: its burst does not read: {error}",
                    path.dotted()
                ))
            })?;

        let at = if matches!(call, EffectsCall::BurstAt) {
            #[allow(clippy::cast_possible_truncation)]
            let x = number(path, args.get(1).unwrap_or(&Value::Null))? as f32;
            #[allow(clippy::cast_possible_truncation)]
            let y = number(path, args.get(2).unwrap_or(&Value::Null))? as f32;
            [x, y]
        } else {
            // The entity's own place. Read now rather than kept, because the
            // thing that threw the burst is usually about to be despawned.
            let position = self
                .world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default()
                .position_2d();
            [position[0], position[1]]
        };
        let effects = self
            .effects
            .as_deref_mut()
            .expect("checked above, and nothing since could have taken it");
        #[allow(clippy::cast_precision_loss)]
        Ok(Value::Number(effects.burst(&burst, at) as f64))
    }
}
