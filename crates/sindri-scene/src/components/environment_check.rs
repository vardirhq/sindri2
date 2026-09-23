//! What an authored environment may hold, and what is said when it does not.
//!
//! Every refusal names the field by the path the scene file and the editor use
//! and says what that field accepts. "Shadow settings are outside their
//! supported ranges" left an author to guess which of four settings, and which
//! range; `shadows.map_size must be 256, 512, 1024, or 2048` does not.

use super::EnvironmentComponent;

/// Why an environment was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EnvironmentError {
    #[error("the scene contains more than one environment")]
    MultipleEnvironments,
    #[error("the environment payload does not match the authored schema")]
    InvalidPayload,
    /// One field holds a value the renderer cannot use.
    ///
    /// `field` is the dotted path into the component, such as
    /// `bloom.intensity`, so a tool can point at the field as well as quote it.
    #[error("environment `{field}` {expected}")]
    Field {
        field: &'static str,
        expected: &'static str,
    },
}

const FINITE: &str = "must be a finite number";
const NON_NEGATIVE: &str = "must be a finite number of at least 0";
const UNIT: &str = "must be between 0 and 1";

fn refuse(field: &'static str, expected: &'static str) -> EnvironmentError {
    EnvironmentError::Field { field, expected }
}

fn finite(field: &'static str, values: &[f32]) -> Result<(), EnvironmentError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(refuse(field, FINITE))
    }
}

fn at_least(
    field: &'static str,
    value: f32,
    minimum: f32,
    expected: &'static str,
) -> Result<(), EnvironmentError> {
    if value.is_finite() && value >= minimum {
        Ok(())
    } else {
        Err(refuse(field, expected))
    }
}

fn within(
    field: &'static str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    expected: &'static str,
) -> Result<(), EnvironmentError> {
    if value.is_finite() && range.contains(&value) {
        Ok(())
    } else {
        Err(refuse(field, expected))
    }
}

impl EnvironmentComponent {
    /// Refuses authored values that would make the renderer's behaviour
    /// surprising or non-finite, naming the first field that does.
    pub fn validate(self) -> Result<Self, EnvironmentError> {
        finite("background", &self.background)?;
        finite("ambient_color", &self.ambient_color)?;
        at_least(
            "ambient_intensity",
            self.ambient_intensity,
            0.0,
            NON_NEGATIVE,
        )?;
        self.validate_directional()?;
        self.validate_shadows()?;
        within(
            "ambient_occlusion.strength",
            self.ambient_occlusion.strength,
            0.0..=1.0,
            UNIT,
        )?;
        self.validate_fog()?;
        self.validate_post_process()?;
        self.validate_bloom()?;
        Ok(self)
    }

    fn validate_directional(self) -> Result<(), EnvironmentError> {
        let direction = self.directional.direction;
        let length_squared = direction.iter().map(|value| value * value).sum::<f32>();
        if !direction.iter().all(|value| value.is_finite()) || length_squared <= f32::EPSILON {
            return Err(refuse(
                "directional.direction",
                "must be finite and point somewhere: it cannot be zero",
            ));
        }
        finite("directional.color", &self.directional.color)?;
        at_least(
            "directional.intensity",
            self.directional.intensity,
            0.0,
            NON_NEGATIVE,
        )
    }

    fn validate_shadows(self) -> Result<(), EnvironmentError> {
        at_least(
            "shadows.distance",
            self.shadows.distance,
            1.0,
            "must be a finite number of at least 1",
        )?;
        if !matches!(self.shadows.map_size, 256 | 512 | 1024 | 2048) {
            return Err(refuse(
                "shadows.map_size",
                "must be 256, 512, 1024, or 2048",
            ));
        }
        within(
            "shadows.bias",
            self.shadows.bias,
            0.0..=0.05,
            "must be between 0 and 0.05",
        )
    }

    fn validate_fog(self) -> Result<(), EnvironmentError> {
        finite("fog.color", &self.fog.color)?;
        at_least("fog.start", self.fog.start, 0.0, NON_NEGATIVE)?;
        if !self.fog.distance.is_finite() || self.fog.distance <= 0.0 {
            return Err(refuse(
                "fog.distance",
                "must be a finite number greater than 0",
            ));
        }
        within("fog.density", self.fog.density, 0.0..=1.0, UNIT)?;
        finite("fog.height", &[self.fog.height])?;
        within(
            "fog.height_falloff",
            self.fog.height_falloff,
            0.0..=1.0,
            UNIT,
        )
    }

    fn validate_post_process(self) -> Result<(), EnvironmentError> {
        let post = self.post_process;
        within(
            "post_process.exposure",
            post.exposure,
            -8.0..=8.0,
            "must be between -8 and 8",
        )?;
        within(
            "post_process.contrast",
            post.contrast,
            0.0..=4.0,
            "must be between 0 and 4",
        )?;
        within(
            "post_process.saturation",
            post.saturation,
            0.0..=4.0,
            "must be between 0 and 4",
        )?;
        within("post_process.vignette", post.vignette, 0.0..=1.0, UNIT)
    }

    fn validate_bloom(self) -> Result<(), EnvironmentError> {
        at_least("bloom.threshold", self.bloom.threshold, 0.0, NON_NEGATIVE)?;
        within(
            "bloom.knee",
            self.bloom.knee,
            1.0e-4..=1.0,
            "must be between 0.0001 and 1",
        )?;
        at_least("bloom.intensity", self.bloom.intensity, 0.0, NON_NEGATIVE)?;
        if !(1..=8).contains(&self.bloom.passes) {
            return Err(refuse("bloom.passes", "must be between 1 and 8"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(environment: EnvironmentComponent) -> &'static str {
        match environment.validate() {
            Err(EnvironmentError::Field { field, .. }) => field,
            other => panic!("expected a field to be refused, got {other:?}"),
        }
    }

    #[test]
    fn default_environment_is_valid() {
        assert!(EnvironmentComponent::default().validate().is_ok());
    }

    #[test]
    fn zero_directional_light_direction_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.directional.direction = [0.0; 3];
        assert_eq!(refused(environment), "directional.direction");
    }

    #[test]
    fn invalid_shadow_map_size_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.shadows.map_size = 4096;
        assert_eq!(refused(environment), "shadows.map_size");
    }

    #[test]
    fn invalid_ambient_occlusion_strength_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.ambient_occlusion.strength = 1.5;
        assert_eq!(refused(environment), "ambient_occlusion.strength");
    }

    #[test]
    fn invalid_fog_distance_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.fog.distance = 0.0;
        assert_eq!(refused(environment), "fog.distance");
    }

    #[test]
    fn invalid_post_process_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.post_process.vignette = 2.0;
        assert_eq!(refused(environment), "post_process.vignette");
    }

    #[test]
    fn non_finite_bloom_is_rejected() {
        let mut environment = EnvironmentComponent::default();
        environment.bloom.intensity = f32::NAN;
        assert_eq!(refused(environment), "bloom.intensity");
    }

    /// The message is what reaches the console, so it has to carry both the
    /// field and what that field accepts.
    #[test]
    fn a_refusal_says_which_field_and_what_it_accepts() {
        let mut environment = EnvironmentComponent::default();
        environment.bloom.intensity = -1.22;
        let message = environment.validate().unwrap_err().to_string();
        assert_eq!(
            message,
            "environment `bloom.intensity` must be a finite number of at least 0"
        );
    }
}
