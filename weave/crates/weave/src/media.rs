//! `@media` queries: which viewports a block of rules applies to.

use crate::{ParseError, Viewport};

/// One condition on the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCondition {
    MaxWidth(f32),
    MinWidth(f32),
    MaxHeight(f32),
    MinHeight(f32),
    Portrait,
    Landscape,
}

impl MediaCondition {
    #[must_use]
    pub fn matches(self, viewport: Viewport) -> bool {
        match self {
            Self::MaxWidth(width) => viewport.width <= width,
            Self::MinWidth(width) => viewport.width >= width,
            Self::MaxHeight(height) => viewport.height <= height,
            Self::MinHeight(height) => viewport.height >= height,
            Self::Portrait => viewport.height >= viewport.width,
            Self::Landscape => viewport.width > viewport.height,
        }
    }
}

/// A whole query: `(min-width: 600px) and (orientation: landscape), (max-width: 400px)`.
///
/// Commas separate alternatives, any of which may match; `and` joins
/// conditions that must all hold. As in CSS.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaQuery {
    pub any_of: Vec<Vec<MediaCondition>>,
}

impl MediaQuery {
    #[must_use]
    pub fn matches(&self, viewport: Viewport) -> bool {
        self.any_of
            .iter()
            .any(|all| all.iter().all(|condition| condition.matches(viewport)))
    }
}

pub(crate) fn parse_query(query: &str) -> Result<MediaQuery, ParseError> {
    let any_of = query
        .split(',')
        .map(|alternative| {
            alternative
                .split(" and ")
                .map(|condition| parse_condition(condition.trim()))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MediaQuery { any_of })
}

fn parse_condition(condition: &str) -> Result<MediaCondition, ParseError> {
    let unsupported = || ParseError::UnsupportedMedia(condition.to_owned());
    let inner = condition
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(unsupported)?;
    let (feature, value) = inner.split_once(':').ok_or_else(unsupported)?;
    let value = value.trim();
    match feature.trim() {
        "orientation" => match value {
            "portrait" => Ok(MediaCondition::Portrait),
            "landscape" => Ok(MediaCondition::Landscape),
            _ => Err(unsupported()),
        },
        "max-width" => pixels(value).map(MediaCondition::MaxWidth),
        "min-width" => pixels(value).map(MediaCondition::MinWidth),
        "max-height" => pixels(value).map(MediaCondition::MaxHeight),
        "min-height" => pixels(value).map(MediaCondition::MinHeight),
        _ => Err(unsupported()),
    }
}

fn pixels(value: &str) -> Result<f32, ParseError> {
    let number = value.strip_suffix("px").unwrap_or(value).trim();
    number
        .parse::<f32>()
        .ok()
        .filter(|parsed| parsed.is_finite())
        .ok_or_else(|| ParseError::InvalidMediaWidth(number.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: Viewport = Viewport {
        width: 390.0,
        height: 844.0,
    };
    const DESKTOP: Viewport = Viewport {
        width: 1440.0,
        height: 900.0,
    };

    #[test]
    fn and_needs_both_and_a_comma_needs_either() {
        let wide_landscape =
            parse_query("(min-width: 800px) and (orientation: landscape)").unwrap();
        assert!(wide_landscape.matches(DESKTOP));
        assert!(!wide_landscape.matches(PHONE));

        let short_or_narrow = parse_query("(max-height: 500px), (max-width: 400px)").unwrap();
        assert!(short_or_narrow.matches(PHONE));
        assert!(!short_or_narrow.matches(DESKTOP));
    }

    #[test]
    fn unknown_features_are_refused() {
        assert!(matches!(
            parse_query("(prefers-color-scheme: dark)"),
            Err(ParseError::UnsupportedMedia(_))
        ));
    }
}
