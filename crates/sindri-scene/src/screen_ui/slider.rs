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
        if !self.value.is_finite() || !span.is_finite() || span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    /// Maps a normalized track position to the slider's authored range.
    ///
    /// Step quantization is relative to `min`, so ranges that do not begin at
    /// zero behave exactly like ranges that do. Invalid ranges stay pinned to
    /// their minimum rather than manufacturing a second interpretation.
    #[must_use]
    pub fn value_at(&self, normalized: f32) -> f32 {
        if !self.min.is_finite() || !self.max.is_finite() || self.max <= self.min {
            return self.min;
        }
        let normalized = if normalized.is_finite() {
            normalized.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let raw = self.min + (self.max - self.min) * normalized;
        self.coerce_value(raw)
    }

    /// Clamps and quantizes a value using the same rules as pointer input.
    ///
    /// Runtime writers such as Decay use this instead of assigning `value`
    /// directly, keeping authored, pointer-driven, and scripted slider values
    /// on one contract.
    #[must_use]
    pub fn coerce_value(&self, value: f32) -> f32 {
        if !self.min.is_finite() || !self.max.is_finite() || self.max <= self.min {
            return self.min;
        }
        let value = if value.is_finite() { value } else { self.min };
        let clamped = value.clamp(self.min, self.max);
        let stepped = if self.step.is_finite() && self.step > 0.0 {
            self.min + ((clamped - self.min) / self.step).round() * self.step
        } else {
            clamped
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
    fn coerces_scripted_values_with_pointer_rules() {
        let slider = slider();
        assert!((slider.coerce_value(15.1) - 16.0).abs() < f32::EPSILON);
        assert!((slider.coerce_value(-50.0) - 10.0).abs() < f32::EPSILON);
        assert!((slider.coerce_value(500.0) - 20.0).abs() < f32::EPSILON);
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
        assert!((slider.coerce_value(999.0) - slider.min).abs() < f32::EPSILON);
    }

    #[test]
    fn non_finite_input_does_not_poison_slider_state() {
        let mut slider = slider();
        slider.value = f32::NAN;
        assert!((slider.normalized() - 0.0).abs() < f32::EPSILON);
        assert!((slider.value_at(f32::NAN) - slider.min).abs() < f32::EPSILON);
        assert!((slider.coerce_value(f32::INFINITY) - slider.min).abs() < f32::EPSILON);
    }
}
