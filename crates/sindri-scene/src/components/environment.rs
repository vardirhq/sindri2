//! Authored world presentation shared by games, the editor, and browser builds.

use serde::{Deserialize, Serialize};
use sindri_core::{SceneComponent, World};

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
            bloom: EnvironmentBloom::default(),
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
    /// Refuses authored values that would make the renderer's behaviour
    /// surprising or non-finite.
    pub fn validate(self) -> Result<Self, EnvironmentError> {
        if !self.background.iter().all(|value| value.is_finite())
            || !self.ambient_color.iter().all(|value| value.is_finite())
            || !self.ambient_intensity.is_finite()
            || self.ambient_intensity < 0.0
        {
            return Err(EnvironmentError::InvalidColourOrIntensity);
        }
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
        Ok(self)
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
    fn non_finite_bloom_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.bloom.intensity = f32::NAN;
        assert_eq!(environment.validate(), Err(EnvironmentError::InvalidBloom));
    }
}
