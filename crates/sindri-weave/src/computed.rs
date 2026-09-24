//! Cascade resolution between parsed Weave rules and Sindri presentation data.
//!
//! Parsing answers what the stylesheet says. This module answers the separate
//! question of which declaration wins for one entity. Keeping that answer as a
//! computed style gives later layout work one stable input instead of teaching
//! every property about selector specificity and source order.

use std::collections::BTreeMap;

use weave::Viewport;

/// A length with its authored unit preserved until the viewport is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Length {
    Overlay(f32),
    Pixels(f32),
    ViewWidth(f32),
    ViewHeight(f32),
    Percent(f32),
}

impl Length {
    pub(super) fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let (number, make): (&str, fn(f32) -> Self) = if let Some(number) = value.strip_suffix('%')
        {
            (number, Self::Percent)
        } else if let Some(number) = value.strip_suffix("vw") {
            (number, Self::ViewWidth)
        } else if let Some(number) = value.strip_suffix("vh") {
            (number, Self::ViewHeight)
        } else if let Some(number) = value.strip_suffix("px") {
            (number, Self::Pixels)
        } else {
            (value, Self::Overlay)
        };
        let number = number.trim().parse::<f32>().ok()?;
        number.is_finite().then(|| make(number))
    }

    #[must_use]
    pub(super) fn resolve(self, viewport: Viewport, percent_basis: Option<f32>) -> Option<f32> {
        match self {
            Self::Overlay(value) => Some(value),
            Self::Pixels(value) => Some(value * 2.0 / viewport.height.max(1.0)),
            Self::ViewWidth(value) => {
                Some((value / 100.0) * 2.0 * viewport.width / viewport.height.max(1.0))
            }
            Self::ViewHeight(value) => Some((value / 100.0) * 2.0),
            Self::Percent(value) => Some((value / 100.0) * percent_basis?),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ComputedValue {
    Length(Option<Length>),
    Raw,
}

#[derive(Clone, Debug, PartialEq)]
struct ComputedDeclaration {
    authored: String,
    value: ComputedValue,
}

impl ComputedDeclaration {
    fn new(property: &str, authored: String) -> Self {
        let value = if is_length_property(property) {
            ComputedValue::Length(Length::parse(&authored))
        } else {
            ComputedValue::Raw
        };
        Self { authored, value }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ComputedStyle {
    declarations: BTreeMap<String, ComputedDeclaration>,
}

impl ComputedStyle {
    /// The declarations the cascade settled on, ready for the bridge to
    /// apply: custom properties are left behind, having done their work.
    #[must_use]
    pub(super) fn from_computed(computed: &weave::Computed) -> Self {
        Self {
            declarations: computed
                .declarations()
                .map(|(property, value)| {
                    (
                        property.to_owned(),
                        ComputedDeclaration::new(property, value.to_owned()),
                    )
                })
                .collect(),
        }
    }

    #[must_use]
    pub(super) fn get(&self, property: &str) -> Option<&str> {
        self.declarations
            .get(property)
            .map(|declaration| declaration.authored.as_str())
    }

    /// Returns an eagerly parsed length for a known length-valued property.
    ///
    /// `Err` preserves the authored spelling so the bridge can report a useful
    /// `ApplyError` without parsing the same declaration again in every layout
    /// phase that consumes it.
    pub(super) fn length(&self, property: &str) -> Option<Result<Length, &str>> {
        let declaration = self.declarations.get(property)?;
        match declaration.value {
            ComputedValue::Length(Some(length)) => Some(Ok(length)),
            ComputedValue::Length(None) => Some(Err(declaration.authored.as_str())),
            ComputedValue::Raw => None,
        }
    }

    pub(super) fn into_declarations(self) -> impl Iterator<Item = (String, String)> {
        self.declarations
            .into_iter()
            .map(|(property, declaration)| (property, declaration.authored))
    }
}

fn is_length_property(property: &str) -> bool {
    matches!(
        property,
        "width"
            | "height"
            | "min-width"
            | "max-width"
            | "min-height"
            | "max-height"
            | "padding"
            | "x"
            | "y"
            | "gap"
            | "font-size"
            | "line-height"
            | "letter-spacing"
            | "border-width"
            | "border-radius"
    )
}

#[cfg(test)]
mod tests {
    use weave::{Viewport, parse};

    use super::{ComputedStyle, Length};

    const DESKTOP: Viewport = Viewport {
        width: 1_440.0,
        height: 900.0,
    };

    #[test]
    fn lengths_keep_their_units_until_the_viewport_is_known() {
        let viewport = Viewport {
            width: 1_200.0,
            height: 800.0,
        };
        assert_eq!(Length::parse("0.5"), Some(Length::Overlay(0.5)));
        assert_eq!(
            Length::parse("200px").and_then(|value| value.resolve(viewport, None)),
            Some(0.5)
        );
        assert_eq!(
            Length::parse("10vw").and_then(|value| value.resolve(viewport, None)),
            Some(0.3)
        );
        assert_eq!(
            Length::parse("25vh").and_then(|value| value.resolve(viewport, None)),
            Some(0.5)
        );
        assert_eq!(
            Length::parse("50%").and_then(|value| value.resolve(viewport, Some(0.8))),
            Some(0.4)
        );
        assert_eq!(
            Length::parse("50%").and_then(|value| value.resolve(viewport, None)),
            None
        );
    }

    #[test]
    fn non_finite_lengths_are_rejected() {
        assert_eq!(Length::parse("NaNpx"), None);
        assert_eq!(Length::parse("inf"), None);
    }

    /// One element with an ID and nothing else, to cascade against.
    struct Lone;

    impl weave::Tree for Lone {
        fn element(&self, _: usize) -> weave::Element<'_> {
            weave::Element {
                id: "panel",
                classes: &[],
                names: &[],
                states: weave::States::NONE,
            }
        }

        fn parent(&self, _: usize) -> Option<usize> {
            None
        }
    }

    fn style(sheet: &str) -> ComputedStyle {
        let sheet = parse(sheet).expect("stylesheet parses");
        ComputedStyle::from_computed(&weave::cascade(&sheet, &Lone, 0, DESKTOP, None))
    }

    #[test]
    fn invalid_known_lengths_are_kept_for_diagnostics() {
        let style = style("#panel { width: nope; }");
        assert_eq!(style.get("width"), Some("nope"));
        assert_eq!(style.length("width"), Some(Err("nope")));
    }

    #[test]
    fn inactive_media_rules_never_enter_the_computed_style() {
        let style = style(
            r#"
                #panel { width: 600px; }
                @media (orientation: portrait) {
                    #panel { width: 90vw; }
                }
            "#,
        );
        assert_eq!(style.get("width"), Some("600px"));
        assert_eq!(style.length("width"), Some(Ok(Length::Pixels(600.0))));
    }
}
