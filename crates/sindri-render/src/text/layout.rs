//! Where a laid-out string ends up.
//!
//! Arithmetic about boxes and alignment, kept apart from the renderer because
//! none of it needs a GPU or a font to be checked — and because getting it wrong
//! is quiet: a label half a box off centre looks almost right until it is beside
//! something that is not.

#[cfg(test)]
use glyphon::Metrics;
use glyphon::{Buffer, cosmic_text::Align};

use super::instance::TextInstance;
use super::options::{LineAlign, TextAlign};

/// Where a string of this size actually starts, given the point it was told to
/// sit at and which end of it that point names.
///
/// Its own function because it is the whole of what can be got wrong: a string
/// is laid out from its top-left, and every element around it is placed by its
/// centre.
#[must_use]
pub fn aligned_origin(instance: &TextInstance, size: [f32; 2]) -> [f32; 2] {
    let [across, down] = instance.align();
    [
        instance.position()[0] + across.start_after(size[0]),
        instance.position()[1] + down.top_above(size[1]),
    ]
}

/// How big a shaped string turned out, in raster pixels, across and down.
///
/// The width is the longest line rather than the box it was shaped in, which
/// would answer the same for every string.
pub(super) fn laid_out(buffer: &Buffer, line_height: f32) -> [f32; 2] {
    let mut width = 0.0_f32;
    let mut lines = 0.0_f32;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        lines += 1.0;
    }
    [width, lines * line_height]
}

/// One string, shaped and measured.
pub(super) struct LaidOut {
    pub(super) buffer: Buffer,
    /// Whether the buffer was given a width to lay out within.
    ///
    /// It decides where the glyph coordinates are measured from, which is the
    /// one thing that cannot be worked out by looking at them: given a width,
    /// cosmic-text positions each line inside it — a centred line already
    /// carries half the slack — so the quads are placed from the box's edge.
    /// Without one, lines start at zero and the quads are placed from the
    /// block's own edge.
    pub(super) wrapped: bool,
    /// Units of the scene per raster pixel, at the size it was drawn.
    pub(super) scale: f32,
    /// What the words cover, in the units the text is placed in.
    pub(super) block: [f32; 2],
}

/// The top-left corner of the room a string is given, and how big that room is.
///
/// Without a box the room is the words themselves. With one it is the box, and
/// the box is what the anchor places — which is what makes "top left, box 0.8
/// wide" mean the same thing for a paragraph as for the panel behind it.
///
/// One function because the frame, the editor's handle and the pick box all
/// have to agree about it, and three copies only have to disagree once.
pub(super) fn outer_corner(instance: &TextInstance, block: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let [across, down] = instance.align();
    let bounds = instance.bounds();
    let outer = [
        if bounds[0] > 0.0 { bounds[0] } else { block[0] },
        if bounds[1] > 0.0 { bounds[1] } else { block[1] },
    ];
    (
        [
            instance.position()[0] + across.start_after(outer[0]),
            instance.position()[1] + down.top_above(outer[1]),
        ],
        outer,
    )
}

/// The corner the glyph coordinates of one laid-out string are measured from.
///
/// Down is always the same: the room the string was given, and then the block
/// hung inside it by the vertical alignment. Cosmic-text stacks lines from the
/// top of the buffer and does nothing else vertically.
///
/// Across depends on whether the buffer had a width. Without one, lines start at
/// zero and the block is placed by its own measured width — the way an
/// unbounded label has always been placed. With one, cosmic-text has already
/// positioned each line inside that width, so a centred line carries half the
/// slack in its own glyph coordinates; placing the block by its width as well
/// would add that slack a second time and push the words off to one side. This
/// is exactly what it looked like: a centred hint sitting a little right of
/// centre, by half of what its box had spare.
pub(super) fn layout_origin(instance: &TextInstance, laid: &LaidOut) -> [f32; 2] {
    let [across, down] = instance.align();
    let (corner, outer) = outer_corner(instance, laid.block);
    [
        if laid.wrapped {
            corner[0]
        } else {
            corner[0] + across.inset(outer[0], laid.block[0])
        },
        corner[1] - down.inset(outer[1], laid.block[1]),
    ]
}

