//! `sindri.ui.*`: what a viewport draws on top of the world.
//!
//! One family, one rule: everything here is placed against the viewport rather
//! than in the scene. A UI component is anchored to an edge of the screen, is
//! drawn through a projection the viewport owns, and no authored camera can
//! move it or lose it behind geometry. That is what makes a UI entity a
//! different kind of thing from a world entity, and it is why these are their
//! own components instead of a `space` field on the world ones: the difference
//! is not a value a component holds, it is which fields the component has.

use serde::Deserialize;
use sindri_core::{SceneComponent, SpriteRef, SpriteRefError};
use sindri_render::{TextAlign, TextError, TextFit, TextInstance, TextStyle};

use super::opaque_white;
use super::ui_text_options::{
    UiTextAutoSize, UiTextCase, UiTextLineAlign, UiTextOutline, UiTextShadow, UiTextWrap,
};
use super::ui_text_template;

/// Where a UI element's origin sits inside the viewport.
///
/// Anchoring is resolved against the viewport extent, so an element keeps its
/// relationship to an edge as the window changes shape. The entity's transform
/// is read as an offset from the anchor, which is what lets a HUD row be five
/// entities at five X offsets from one corner.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiAnchor {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl UiAnchor {
    /// Every anchor, in the order a chooser should offer them.
    ///
    /// Named here rather than in whatever draws the list, so an anchor added
    /// to the enum appears in the editor without anyone remembering to add it
    /// twice.
    pub const ALL: [Self; 9] = [
        Self::Center,
        Self::Top,
        Self::Bottom,
        Self::Left,
        Self::Right,
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    /// The name this anchor is stored under, which is the one a payload holds.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }

    /// Which end of a string anchored here its position names.
    ///
    /// The same corner the anchor already means: a label anchored top-left
    /// begins at that corner and runs right and down, and one anchored centre
    /// is centred on it. Derived rather than authored, because an anchor that
    /// meant one thing for an image and another for its words is the mismatch
    /// this is fixing.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn text_align(self) -> [TextAlign; 2] {
        let [across, down] = self.unit_offset();
        // Up is positive in the overlay and down the page is positive for a
        // string, so the vertical pair is the other way round.
        let horizontal = match across {
            x if x < 0.0 => TextAlign::Start,
            x if x > 0.0 => TextAlign::End,
            _ => TextAlign::Middle,
        };
        let vertical = match down {
            y if y > 0.0 => TextAlign::Start,
            y if y < 0.0 => TextAlign::End,
            _ => TextAlign::Middle,
        };
        [horizontal, vertical]
    }

    /// The anchor as a fraction of the half-extent, in `[-1, 1]` per axis.
    #[must_use]
    pub const fn unit_offset(self) -> [f32; 2] {
        match self {
            Self::Center => [0.0, 0.0],
            Self::Top => [0.0, 1.0],
            Self::Bottom => [0.0, -1.0],
            Self::Left => [-1.0, 0.0],
            Self::Right => [1.0, 0.0],
            Self::TopLeft => [-1.0, 1.0],
            Self::TopRight => [1.0, 1.0],
            Self::BottomLeft => [-1.0, -1.0],
            Self::BottomRight => [1.0, -1.0],
        }
    }
}

/// An image drawn on the viewport: a HUD pip, a banner, a crosshair.
///
/// Its Z says how far back in the stack it sits and nothing else: it orders the
/// element without moving it, so no HUD can be lost off a camera's far plane by
/// someone typing a big number.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiImageComponent {
    pub texture: String,
    #[serde(default)]
    pub anchor: UiAnchor,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    /// The explicit override on draw order within the overlay.
    #[serde(default)]
    pub layer: i32,
    /// How much of the element is drawn, in `[0, 1]`, and from which edge.
    ///
    /// This is what makes a health bar a bar rather than a picture of one. A
    /// script sets the fraction with `Ui.set_fill`; the element keeps its
    /// authored rect and draws a part of it, so the empty part of the bar is
    /// wherever the full one was rather than the bar shrinking towards its
    /// middle.
    ///
    /// It clips the image with it: a bar at a third shows the left third of its
    /// texture, not the whole texture squashed. A stretched picture would make
    /// a segmented or lettered bar wrong, and a filling bar is the only reason
    /// this field exists.
    #[serde(default)]
    pub fill: UiFill,
}

