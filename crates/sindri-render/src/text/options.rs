//! The knobs a string carries besides its words.
//!
//! Separate types rather than a dozen more fields on [`super::TextInstance`],
//! because they group the way an author thinks about them: how the letters are
//! shaped, how the block is laid out in its box, and what is drawn around the
//! glyphs. Each one is `Default` and each default is "as if this feature did not
//! exist", so a caller that wants none of it writes none of it.

/// Which end of a string the point it was given belongs to.
///
/// A string is laid out from a corner, so a point alone does not say where it
/// goes: a title told to sit at the middle of the screen had its *top-left* put
/// there and ran off to the right. Every other element is placed by its centre,
/// so the anchor an author chose meant one thing for an image and something else
/// for the words on it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAlign {
    /// The point is where the string begins — its left edge, or its top.
    #[default]
    Start,
    /// The point is the middle of the string.
    Middle,
    /// The point is where the string ends — its right edge, or its bottom.
    End,
}

impl TextAlign {
    /// How far right of the point a string of this width starts.
    pub(super) fn start_after(self, width: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Middle => -width * 0.5,
            Self::End => -width,
        }
    }

    /// How far *above* the point the top of a string of this height sits.
    ///
    /// Text is laid out downwards while the units it is placed in run upwards,
    /// so this is the one place that flip lives. Reading it off `start_after`
    /// with a minus sign is exactly the sort of thing that ends up written once
    /// per caller and once wrong.
    pub(super) fn top_above(self, height: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Middle => height * 0.5,
            Self::End => height,
        }
    }

    /// How far into `outer` a run of `inner` sits when aligned this way.
    ///
    /// What puts a block inside a box, on either axis, measured in the
    /// direction the axis runs.
    pub(super) fn inset(self, outer: f32, inner: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Middle => (outer - inner) * 0.5,
            Self::End => outer - inner,
        }
    }
}

/// How the letters themselves are picked from the face.
///
/// A real weight and slant asked of the font rather than a transform applied to
/// the glyphs: a face's own bold has different letterforms, not thicker ones,
/// and faking it is how text ends up looking like text in a game jam.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
}

/// Whether the words are drawn as written, or shouted, or hushed.
///
/// Applied to the string before shaping, so it is the drawn text that changes
/// and not a per-glyph substitution — which means it composes with wrapping and
/// with the visible-character count instead of fighting them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextCase {
    #[default]
    AsWritten,
    Upper,
    Lower,
}

impl TextCase {
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

    /// The string as it should be shaped.
    #[must_use]
    pub fn applied(self, text: &str) -> std::borrow::Cow<'_, str> {
        match self {
            Self::AsWritten => std::borrow::Cow::Borrowed(text),
            Self::Upper => std::borrow::Cow::Owned(text.to_uppercase()),
            Self::Lower => std::borrow::Cow::Owned(text.to_lowercase()),
        }
    }
}

/// What happens to a line too long for its box.
///
/// Only meaningful with a box to be too long for: with no width, nothing wraps
/// and every mode draws the same string.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextWrap {
    /// Runs on past the edge. What a HUD reading wants — a score is one word and
    /// breaking it is worse than overflowing.
    #[default]
    None,
    /// Breaks between words, and inside a word only when it cannot fit alone.
    Word,
    /// Breaks wherever it has to. For a language without spaces, or a box too
    /// narrow to respect words in.
    Glyph,
}

impl TextWrap {
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
}

/// Where each line sits across its box, when that differs from where the block
/// sits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineAlign {
    /// The lines line up the way the block does, which is what almost every
    /// label wants: a centred title's lines are centred.
    #[default]
    Follow,
    Left,
    Center,
    Right,
    /// Spread to both edges, leaving the last line alone. Needs a box width;
    /// without one there is no second edge to reach.
    Justify,
}

impl LineAlign {
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
}

/// A stroke drawn around the glyphs.
///
/// The width is in the same units everything else here is, so an outline stays
/// the same share of a letter as the letter changes size — which is the whole
/// reason it is a distance from the edge rather than a count of pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextStroke {
    pub width: f32,
    pub color: [f32; 4],
}

impl TextStroke {
    /// Whether this stroke would put anything on the screen.
    #[must_use]
    pub fn is_drawn(self) -> bool {
        self.width > 0.0 && self.color[3] > 0.0
    }
}

/// A copy of the text drawn behind itself, offset and softened.
///
/// The cheapest thing that makes a label readable over a busy scene, and the
/// reason it belongs here rather than in a game's own code: it is the same
/// glyphs through the same field, one threshold wider.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextShadow {
    /// How far the shadow is displaced, in the units the text is placed in.
    /// Positive Y is up, as everywhere else, so a shadow below reads negative.
    pub offset: [f32; 2],
    pub color: [f32; 4],
    /// How far the shadow's edge is spread, in those same units. Zero is a hard
    /// copy of the letter; anything more is a blur.
    pub softness: f32,
}

impl TextShadow {
    /// Whether this shadow would put anything on the screen.
    ///
    /// A shadow at no offset and no softness is exactly behind the text and
    /// invisible, so it is not drawn: the alternative is every label in a scene
    /// paying for a second set of glyphs that cannot be seen.
    #[must_use]
    pub fn is_drawn(self) -> bool {
        self.color[3] > 0.0 && (self.offset != [0.0, 0.0] || self.softness > 0.0)
    }
}

