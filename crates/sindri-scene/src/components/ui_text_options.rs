//! The stored spellings of a text component's options, and what they mean.
//!
//! The renderer already has types for all of this — [`sindri_render::TextWrap`]
//! and friends — but `sindri-render` takes no serde: it depends on wgpu, glam
//! and bytemuck, and `sindri-scene` is the seam where a drawn thing becomes a
//! stored one. So the names a scene file holds live here, and each type says
//! which renderer value it is. That is the same arrangement `UiAnchor` already
//! has with `TextAlign`, and it is what keeps a rename in a scene file from
//! being a change to the renderer.
//!
//! Every option defaults to the behaviour text had before it existed, so an
//! older scene that says none of this draws exactly what it drew.

use serde::Deserialize;
use sindri_render::{LineAlign, TextCase, TextShadow, TextStroke, TextWrap};

use super::opaque_white;

/// What happens to a line too long for its box.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiTextWrap {
    /// Runs on past the edge. What a HUD reading wants: a score is one word and
    /// breaking it is worse than overflowing.
    #[default]
    None,
    /// Breaks between words, and inside a word only when it cannot fit alone.
    Word,
    /// Breaks wherever it has to — for a language without spaces, or a box too
    /// narrow to respect words in.
    Glyph,
}

impl UiTextWrap {
    /// Every mode, in the order a chooser should offer them.
    pub const ALL: [Self; 3] = [Self::None, Self::Word, Self::Glyph];

    /// The name this mode is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Word => "word",
            Self::Glyph => "glyph",
        }
    }

    #[must_use]
    pub const fn wrap(self) -> TextWrap {
        match self {
            Self::None => TextWrap::None,
            Self::Word => TextWrap::Word,
            Self::Glyph => TextWrap::Glyph,
        }
    }
}

/// Where each line sits across its box.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiTextLineAlign {
    /// The lines line up the way the anchor does, which is what almost every
    /// label wants: a centred title's lines are centred.
    #[default]
    Follow,
    Left,
    Center,
    Right,
    /// Spread to both edges, leaving the last line alone.
    Justify,
}

impl UiTextLineAlign {
    /// Every alignment, in the order a chooser should offer them.
    pub const ALL: [Self; 5] = [
        Self::Follow,
        Self::Left,
        Self::Center,
        Self::Right,
        Self::Justify,
    ];

    /// The name this alignment is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Follow => "follow",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Justify => "justify",
        }
    }

    #[must_use]
    pub const fn line_align(self) -> LineAlign {
        match self {
            Self::Follow => LineAlign::Follow,
            Self::Left => LineAlign::Left,
            Self::Center => LineAlign::Center,
            Self::Right => LineAlign::Right,
            Self::Justify => LineAlign::Justify,
        }
    }
}

/// Whether the words are drawn as written, or shouted, or hushed.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiTextCase {
    #[default]
    AsWritten,
    Upper,
    Lower,
}

impl UiTextCase {
    /// Every case, in the order a chooser should offer them.
    pub const ALL: [Self; 3] = [Self::AsWritten, Self::Upper, Self::Lower];

    /// The name this case is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AsWritten => "as_written",
            Self::Upper => "upper",
            Self::Lower => "lower",
        }
    }

    #[must_use]
    pub const fn case(self) -> TextCase {
        match self {
            Self::AsWritten => TextCase::AsWritten,
            Self::Upper => TextCase::Upper,
            Self::Lower => TextCase::Lower,
        }
    }
}

/// A stroke drawn around the glyphs.
///
/// The width is in the overlay's units like every other size on the component,
/// so an outline stays the same share of a letter as the letter changes size.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiTextOutline {
    /// How far the stroke reaches out from the edge. Zero draws none.
    #[serde(default)]
    pub width: f32,
    #[serde(default = "black")]
    pub color: [f32; 4],
}

impl Default for UiTextOutline {
    fn default() -> Self {
        Self {
            width: 0.0,
            color: black(),
        }
    }
}

impl UiTextOutline {
    #[must_use]
    pub const fn stroke(self) -> TextStroke {
        TextStroke {
            width: self.width,
            color: self.color,
        }
    }
}