/// How much of an element is drawn, and which way it empties.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiFill {
    /// The fraction drawn, clamped to `[0, 1]` when it is read.
    #[serde(default = "full")]
    pub amount: f32,
    /// The edge the drawn part keeps.
    #[serde(default)]
    pub from: UiFillEdge,
}

impl Default for UiFill {
    fn default() -> Self {
        Self {
            amount: 1.0,
            from: UiFillEdge::default(),
        }
    }
}

const fn full() -> f32 {
    1.0
}

impl UiFill {
    /// The fraction, made usable: clamped, and with a NaN read as empty.
    ///
    /// Empty rather than full for a NaN, so a bar driven by a broken
    /// calculation reads as obviously wrong instead of as perfect health.
    #[must_use]
    pub fn fraction(self) -> f32 {
        if self.amount.is_nan() {
            0.0
        } else {
            self.amount.clamp(0.0, 1.0)
        }
    }

    /// The drawn rect within the element's own unit square, as
    /// `(offset, scale)` per axis, with the origin at the element's centre.
    ///
    /// Returned rather than applied here because the same numbers drive both
    /// the quad and the texture coordinates, and computing them twice is how
    /// the two drift apart.
    #[must_use]
    pub fn sub_rect(self) -> ([f32; 2], [f32; 2]) {
        let kept = self.fraction();
        let shift = (1.0 - kept) / 2.0;
        match self.from {
            UiFillEdge::Left => ([-shift, 0.0], [kept, 1.0]),
            UiFillEdge::Right => ([shift, 0.0], [kept, 1.0]),
            UiFillEdge::Bottom => ([0.0, -shift], [1.0, kept]),
            UiFillEdge::Top => ([0.0, shift], [1.0, kept]),
        }
    }
}

/// The edge a partly-filled element keeps.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiFillEdge {
    /// Empties rightwards, which is what a health bar does.
    #[default]
    Left,
    Right,
    Bottom,
    Top,
}

impl UiFillEdge {
    /// Every edge, in the order a chooser should offer them.
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Bottom, Self::Top];

    /// The name this edge is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Top => "top",
        }
    }
}

impl UiImageComponent {
    /// The texture this element draws, and which named part of it.
    ///
    /// The same reference a world sprite uses, checked the same way and for the
    /// same reason: a scene carrying a bad reference has to open, because the
    /// editor is where it gets fixed.
    pub fn reference(&self) -> Result<SpriteRef, SpriteRefError> {
        SpriteRef::parse(&self.texture)
    }
}

impl SceneComponent for UiImageComponent {
    const TYPE_NAME: &'static str = "sindri.ui.image";
}

