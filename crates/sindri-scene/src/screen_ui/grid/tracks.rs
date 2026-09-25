//! One grid track, and how a line of them is sized.

use std::fmt;

use serde::{Deserialize, Deserializer, de};

/// How big one row or column is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiTrack {
    /// A length in overlay units.
    Fixed(f32),
    /// As big as the largest child that sits in it alone.
    Auto,
    /// A share of the room the fixed and `auto` tracks leave.
    Fraction(f32),
}

impl UiTrack {
    /// Reads a track as it is stored: `"1fr"`, `"auto"`, or a number.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text == "auto" {
            return Some(Self::Auto);
        }
        if let Some(share) = text.strip_suffix("fr") {
            let share: f32 = share.trim().parse().ok()?;
            return (share.is_finite() && share >= 0.0).then_some(Self::Fraction(share));
        }
        let length: f32 = text.parse().ok()?;
        (length.is_finite() && length >= 0.0).then_some(Self::Fixed(length))
    }
}

impl fmt::Display for UiTrack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(length) => write!(formatter, "{length}"),
            Self::Auto => formatter.write_str("auto"),
            Self::Fraction(share) => write!(formatter, "{share}fr"),
        }
    }
}

impl<'de> Deserialize<'de> for UiTrack {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            de::Error::custom(format!(
                "`{text}` is not a grid track: use \"1fr\", \"auto\" or a length"
            ))
        })
    }
}

/// Sizes a line of tracks within `room`.
///
/// `content` is, per track, the largest single-track child in it, which is
/// what an `auto` track is. Fixed tracks are their length; `fr` tracks share
/// what fixed and `auto` tracks and the gaps leave, and are at least their
/// content, as CSS's `1fr` means `minmax(auto, 1fr)`. With no room to share
/// (`room` of `None`, a grid sizing itself), an `fr` track is its content.
pub(super) fn size_line(
    tracks: &[UiTrack],
    content: &[f32],
    room: Option<f32>,
    gap: f32,
) -> Vec<f32> {
    let mut sizes: Vec<f32> = tracks
        .iter()
        .zip(content)
        .map(|(track, content)| match track {
            UiTrack::Fixed(length) => *length,
            UiTrack::Auto | UiTrack::Fraction(_) => *content,
        })
        .collect();
    let Some(room) = room else {
        return sizes;
    };
    let shares: f32 = tracks
        .iter()
        .map(|track| match track {
            UiTrack::Fraction(share) => *share,
            _ => 0.0,
        })
        .sum();
    if shares <= 0.0 {
        return sizes;
    }
    #[allow(clippy::cast_precision_loss)]
    let gaps = gap * tracks.len().saturating_sub(1) as f32;
    let taken: f32 = tracks
        .iter()
        .zip(&sizes)
        .filter(|(track, _)| !matches!(track, UiTrack::Fraction(_)))
        .map(|(_, size)| *size)
        .sum();
    let free = (room - gaps - taken).max(0.0);
    for (track, size) in tracks.iter().zip(&mut sizes) {
        if let UiTrack::Fraction(share) = track {
            *size = (free * share / shares).max(*size);
        }
    }
    sizes
}

#[cfg(test)]
mod tests {
    use super::{UiTrack, size_line};

    #[test]
    fn tracks_read_as_css_writes_them() {
        assert_eq!(UiTrack::parse("1fr"), Some(UiTrack::Fraction(1.0)));
        assert_eq!(UiTrack::parse("2.5fr"), Some(UiTrack::Fraction(2.5)));
        assert_eq!(UiTrack::parse("auto"), Some(UiTrack::Auto));
        assert_eq!(UiTrack::parse("0.4"), Some(UiTrack::Fixed(0.4)));
        assert_eq!(UiTrack::parse("-1"), None);
        assert_eq!(UiTrack::parse("wide"), None);
    }

    #[test]
    fn fractions_share_what_fixed_auto_and_gaps_leave() {
        let tracks = [
            UiTrack::Fixed(1.0),
            UiTrack::Auto,
            UiTrack::Fraction(1.0),
            UiTrack::Fraction(3.0),
        ];
        // 10 wide, gaps of 0.5 thrice, 1 fixed, 2 auto: 5.5 to share 1:3.
        let sizes = size_line(&tracks, &[0.0, 2.0, 0.0, 0.0], Some(10.0), 0.5);
        let want = [1.0, 2.0, 5.5 / 4.0, 5.5 * 3.0 / 4.0];
        assert!(
            sizes.iter().zip(want).all(|(a, b)| (a - b).abs() < 1.0e-5),
            "{sizes:?}"
        );
    }

    #[test]
    fn a_fraction_is_never_smaller_than_what_is_in_it() {
        let sizes = size_line(&[UiTrack::Fraction(1.0)], &[3.0], Some(1.0), 0.0);
        assert!((sizes[0] - 3.0).abs() < 1.0e-6);
    }
}