/// A copy of the text drawn behind itself, offset and softened.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiTextShadow {
    /// How far the shadow is displaced, in the overlay's units. Positive Y is
    /// up, as everywhere else, so a shadow below reads negative.
    #[serde(default)]
    pub offset: [f32; 2],
    #[serde(default = "black")]
    pub color: [f32; 4],
    /// How far the shadow's edge is spread. Zero is a hard copy of the letter.
    #[serde(default)]
    pub softness: f32,
}

impl Default for UiTextShadow {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            color: black(),
            softness: 0.0,
        }
    }
}

impl UiTextShadow {
    #[must_use]
    pub const fn shadow(self) -> TextShadow {
        TextShadow {
            offset: self.offset,
            color: self.color,
            softness: self.softness,
        }
    }
}

/// Shrinking the words to fit the box they were given.
///
/// Off by default, because a size an author typed should be the size they get.
/// Turned on, it is the answer to the one thing a fixed size cannot do: hold a
/// translated string, or a player's name, inside a button drawn for something
/// shorter.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiTextAutoSize {
    #[serde(default)]
    pub enabled: bool,
    /// The smallest size it may shrink to. The largest is the authored
    /// `font_size`, so turning this on can only ever make text smaller — which
    /// is what keeps a label from growing over the art beside it.
    #[serde(default = "default_min_size")]
    pub min: f32,
}

impl Default for UiTextAutoSize {
    fn default() -> Self {
        Self {
            enabled: false,
            min: default_min_size(),
        }
    }
}

const fn default_min_size() -> f32 {
    0.02
}

/// Opaque black, which is what an outline and a shadow want to be.
const fn black() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

/// Kept so the module that owns the white default is the one that defines it.
#[allow(dead_code)]
const fn unused_white() -> [f32; 4] {
    opaque_white()
}

#[cfg(test)]
mod tests {
    use super::{
        UiTextAutoSize, UiTextCase, UiTextLineAlign, UiTextOutline, UiTextShadow, UiTextWrap,
    };
    use sindri_render::{LineAlign, TextCase, TextWrap};

    /// Every stored name round-trips to the renderer value it stands for, and
    /// nothing in the list is spelled twice.
    #[test]
    fn every_stored_name_means_one_thing() {
        let names: Vec<&str> = UiTextWrap::ALL.iter().map(|mode| mode.as_str()).collect();
        assert_eq!(names, ["none", "word", "glyph"]);
        assert_eq!(UiTextWrap::Word.wrap(), TextWrap::Word);
        assert_eq!(UiTextLineAlign::Justify.line_align(), LineAlign::Justify);
        assert_eq!(UiTextCase::Upper.case(), TextCase::Upper);

        let names: Vec<&str> = UiTextLineAlign::ALL.iter().map(|a| a.as_str()).collect();
        assert_eq!(names.len(), 5);
        assert!(names.iter().all(|name| !name.is_empty()));
    }

    /// Every option's default is the behaviour text had before the option
    /// existed, which is what lets an older scene keep drawing what it drew.
    #[test]
    fn nothing_is_on_until_it_is_asked_for() {
        assert_eq!(UiTextWrap::default(), UiTextWrap::None);
        assert_eq!(UiTextLineAlign::default(), UiTextLineAlign::Follow);
        assert_eq!(UiTextCase::default(), UiTextCase::AsWritten);
        assert!(!UiTextOutline::default().stroke().is_drawn());
        assert!(!UiTextShadow::default().shadow().is_drawn());
        assert!(!UiTextAutoSize::default().enabled);
    }

    /// A scene that says nothing about these fields still deserializes, which
    /// is the whole reason none of this needed a format version.
    #[test]
    fn an_older_payload_still_reads() {
        let outline: UiTextOutline = serde_json::from_str("{}").expect("all defaulted");
        assert_eq!(outline, UiTextOutline::default());
        let shadow: UiTextShadow = serde_json::from_str("{}").expect("all defaulted");
        assert_eq!(shadow, UiTextShadow::default());
        let fit: UiTextAutoSize = serde_json::from_str("{}").expect("all defaulted");
        assert_eq!(fit, UiTextAutoSize::default());
    }
}
