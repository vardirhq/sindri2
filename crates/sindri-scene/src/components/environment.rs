//! Authored world presentation shared by games, the editor, and browser builds.

use serde::{Deserialize, Serialize};
use sindri_core::{EntityId, SceneComponent, World};

use super::EnvironmentError;
use sindri_render::{
    BloomSettings, FogSettings, PostProcessSettings, ShadowSettings, ToneMapping, WorldLighting,
};

/// Scene-wide visual environment: the sky, ambient light, shadows, fog and
/// the grade.
///
/// The sun is not here. It is a light entity (`sindri.light`), aimed by its
/// rotation and drawn in the Scene view; scenes that held a direction here are
/// migrated to one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentComponent {
    pub background: [f32; 4],
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub shadows: EnvironmentShadows,
    pub ambient_occlusion: EnvironmentAmbientOcclusion,
    pub fog: EnvironmentFog,
    pub post_process: EnvironmentPostProcess,
    pub bloom: EnvironmentBloom,
}

impl SceneComponent for EnvironmentComponent {
    const TYPE_NAME: &'static str = "sindri.environment";
}

impl Default for EnvironmentComponent {
    fn default() -> Self {
        Self {
            background: [0.035, 0.045, 0.065, 1.0],
            ambient_color: [1.0, 1.0, 1.0],
            ambient_intensity: 1.0,
            shadows: EnvironmentShadows::default(),
            ambient_occlusion: EnvironmentAmbientOcclusion::default(),
            fog: EnvironmentFog::default(),
            post_process: EnvironmentPostProcess::default(),
            bloom: EnvironmentBloom::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentShadows {
    pub enabled: bool,
    pub distance: f32,
    pub map_size: u32,
    pub bias: f32,
}

impl Default for EnvironmentShadows {
    fn default() -> Self {
        Self {
            enabled: false,
            distance: 48.0,
            map_size: 1024,
            bias: 0.002,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentAmbientOcclusion {
    pub enabled: bool,
    pub strength: f32,
}

impl Default for EnvironmentAmbientOcclusion {
    fn default() -> Self {
        Self {
            enabled: false,
            strength: 0.65,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentFog {
    pub enabled: bool,
    pub color: [f32; 3],
    pub start: f32,
    pub distance: f32,
    pub density: f32,
    pub height: f32,
    pub height_falloff: f32,
}

impl Default for EnvironmentFog {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [0.55, 0.65, 0.75],
            start: 24.0,
            distance: 64.0,
            density: 0.0,
            height: 0.0,
            height_falloff: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentPostProcess {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub tone_mapping: EnvironmentToneMapping,
    pub vignette: f32,
}

impl Default for EnvironmentPostProcess {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            tone_mapping: EnvironmentToneMapping::None,
            vignette: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentToneMapping {
    #[default]
    None,
    Reinhard,
    Aces,
}

impl EnvironmentToneMapping {
    /// Every spelling a scene may write, for a tool offering the choice.
    pub const NAMES: [&'static str; 3] = ["none", "reinhard", "aces"];

    const fn renderer(self) -> ToneMapping {
        match self {
            Self::None => ToneMapping::None,
            Self::Reinhard => ToneMapping::Reinhard,
            Self::Aces => ToneMapping::Aces,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentBloom {
    pub enabled: bool,
    pub threshold: f32,
    pub knee: f32,
    pub intensity: f32,
    pub passes: u32,
}

impl Default for EnvironmentBloom {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.65,
            knee: 0.20,
            intensity: 0.45,
            passes: 3,
        }
    }
}

impl EnvironmentComponent {
    /// The environment's share of the light: ambient only. The sun comes
    /// from a light entity; see `SceneExtractor::lighting`.
    #[must_use]
    pub fn world_lighting(self) -> WorldLighting {
        WorldLighting {
            ambient_color: self.ambient_color,
            ambient_intensity: self.ambient_intensity,
            directional_intensity: 0.0,
            ..WorldLighting::default()
        }
    }

    #[must_use]
    pub const fn shadow_settings(self) -> ShadowSettings {
        ShadowSettings {
            enabled: self.shadows.enabled,
            distance: self.shadows.distance,
            map_size: self.shadows.map_size,
            bias: self.shadows.bias,
        }
    }

    #[must_use]
    pub const fn fog_settings(self) -> FogSettings {
        FogSettings {
            enabled: self.fog.enabled,
            color: self.fog.color,
            start: self.fog.start,
            distance: self.fog.distance,
            density: self.fog.density,
            height: self.fog.height,
            height_falloff: self.fog.height_falloff,
        }
    }

    #[must_use]
    pub const fn post_process_settings(self) -> PostProcessSettings {
        PostProcessSettings {
            exposure: self.post_process.exposure,
            contrast: self.post_process.contrast,
            saturation: self.post_process.saturation,
            tone_mapping: self.post_process.tone_mapping.renderer(),
            vignette: self.post_process.vignette,
            bloom: BloomSettings {
                enabled: self.bloom.enabled,
                threshold: self.bloom.threshold,
                knee: self.bloom.knee,
                intensity: self.bloom.intensity,
                passes: self.bloom.passes,
            },
        }
    }
}

/// Finds the single authored environment in a world.
pub fn environment_of(world: &World) -> Result<Option<EnvironmentComponent>, EnvironmentError> {
    let mut found = None;
    for (_, environment) in environments_in(world) {
        if found.is_some() {
            return Err(EnvironmentError::MultipleEnvironments);
        }
        found = Some(environment?);
    }
    Ok(found)
}

/// Every entity carrying an environment, each with its own verdict.
///
/// The per-entity form of [`environment_of`], for a caller that has to say
/// *which* environment is wrong rather than only that one is.
pub fn environments_in(
    world: &World,
) -> Vec<(EntityId, Result<EnvironmentComponent, EnvironmentError>)> {
    world
        .entities()
        .filter_map(|(entity, data)| {
            let payload = data.components.get(EnvironmentComponent::TYPE_NAME)?;
            let environment = serde_json::from_value::<EnvironmentComponent>(payload.clone())
                .map_err(|_| EnvironmentError::InvalidPayload)
                .and_then(EnvironmentComponent::validate);
            Some((entity, environment))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spellings a tool offers are the ones serde accepts, all of them.
    #[test]
    fn every_tone_mapping_name_is_a_tone_mapping() {
        let parsed: Vec<EnvironmentToneMapping> = EnvironmentToneMapping::NAMES
            .iter()
            .map(|name| serde_json::from_value(serde_json::json!(name)).expect(name))
            .collect();
        assert_eq!(
            parsed,
            [
                EnvironmentToneMapping::None,
                EnvironmentToneMapping::Reinhard,
                EnvironmentToneMapping::Aces
            ]
        );
    }

    #[test]
    fn legacy_environment_does_not_gain_shadows_or_effects() {
        let environment: EnvironmentComponent = serde_json::from_value(serde_json::json!({
            "background": [0.0, 0.0, 0.0, 1.0],
            "ambient_color": [1.0, 1.0, 1.0],
            "ambient_intensity": 1.0,
            "bloom": {
                "enabled": false,
                "threshold": 0.65,
                "knee": 0.2,
                "intensity": 0.45,
                "passes": 3
            }
        }))
        .expect("legacy environment should deserialize");
        assert!(!environment.shadows.enabled);
        assert!(!environment.ambient_occlusion.enabled);
        assert!(!environment.fog.enabled);
    }
}