/// Text drawn on the viewport, anchored the same way an image is.
///
/// The font is a project asset reference rather than a family installed on the
/// machine. That keeps a scene reproducible across the editor, captures, and
/// the browser: a host binds the bytes at that reference before drawing.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiTextComponent {
    /// The words, with `{}` where a script's numbers go.
    ///
    /// A template rather than a finished string, because Decay has no string
    /// concatenation and a HUD that cannot show a number is not a HUD. See
    /// `ui_text_template` for why the scene owns the words and the script owns
    /// the numbers.
    pub text: String,
    pub font: String,
    /// How tall the text is, in the overlay's units.
    ///
    /// The same units the element's own transform uses: two is the full height
    /// of the screen whatever the screen is, so a size here is a share of it
    /// rather than a count of pixels. That is what makes a HUD authored on a
    /// desktop readable on a phone — and it is what this field was not, which
    /// cost a shipped build every word on its screen.
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "opaque_white")]
    pub color: [f32; 4],
    #[serde(default)]
    pub anchor: UiAnchor,
    #[serde(default)]
    pub layer: i32,
    /// What fills the template's slots, left to right.
    ///
    /// Authored values are a designer's preview — a scene opens showing
    /// `Score: 1200` rather than a row of braces — and a script overwrites them
    /// with `Ui.set_number`. Slots past the end read as zero.
    #[serde(default)]
    pub values: Vec<f32>,

    /// The rect the words are laid out in, in the overlay's units.
    ///
    /// Zero on an axis means unbounded there, which is what a HUD reading wants:
    /// a score has no box and should be neither wrapped nor shrunk to fit one.
    /// A width is what wrapping needs; a height is what auto-sizing needs.
    ///
    /// Given a box, the box takes the place the words used to: it is placed by
    /// the anchor, and the words sit inside it. That is what makes "top left,
    /// box 0.8 wide" mean the same thing for a paragraph as for the panel behind
    /// it.
    #[serde(default)]
    pub bounds: [f32; 2],
    #[serde(default)]
    pub wrap: UiTextWrap,
    /// Where each line sits across the box, when that differs from where the
    /// block sits.
    #[serde(default)]
    pub line_align: UiTextLineAlign,
    /// Extra space after every glyph, in the overlay's units. Negative tightens.
    #[serde(default)]
    pub letter_spacing: f32,
    /// The face's own bold, asked of the font rather than faked by thickening
    /// the glyphs — a real bold has different letterforms.
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub case: UiTextCase,
    /// How many glyphs are drawn, for a reveal. Negative draws them all, which
    /// is the default; a script counts it up for a typewriter.
    #[serde(default = "draw_every_glyph")]
    pub visible: f32,
    #[serde(default)]
    pub outline: UiTextOutline,
    #[serde(default)]
    pub shadow: UiTextShadow,
    #[serde(default)]
    pub auto_size: UiTextAutoSize,
}

impl UiTextComponent {
    /// The words as they should be drawn: the template with its slots filled.
    ///
    /// Everything that draws text goes through this rather than reading `text`,
    /// so a HUD looks the same in the editor viewport, a capture, and the
    /// browser without each of them remembering to format.
    #[must_use]
    pub fn resolved(&self) -> String {
        ui_text_template::fill(&self.text, &self.values)
    }

    /// How many numbers this component's template asks for.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        ui_text_template::slot_count(&self.text)
    }

    /// How many glyphs to draw, or `None` for all of them.
    ///
    /// Stored as a float so a script can drive it the way it drives every other
    /// number — a typewriter is a count rising with time — and read back here so
    /// that the rounding and the "all of them" case are decided once.
    #[must_use]
    pub fn visible_glyphs(&self) -> Option<usize> {
        if self.visible.is_nan() || self.visible < 0.0 {
            return None;
        }
        // Clamped so a script that ran a reveal counter away does not overflow
        // the cast; no string has anything like this many glyphs, so the clamp
        // means "all of them" wherever it bites.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(self.visible.floor().min(1.0e9) as usize)
    }

    /// The range of sizes this text may pick from, if it was told to fit.
    ///
    /// The authored size is the ceiling, so turning auto-size on can only ever
    /// make text smaller — which is what keeps a short label from growing over
    /// the art beside it.
    #[must_use]
    pub fn fit(&self) -> Option<TextFit> {
        self.auto_size
            .enabled
            .then(|| TextFit::checked(self.auto_size.min, self.font_size))
            .flatten()
    }

    /// The weight and slant asked of the face.
    #[must_use]
    pub const fn style(&self) -> TextStyle {
        TextStyle {
            bold: self.bold,
            italic: self.italic,
        }
    }

    /// This component as the renderer takes it, placed at `position`.
    ///
    /// The one place a stored text component becomes a drawable one. It matters
    /// that there is only one: the frame draws from this, and so does the editor
    /// when it works out what a click landed on and where to put a handle. Built
    /// twice, the handle would be around a box the words are not in as soon as
    /// the two copies disagreed about a single option — which, with this many
    /// options, is a matter of time rather than luck.
    pub fn instance(&self, position: [f32; 2]) -> Result<TextInstance, TextError> {
        let mut instance = TextInstance::new(
            self.resolved(),
            &self.font,
            position,
            self.font_size,
            self.line_height,
            self.color,
            self.anchor.text_align(),
        )?
        .in_box(self.bounds, self.wrap.wrap())
        .with_line_align(self.line_align.line_align())
        .with_letter_spacing(self.letter_spacing)
        .with_style(self.style())
        .with_case(self.case.case())
        .with_outline(self.outline.stroke())
        .with_shadow(self.shadow.shadow());
        if let Some(glyphs) = self.visible_glyphs() {
            instance = instance.with_visible_glyphs(glyphs);
        }
        if let Some(fit) = self.fit() {
            instance = instance.fitted(fit);
        }
        Ok(instance)
    }
}

