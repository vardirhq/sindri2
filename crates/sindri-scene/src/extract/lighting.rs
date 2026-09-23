//! The light a frame is drawn with: the environment's ambient, and the sun.

use sindri_core::{SceneComponent, World};
use sindri_render::WorldLighting;

use super::{SceneExtractError, SceneExtractor};
use crate::{EnvironmentComponent, LightComponent, Sun, lights_in, sun_in};

impl SceneExtractor {
    /// Ambient light from the environment and the sun from the first active
    /// light entity. A scene with neither is lit by the renderer's defaults
    /// and no sun.
    pub fn lighting(&self, world: &World) -> Result<WorldLighting, SceneExtractError> {
        let unlit = WorldLighting {
            directional_intensity: 0.0,
            ..WorldLighting::default()
        };
        let mut lighting = self
            .environment(world)?
            .map_or(unlit, EnvironmentComponent::world_lighting);
        if let Some(sun) = self.sun(world)? {
            lighting.directional_direction = sun.direction;
            lighting.directional_color = sun.color;
            lighting.directional_intensity = sun.intensity;
        }
        Ok(lighting)
    }

    /// The light entity lighting the world, if any.
    ///
    /// Strictly, a light the engine cannot use fails the frame. Tolerantly it
    /// is recorded against its entity and passed over, so a value dragged out
    /// of range in the inspector turns the light off while it is wrong rather
    /// than freezing the view.
    pub fn sun(&self, world: &World) -> Result<Option<Sun>, SceneExtractError> {
        for (entity, light) in lights_in(world) {
            match light {
                Ok(light) => {
                    if let Some(sun) = sun_in(world, light, entity) {
                        return Ok(Some(sun));
                    }
                }
                Err(error) if self.tolerant() => {
                    self.record(entity, LightComponent::TYPE_NAME, &error);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }
}
