//! Where a screen element is, and what the pointer is in the same terms.
//!
//! The overlay is authored in normalized units: vertical size 2, centred on the
//! origin, `+Y` up, so `X` runs to the aspect ratio either side. That is what
//! makes a HUD responsive without anyone writing a breakpoint — an element
//! anchored to a corner is in that corner on a portrait phone and a wide
//! desktop window alike, because the corner is where the extent says it is.

/// The usable overlay, after the parts of the screen a game may not use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenExtent {
    /// Half the whole viewport, in overlay units: `[aspect, 1]`.
    half: [f32; 2],
    /// What each edge loses to a notch, a rounded corner, or a home indicator,
    /// in overlay units, ordered left, right, bottom, top.
    insets: [f32; 4],
    /// The viewport this came from, in pixels.
    ///
    /// Kept so that converting a pointer or a safe-area inset does not make
    /// every caller carry the window size alongside the extent derived from it,
    /// which is two things that can disagree.
    viewport: [f32; 2],
}

impl ScreenExtent {
    /// The overlay for a viewport of this pixel size, with nothing cut off.
    ///
    /// A zero or absent dimension gives a square extent rather than an infinity
    /// or a NaN: a viewport with no area draws nothing, and the arithmetic
    /// downstream should not have to know that.
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        let aspect = if width > 0.0 && height > 0.0 {
            width / height
        } else {
            1.0
        };
        Self {
            half: [aspect, 1.0],
            insets: [0.0; 4],
            viewport: [width, height],
        }
    }

    /// The same overlay with the device's safe area taken off, given in pixels
    /// from each edge as a platform reports them.
    ///
    /// Insets are the one part of responsive layout a designer cannot author
    /// around, because the notch is not in the scene — it is in the hardware
    /// the scene happens to be running on.
    #[must_use]
    pub fn with_safe_area(mut self, insets: SafeArea) -> Self {
        let height = self.viewport[1];
        if self.viewport[0] <= 0.0 || height <= 0.0 {
            return self;
        }
        // Two overlay units span the height, and the same `2 / height` converts
        // a horizontal pixel too: `2 * aspect / width` is the same number,
        // because the overlay is square-unit rather than square-pixel.
        let per_pixel = 2.0 / height;
        let clamp = |value: f32, limit: f32| (value * per_pixel).clamp(0.0, limit);
        self.insets = [
            clamp(insets.left, self.half[0]),
            clamp(insets.right, self.half[0]),
            clamp(insets.bottom, self.half[1]),
            clamp(insets.top, self.half[1]),
        ];
        self
    }

    /// Where an anchor's origin sits, with the safe area respected.
    ///
    /// An anchored element moves in from the edge it is anchored to rather than
    /// the whole overlay shrinking, so a centred element stays centred on the
    /// screen instead of drifting away from a notch on one side.
    #[must_use]
    pub fn anchor_origin(self, unit: [f32; 2]) -> [f32; 2] {
        let inset_x = if unit[0] < 0.0 {
            self.insets[0]
        } else {
            -self.insets[1]
        };
        let inset_y = if unit[1] < 0.0 {
            self.insets[2]
        } else {
            -self.insets[3]
        };
        [
            unit[0].mul_add(self.half[0], unit[0].abs() * inset_x),
            unit[1].mul_add(self.half[1], unit[1].abs() * inset_y),
        ]
    }

    /// A pointer position in viewport pixels, in overlay units.
    ///
    /// `None` when the viewport has no area, because a point inside nothing is
    /// not a point. Pixels run down from the top-left and the overlay runs up
    /// from the middle, and this is the one place that is written down.
    #[must_use]
    pub fn pointer(self, position: [f32; 2]) -> Option<[f32; 2]> {
        let [width, height] = self.viewport;
        (width > 0.0 && height > 0.0).then(|| {
            [
                (position[0] / width - 0.5) * 2.0 * self.half[0],
                (0.5 - position[1] / height) * 2.0 * self.half[1],
            ]
        })
    }

    /// Half the whole overlay, before any safe area.
    #[must_use]
    pub const fn half(self) -> [f32; 2] {
        self.half
    }
}

/// What each edge of the screen loses to the device, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeArea {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
    pub top: f32,
}

/// An element's box on the overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub center: [f32; 2],
    pub size: [f32; 2],
}

impl ScreenRect {
    /// Whether a point in overlay units is inside.
    ///
    /// A rect with no width or height contains nothing, so an element scaled to
    /// zero cannot be clicked — it is not on the screen in any sense a person
    /// would recognise.
    #[must_use]
    pub fn contains(self, point: [f32; 2]) -> bool {
        let inside =
            |value: f32, center: f32, size: f32| size > 0.0 && (value - center).abs() <= size / 2.0;
        inside(point[0], self.center[0], self.size[0])
            && inside(point[1], self.center[1], self.size[1])
    }
}