/// A readable default, in the overlay's units.
///
/// Two units is the whole height of the screen, so this is a line a little over
/// three per cent of it — the twenty-four pixels this used to be, on the seven
/// hundred and twenty pixel screen it used to assume, and now that size on any
/// screen rather than that many pixels on every one.
const fn default_font_size() -> f32 {
    0.0667
}

const fn default_line_height() -> f32 {
    0.0833
}

/// The reveal count that reveals everything.
///
/// Negative rather than a sentinel like zero, because zero is a real answer —
/// the first frame of a typewriter, before the first letter — and a field whose
/// "off" value is also a meaningful value cannot express it.
const fn draw_every_glyph() -> f32 {
    -1.0
}

impl SceneComponent for UiTextComponent {
    const TYPE_NAME: &'static str = "sindri.ui.text";
}

#[cfg(test)]
mod anchor_alignment_tests {
    use super::{TextAlign, UiAnchor};

    /// A label reads out of the corner it is anchored to, which is the corner
    /// the anchor already meant for everything that is not text.
    #[test]
    fn an_anchor_says_which_end_of_a_string_its_position_names() {
        for (anchor, expected) in [
            (UiAnchor::Center, [TextAlign::Middle, TextAlign::Middle]),
            (UiAnchor::Top, [TextAlign::Middle, TextAlign::Start]),
            (UiAnchor::Bottom, [TextAlign::Middle, TextAlign::End]),
            (UiAnchor::Left, [TextAlign::Start, TextAlign::Middle]),
            (UiAnchor::Right, [TextAlign::End, TextAlign::Middle]),
            (UiAnchor::TopLeft, [TextAlign::Start, TextAlign::Start]),
            (UiAnchor::TopRight, [TextAlign::End, TextAlign::Start]),
            (UiAnchor::BottomLeft, [TextAlign::Start, TextAlign::End]),
            (UiAnchor::BottomRight, [TextAlign::End, TextAlign::End]),
        ] {
            assert_eq!(anchor.text_align(), expected, "{anchor:?}");
        }
    }

    /// Up is positive in the overlay and down the page is positive for a
    /// string, and getting that backwards puts every top-anchored label off
    /// the top of the screen.
    #[test]
    fn the_vertical_pair_is_the_other_way_round_from_the_anchor() {
        assert!((UiAnchor::Top.unit_offset()[1] - 1.0).abs() < f32::EPSILON);
        assert_eq!(UiAnchor::Top.text_align()[1], TextAlign::Start);
        assert!((UiAnchor::Bottom.unit_offset()[1] + 1.0).abs() < f32::EPSILON);
        assert_eq!(UiAnchor::Bottom.text_align()[1], TextAlign::End);
    }
}