/// Which alignment cosmic-text should give each line, if any.
///
/// `None` leaves the line laid out from its start, which is what an unwrapped
/// string wants and what cosmic-text does with no alignment set.
pub(super) fn line_alignment(instance: &TextInstance) -> Option<Align> {
    match instance.line_align {
        LineAlign::Follow => match instance.align()[0] {
            TextAlign::Start => None,
            TextAlign::Middle => Some(Align::Center),
            TextAlign::End => Some(Align::Right),
        },
        LineAlign::Left => Some(Align::Left),
        LineAlign::Center => Some(Align::Center),
        LineAlign::Right => Some(Align::Right),
        LineAlign::Justify => Some(Align::Justified),
    }
}
#[cfg(test)]
impl LaidOut {
    /// A layout with only the parts placement reads, for testing placement
    /// without a font.
    fn measured(block: [f32; 2], wrapped: bool) -> Self {
        Self {
            buffer: Buffer::new_empty(Metrics::new(1.0, 1.0)),
            wrapped,
            scale: 1.0,
            block,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::options::TextWrap;
    use super::{LaidOut, TextAlign, TextInstance, layout_origin};

    fn instance(align: [TextAlign; 2]) -> TextInstance {
        TextInstance::new("hi", "font.ttf", [0.0, 0.0], 0.1, 0.12, [1.0; 4], align)
            .expect("a finite instance")
    }

    /// Where a block of this size would be placed, without shaping anything.
    ///
    /// `wrapped` is the one thing the placement cannot read off the block, so
    /// the tests below say it: it is whether cosmic-text was given a width and
    /// has therefore already positioned the lines inside it.
    fn placed(instance: &TextInstance, block: [f32; 2], wrapped: bool) -> [f32; 2] {
        layout_origin(instance, &LaidOut::measured(block, wrapped))
    }

    /// With no box the block is placed by its own size, exactly as it was
    /// before boxes existed.
    #[test]
    fn a_string_with_no_box_is_placed_by_its_own_size() {
        let centred = instance([TextAlign::Middle; 2]);
        let origin = placed(&centred, [0.4, 0.1], false);
        assert!((origin[0] + 0.2).abs() < 1.0e-6, "{origin:?}");
        assert!((origin[1] - 0.05).abs() < 1.0e-6, "{origin:?}");
    }

    /// With a box, the box takes the place the block used to and the block sits
    /// inside it — so a paragraph and the panel behind it agree about where
    /// "top left" is.
    #[test]
    fn a_box_is_placed_by_the_anchor_and_the_words_sit_inside_it() {
        let boxed = instance([TextAlign::Start; 2]).in_box([1.0, 0.5], TextWrap::Word);
        // Anchored at its start, the box's top-left is the point itself, and a
        // block aligned to the start sits in that corner.
        let origin = placed(&boxed, [0.4, 0.1], false);
        assert!(origin[0].abs() < 1.0e-6, "{origin:?}");
        assert!(origin[1].abs() < 1.0e-6, "{origin:?}");

        let centred = instance([TextAlign::Middle; 2]).in_box([1.0, 0.5], TextWrap::Word);
        let origin = placed(&centred, [0.4, 0.1], false);
        // The box spans -0.5..0.5, and a 0.4-wide block centred in it starts at
        // -0.2 — the same place it would without a box, which is the point:
        // a box changes what wraps, not where a centred label sits.
        assert!((origin[0] + 0.2).abs() < 1.0e-6, "{origin:?}");
        assert!((origin[1] - 0.05).abs() < 1.0e-6, "{origin:?}");

        // Anchored to the top of its box, a short block hangs from the top edge
        // rather than floating in the middle.
        let top =
            instance([TextAlign::Middle, TextAlign::Start]).in_box([1.0, 0.5], TextWrap::Word);
        let origin = placed(&top, [0.4, 0.1], false);
        assert!(origin[1].abs() < 1.0e-6, "{origin:?}");
    }

    /// An unbounded axis is not a box, however the other one is set.
    #[test]
    fn a_box_with_no_width_still_measures_the_words_across() {
        let tall = instance([TextAlign::Start; 2]).in_box([0.0, 0.5], TextWrap::Word);
        assert!(tall.bounds()[0].abs() < f32::EPSILON);
        assert!((tall.bounds()[1] - 0.5).abs() < f32::EPSILON);
        let origin = placed(&tall, [0.4, 0.1], false);
        assert!(origin[0].abs() < 1.0e-6, "{origin:?}");
    }

    /// A wrapped line is already positioned inside its box, so the block must
    /// not be positioned again.
    ///
    /// The bug this is here for put a centred hint half its box's slack to the
    /// right of centre — visible, but only just, and only against something
    /// else that was centred properly.
    #[test]
    fn a_wrapped_block_is_placed_from_its_box_rather_than_from_its_words() {
        let centred = instance([TextAlign::Middle; 2]).in_box([1.0, 0.5], TextWrap::Word);
        // The box spans -0.5..0.5, so wrapped glyph coordinates are measured
        // from -0.5 whatever the words came out as.
        let origin = placed(&centred, [0.4, 0.1], true);
        assert!((origin[0] + 0.5).abs() < 1.0e-6, "{origin:?}");
        // Unwrapped, the same block is centred by its own width instead.
        let origin = placed(&centred, [0.4, 0.1], false);
        assert!((origin[0] + 0.2).abs() < 1.0e-6, "{origin:?}");
    }

    /// A bound that is not a number is no bound, rather than a box of NaN that
    /// every later comparison quietly fails.
    #[test]
    fn a_bound_that_is_not_a_number_is_no_bound() {
        let odd = instance([TextAlign::Start; 2]).in_box([f32::NAN, -3.0], TextWrap::Word);
        assert!(odd.bounds().iter().all(|side| side.abs() < f32::EPSILON));
    }
}