#[cfg(test)]
mod tests {
    use super::{SafeArea, ScreenExtent, ScreenRect};

    #[test]
    fn the_overlay_is_two_units_tall_whatever_the_window() {
        for (width, height) in [(800.0, 600.0), (390.0, 844.0)] {
            let extent = ScreenExtent::new(width, height);
            assert!((extent.half()[1] - 1.0).abs() < 1.0e-5);
            assert!((extent.half()[0] - width / height).abs() < 1.0e-5);
        }
    }

    #[test]
    fn a_viewport_with_no_area_is_square_rather_than_infinite() {
        let extent = ScreenExtent::new(0.0, 0.0);
        assert!(extent.half()[0].is_finite());
        assert!(extent.pointer([1.0, 1.0]).is_none());
    }

    /// The middle of the screen is the middle of the overlay, both ways round.
    #[test]
    fn the_centre_of_the_viewport_is_the_origin() {
        let extent = ScreenExtent::new(800.0, 600.0);
        let point = extent.pointer([400.0, 300.0]).expect("area");
        assert!(point[0].abs() < 1.0e-5, "{point:?}");
        assert!(point[1].abs() < 1.0e-5, "{point:?}");
    }

    /// Pixels run down and the overlay runs up.
    #[test]
    fn the_top_of_the_window_is_the_top_of_the_overlay() {
        let extent = ScreenExtent::new(800.0, 600.0);
        let top = extent.pointer([400.0, 0.0]).expect("area");
        assert!((top[1] - 1.0).abs() < 1.0e-5, "{top:?}");
        let bottom = extent.pointer([400.0, 600.0]).expect("area");
        assert!((bottom[1] + 1.0).abs() < 1.0e-5, "{bottom:?}");
    }

    #[test]
    fn a_corner_anchor_is_in_the_corner() {
        let extent = ScreenExtent::new(800.0, 600.0);
        let origin = extent.anchor_origin([-1.0, 1.0]);
        assert!((origin[0] + 800.0 / 600.0).abs() < 1.0e-5, "{origin:?}");
        assert!((origin[1] - 1.0).abs() < 1.0e-5, "{origin:?}");
    }

    /// The notch case: a top-anchored element comes down below it.
    #[test]
    fn a_safe_area_moves_an_anchored_element_in_from_its_own_edge() {
        let extent = ScreenExtent::new(390.0, 844.0).with_safe_area(SafeArea {
            top: 47.0,
            ..SafeArea::default()
        });
        let top = extent.anchor_origin([0.0, 1.0]);
        assert!(top[1] < 1.0, "a notch did not move the anchor: {top:?}");
        assert!(
            (top[1] - (1.0 - 2.0 * 47.0 / 844.0)).abs() < 1.0e-5,
            "{top:?}"
        );
    }

    /// A centred element stays centred: the screen does not shrink, the edges
    /// come in.
    #[test]
    fn a_safe_area_leaves_the_middle_where_it_was() {
        let extent = ScreenExtent::new(390.0, 844.0).with_safe_area(SafeArea {
            top: 47.0,
            bottom: 34.0,
            ..SafeArea::default()
        });
        let middle = extent.anchor_origin([0.0, 0.0]);
        assert!(
            middle[0].abs() < 1.0e-5 && middle[1].abs() < 1.0e-5,
            "{middle:?}"
        );
    }

    /// An inset larger than the screen would put an anchor past the far edge.
    #[test]
    fn an_absurd_inset_cannot_turn_the_screen_inside_out() {
        let extent = ScreenExtent::new(390.0, 844.0).with_safe_area(SafeArea {
            top: 9_000.0,
            ..SafeArea::default()
        });
        let top = extent.anchor_origin([0.0, 1.0]);
        assert!((-1.0..=1.0).contains(&top[1]), "{top:?}");
    }

    #[test]
    fn a_rect_contains_its_middle_and_not_what_is_past_its_edge() {
        let rect = ScreenRect {
            center: [0.5, 0.0],
            size: [0.4, 0.2],
        };
        assert!(rect.contains([0.5, 0.0]));
        assert!(rect.contains([0.7, 0.1]), "its own corner");
        assert!(!rect.contains([0.71, 0.0]));
        assert!(!rect.contains([0.5, 0.11]));
    }

    /// An element scaled to nothing is not on the screen in any sense a person
    /// would recognise, so it cannot be clicked.
    #[test]
    fn a_rect_with_no_area_contains_nothing() {
        let rect = ScreenRect {
            center: [0.0, 0.0],
            size: [0.0, 0.2],
        };
        assert!(!rect.contains([0.0, 0.0]));
    }
}