/// The range of sizes a string may pick from to fit its box.
///
/// Auto-sizing is the answer to the one thing a fixed size cannot do: hold a
/// translated string, or a player's name, inside a button that was drawn for
/// something shorter. The box is the element's own, so it is bounded by what the
/// author drew rather than by a guess.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFit {
    pub min: f32,
    pub max: f32,
}

impl TextFit {
    /// A fit whose bounds make sense, or `None`.
    ///
    /// A range that is empty or not finite would send the search looking for a
    /// size between two numbers with nothing between them.
    #[must_use]
    pub fn checked(min: f32, max: f32) -> Option<Self> {
        (min.is_finite() && max.is_finite() && min > 0.0 && max >= min).then_some(Self { min, max })
    }
}

#[cfg(test)]
mod tests {
    use super::{LineAlign, TextAlign, TextCase, TextFit, TextShadow, TextStroke, TextWrap};

    #[test]
    fn an_alignment_says_how_far_back_a_string_starts() {
        assert!((TextAlign::Start.start_after(1.2) - 0.0).abs() < f32::EPSILON);
        assert!((TextAlign::Middle.start_after(1.2) + 0.6).abs() < f32::EPSILON);
        assert!((TextAlign::End.start_after(1.2) + 1.2).abs() < f32::EPSILON);
    }

    /// Down runs the other way from across, because layout goes down the page
    /// and the units a string is placed in go up it.
    #[test]
    fn the_top_of_a_string_is_above_the_point_it_was_given() {
        assert!((TextAlign::Start.top_above(1.2) - 0.0).abs() < f32::EPSILON);
        assert!((TextAlign::Middle.top_above(1.2) - 0.6).abs() < f32::EPSILON);
        assert!((TextAlign::End.top_above(1.2) - 1.2).abs() < f32::EPSILON);
    }

    /// A string of no size sits at its point however it is aligned, so an empty
    /// label does not jump about.
    #[test]
    fn nothing_is_in_the_same_place_whichever_end_it_is_measured_from() {
        for align in [TextAlign::Start, TextAlign::Middle, TextAlign::End] {
            assert!(align.start_after(0.0).abs() < f32::EPSILON, "{align:?}");
            assert!(align.top_above(0.0).abs() < f32::EPSILON, "{align:?}");
        }
    }

    /// Putting a block in a box: hard against one edge, centred, or hard against
    /// the other.
    #[test]
    fn a_block_sits_where_its_alignment_puts_it_in_its_box() {
        assert!((TextAlign::Start.inset(10.0, 4.0) - 0.0).abs() < f32::EPSILON);
        assert!((TextAlign::Middle.inset(10.0, 4.0) - 3.0).abs() < f32::EPSILON);
        assert!((TextAlign::End.inset(10.0, 4.0) - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn case_is_applied_to_the_words_rather_than_the_glyphs() {
        assert_eq!(TextCase::AsWritten.applied("Start"), "Start");
        assert_eq!(TextCase::Upper.applied("Start"), "START");
        assert_eq!(TextCase::Lower.applied("Start"), "start");
    }

    /// Nothing invisible is drawn. A shadow exactly behind its text and a
    /// stroke of no width are the two ways to ask for a second set of glyphs
    /// that cannot be seen, and every label in a scene would pay for them.
    #[test]
    fn an_effect_that_could_not_be_seen_is_not_drawn() {
        assert!(!TextStroke::default().is_drawn());
        assert!(
            !TextStroke {
                width: 0.01,
                color: [1.0, 1.0, 1.0, 0.0]
            }
            .is_drawn()
        );
        assert!(
            TextStroke {
                width: 0.01,
                color: [1.0; 4]
            }
            .is_drawn()
        );

        assert!(!TextShadow::default().is_drawn());
        assert!(
            !TextShadow {
                offset: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 1.0],
                softness: 0.0,
            }
            .is_drawn(),
            "a hard shadow at no offset is exactly behind the text"
        );
        assert!(
            TextShadow {
                offset: [0.0, -0.01],
                color: [0.0, 0.0, 0.0, 1.0],
                softness: 0.0,
            }
            .is_drawn()
        );
    }

    /// A fit needs a range with something in it.
    #[test]
    fn a_fit_refuses_a_range_it_could_not_search() {
        assert!(TextFit::checked(0.02, 0.2).is_some());
        assert!(TextFit::checked(0.2, 0.2).is_some(), "one size is a range");
        assert!(TextFit::checked(0.2, 0.02).is_none());
        assert!(TextFit::checked(0.0, 0.2).is_none());
        assert!(TextFit::checked(f32::NAN, 0.2).is_none());
    }

    /// Every choice a scene can store is one the engine can name back.
    #[test]
    fn every_choice_has_a_stored_name() {
        assert!(TextWrap::ALL.iter().all(|mode| !mode.as_str().is_empty()));
        assert!(LineAlign::ALL.iter().all(|how| !how.as_str().is_empty()));
        assert!(TextCase::ALL.iter().all(|case| !case.as_str().is_empty()));
    }
}
