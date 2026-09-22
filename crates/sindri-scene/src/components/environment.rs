//! Authored world presentation shared by games, the editor, and browser builds.

use serde::{Deserialize, Serialize};
use sindri_core::{SceneComponent, World};
use sindri_render::{
    BloomSettings, PostProcessSettings, ShadowSettings, ToneMapping, WorldLighting,
};

/// Scene-wide visual environment.
///
/// Phase one deliberately authors ambient values before the lighting renderer
/// consumes them. That keeps the environment contract stable while the next
/// slice adds directional and ambient lighting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentComponent {
    pub background: [f32; 4],
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub directional: EnvironmentDirectionalLight,
    pub shadows: EnvironmentShadows,
    pub ambient_occlusion: EnvironmentAmbientOcclusion,
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
            directional: EnvironmentDirectionalLight::default(),
            shadows: EnvironmentShadows::default(),
            ambient_occlusion: EnvironmentAmbientOcclusion::default(),
            post_process: EnvironmentPostProcess::default(),
            bloom: EnvironmentBloom::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentDirectionalLight {
    /// Direction light travels from its source toward the world.
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for EnvironmentDirectionalLight {
    fn default() -> Self {
        Self {
            direction: [-0.45, -1.0, -0.35],
            color: [1.0, 0.95, 0.86],
            intensity: 0.0,
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
    /// Converts authored environment light into renderer-neutral frame state.
    #[must_use]
    pub const fn world_lighting(self) -> WorldLighting {
        WorldLighting {
            ambient_color: self.ambient_color,
            ambient_intensity: self.ambient_intensity,
            directional_direction: self.directional.direction,
            directional_color: self.directional.color,
            directional_intensity: self.directional.intensity,
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

    /// Refuses authored values that would make the renderer's behaviour
    /// surprising or non-finite.
    pub fn validate(self) -> Result<Self, EnvironmentError> {
        if !self.background.iter().all(|value| value.is_finite())
            || !self.ambient_color.iter().all(|value| value.is_finite())
            || !self.ambient_intensity.is_finite()
            || self.ambient_intensity < 0.0
            || !self.directional.color.iter().all(|value| value.is_finite())
            || !self.directional.intensity.is_finite()
            || self.directional.intensity < 0.0
        {
            return Err(EnvironmentError::InvalidColourOrIntensity);
        }
        let direction_length_squared = self
            .directional
            .direction
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        if !self
            .directional
            .direction
            .iter()
            .all(|value| value.is_finite())
            || direction_length_squared <= f32::EPSILON
        {
            return Err(EnvironmentError::InvalidDirectionalLight);
        }
        if !self.shadows.distance.is_finite()
            || self.shadows.distance < 1.0
            || !matches!(self.shadows.map_size, 256 | 512 | 1024 | 2048)
            || !self.shadows.bias.is_finite()
            || !(0.0..=0.05).contains(&self.shadows.bias)
        {
            return Err(EnvironmentError::InvalidShadows);
        }
        if !self.ambient_occlusion.strength.is_finite()
            || !(0.0..=1.0).contains(&self.ambient_occlusion.strength)
        {
            return Err(EnvironmentError::InvalidAmbientOcclusion);
        }
        self.validate_post_process()?;
        self.validate_bloom()?;
        Ok(self)
    }
    fn validate_post_process(self) -> Result<(), EnvironmentError> {
        if !self.post_process.exposure.is_finite()
            || !(-8.0..=8.0).contains(&self.post_process.exposure)
            || !self.post_process.contrast.is_finite()
            || !(0.0..=4.0).contains(&self.post_process.contrast)
            || !self.post_process.saturation.is_finite()
            || !(0.0..=4.0).contains(&self.post_process.saturation)
            || !self.post_process.vignette.is_finite()
            || !(0.0..=1.0).contains(&self.post_process.vignette)
        {
            return Err(EnvironmentError::InvalidPostProcess);
        }
        Ok(())
    }

    fn validate_bloom(self) -> Result<(), EnvironmentError> {
        if !self.bloom.threshold.is_finite()
            || self.bloom.threshold < 0.0
            || !self.bloom.knee.is_finite()
            || !(1.0e-4..=1.0).contains(&self.bloom.knee)
            || !self.bloom.intensity.is_finite()
            || self.bloom.intensity < 0.0
            || !(1..=8).contains(&self.bloom.passes)
        {
            return Err(EnvironmentError::InvalidBloom);
        }
        Ok(())
    }
}

/// Finds the single authored environment in a world.
pub fn environment_of(world: &World) -> Result<Option<EnvironmentComponent>, EnvironmentError> {
    let mut found = None;
    for (_, data) in world.entities() {
        let Some(payload) = data.components.get(EnvironmentComponent::TYPE_NAME) else {
            continue;
        };
        if found.is_some() {
            return Err(EnvironmentError::MultipleEnvironments);
        }
        let environment: EnvironmentComponent = serde_json::from_value(payload.clone())
            .map_err(|_| EnvironmentError::InvalidPayload)?;
        found = Some(environment.validate()?);
    }
    Ok(found)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EnvironmentError {
    #[error("the scene contains more than one environment")]
    MultipleEnvironments,
    #[error("the environment payload does not match the authored schema")]
    InvalidPayload,
    #[error(
        "environment colours and ambient intensity must be finite, with non-negative intensity"
    )]
    InvalidColourOrIntensity,
    #[error("environment directional light needs a finite, non-zero direction")]
    InvalidDirectionalLight,
    #[error("environment shadow settings are outside their supported ranges")]
    InvalidShadows,
    #[error("environment ambient-occlusion settings are outside their supported ranges")]
    InvalidAmbientOcclusion,
    #[error("environment post-process settings are outside their supported ranges")]
    InvalidPostProcess,
    #[error("environment bloom settings are outside their supported ranges")]
    InvalidBloom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_environment_is_valid() {
        assert!(EnvironmentComponent::default().validate().is_ok());
    }

    #[test]
    fn zero_directional_light_direction_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.directional.direction = [0.0; 3];
        assert_eq!(
            environment.validate(),
            Err(EnvironmentError::InvalidDirectionalLight)
        );
    }

    #[test]
    fn legacy_environment_does_not_gain_directional_light_or_shadows() {
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
        assert!(environment.directional.intensity.abs() < f32::EPSILON);
        assert!(!environment.shadows.enabled);
        assert!(!environment.ambient_occlusion.enabled);
    }

    #[test]
    fn invalid_shadow_map_size_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.shadows.map_size = 4096;
        assert_eq!(
            environment.validate(),
            Err(EnvironmentError::InvalidShadows)
        );
    }

    #[test]
    fn invalid_ambient_occlusion_strength_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.ambient_occlusion.strength = 1.5;
        assert_eq!(
            environment.validate(),
            Err(EnvironmentError::InvalidAmbientOcclusion)
        );
    }

    #[test]
    fn invalid_post_process_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.post_process.vignette = 2.0;
        assert_eq!(
            environment.validate(),
            Err(EnvironmentError::InvalidPostProcess)
        );
    }

    #[test]
    fn non_finite_bloom_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.bloom.intensity = f32::NAN;
        assert_eq!(environment.validate(), Err(EnvironmentError::InvalidBloom));
    }
}
