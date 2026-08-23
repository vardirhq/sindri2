use serde::{Deserialize, Serialize};
use sindri_core::SceneComponent;

fn default_volume() -> f32 {
    1.0
}

/// Authored audio attached to a scene entity.
///
/// The component only describes what should play. Device state, active voices,
/// and browser unlock state are runtime concerns and never serialize into a
/// scene file.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioSourceComponent {
    /// Logical project asset ID, for example `audio/music.ogg`.
    pub clip: String,
    /// Start this source when the scene/game starts.
    #[serde(default)]
    pub autoplay: bool,
    /// Repeat until stopped rather than playing once.
    #[serde(default)]
    pub looping: bool,
    /// Linear gain in the inclusive 0..=1 range.
    #[serde(default = "default_volume")]
    pub volume: f32,
}

impl AudioSourceComponent {
    #[must_use]
    pub fn normalized_volume(&self) -> f32 {
        self.volume.clamp(0.0, 1.0)
    }
}

impl SceneComponent for AudioSourceComponent {
    const TYPE_NAME: &'static str = "sindri.audio.source";
}

#[cfg(test)]
mod tests {
    use sindri_core::SceneComponent;

    use super::AudioSourceComponent;

    #[test]
    fn audio_source_has_the_canonical_component_name() {
        assert_eq!(AudioSourceComponent::TYPE_NAME, "sindri.audio.source");
    }

    #[test]
    fn missing_optional_fields_have_safe_defaults() {
        let source: AudioSourceComponent = serde_json::from_value(serde_json::json!({
            "clip": "audio/pickup.wav"
        }))
        .expect("audio component");
        assert!(!source.autoplay);
        assert!(!source.looping);
        assert!((source.volume - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn runtime_volume_is_bounded() {
        let mut source = AudioSourceComponent {
            clip: "audio/music.ogg".to_owned(),
            autoplay: true,
            looping: true,
            volume: 4.0,
        };
        assert!((source.normalized_volume() - 1.0).abs() < f32::EPSILON);

        source.volume = -2.0;
        assert!(source.normalized_volume().abs() < f32::EPSILON);
    }
}
