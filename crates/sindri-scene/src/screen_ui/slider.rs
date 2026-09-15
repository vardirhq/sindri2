//! Slider semantics and value mapping for screen UI.

use serde::Deserialize;
use sindri_core::SceneComponent;

/// Direction a slider grows from minimum to maximum.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiSliderOrientation {
    #[default]
    Horizontal,
    Vertical,
}

impl UiSliderOrientation {
    pub const ALL: [Self; 2] = [Self::Horizontal, Self::Vertical];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// A value selected by dragging along an entity's screen-space rectangle.
///
/// The component owns interaction semantics only. Visuals remain ordinary UI
/// children so authored art and Weave can style them without a second rendering
/// path hidden inside the control.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiSliderComponent {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub orientation: UiSliderOrientation,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "UiSliderComponent::default_max")]
    pub max: f32,
    #[serde(default)]
    pub step: f32,
    #[serde(default)]
    pub value: f32,
    #[serde(default)]
    pub disabled: bool,
}

impl UiSliderComponent {
    const fn default_max() -> f32 {
        1.0
    }

    #[must_use]
    pub fn normalized(&self) -> f32 {
        let span = self.max - self.min;
        if !span.is_finite() || span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn value_at(&self, normalized: f32) -> f32 {
        if !self.min.is_finite() || !self.max.is_finite() || self.max <= self.min {
            return self.min;
        }
        let normalized = normalized.clamp(0.0, 1.0);
        let raw = self.min + (self.max - self.min) * normalized;
        let stepped = if self.step.is_finite() && self.step > 0.0 {
            self.min + ((raw - self.min) / self.step).round() * self.step
        } else {
            raw
        };
        stepped.clamp(self.min, self.max)
    }
}

impl SceneComponent for UiSliderComponent {
    const TYPE_NAME: &'static str = "sindri.ui.slider";
}

#[cfg(test)]
mod tests {
    use super::{UiSliderComponent, UiSliderOrientation};

    fn slider() -> UiSliderComponent {
        UiSliderComponent {
            label: String::new(),
            orientation: UiSliderOrientation::Horizontal,
            min: 10.0,
            max: 20.0,
            step: 2.0,
            value: 14.0,
            disabled: false,
        }
    }

    #[test]
    fn maps_range_and_quantizes_to_step() {
        let slider = slider();
        assert!((slider.value_at(0.0) - 10.0).abs() < f32::EPSILON);
        assert!((slider.value_at(0.51) - 16.0).abs() < f32::EPSILON);
        assert!((slider.value_at(1.0) - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reports_normalized_value() {
        assert!((slider().normalized() - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn invalid_ranges_are_stable() {
        let mut slider = slider();
        slider.max = slider.min;
        assert!((slider.normalized() - 0.0).abs() < f32::EPSILON);
        assert!((slider.value_at(0.75) - slider.min).abs() < f32::EPSILON);
    }
}
