//! Where a group of panels can be: the vocabulary, without the arrangement.
//!
//! Two kinds, and the difference between them is whether a panel takes room
//! from the scene or covers it. A [`Slot`] is a dock: it claims space, and the
//! scene view gets what is left. A [`Corner`] is an overlay: it floats over the
//! scene, anchored, and the scene keeps the whole window. [`Place`] is the
//! choice between them, and is what the rest of the editor passes around.

use serde::{Deserialize, Serialize};

/// Where in the window a group of panels sits.
///
/// Seven rather than four because the arrangements people actually want are
/// asymmetric: two columns down one side and one down the other is the common
/// shape of an editor, and a model with a single slot per edge cannot say it.
/// Slots claim space from the outside in, so `FarLeft` is against the window
/// and `Left` sits inside it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    FarLeft,
    Left,
    FarRight,
    Right,
    /// Under the centre, between the side columns.
    Bottom,
    /// The centre itself: whatever is left when every other slot has taken its
    /// share. Cannot be empty — see [`Workspace::take`].
    Main,
    /// A second group under [`Slot::Main`], for showing two views at once.
    MainBottom,
}

impl Slot {
    /// Every slot in the order it claims space, outermost first.
    ///
    /// The frame loop walks this, so the order here *is* the arrangement:
    /// a slot claiming space earlier ends up further out, and `Main` is last
    /// because it is defined as the remainder.
    pub const ALL: [Self; 7] = [
        Self::FarLeft,
        Self::FarRight,
        Self::Left,
        Self::Right,
        Self::Bottom,
        Self::MainBottom,
        Self::Main,
    ];

    /// Which way the slot is measured: `true` when its size is a width.
    pub const fn is_column(self) -> bool {
        matches!(
            self,
            Self::FarLeft | Self::Left | Self::FarRight | Self::Right
        )
    }

    /// How big a slot is before anyone has resized it.
    pub(super) const fn default_size(self) -> f32 {
        match self {
            Self::FarLeft | Self::Left => 260.0,
            Self::FarRight => 340.0,
            Self::Right | Self::Bottom | Self::MainBottom => 300.0,
            // Never used: `Main` is the remainder and has no size of its own.
            Self::Main => 0.0,
        }
    }

    /// The smallest a slot may be dragged to.
    ///
    /// Small enough to be a genuine choice rather than the editor overruling
    /// one. The old panels had minimums near their defaults, which meant the
    /// resize handle had a few pixels of travel and reads as broken.
    pub const fn min_size(self) -> f32 {
        if self.is_column() { 150.0 } else { 90.0 }
    }

    /// A stable id for the egui panel, so a slot keeps its size across frames.
    pub const fn id(self) -> &'static str {
        match self {
            Self::FarLeft => "dock-far-left",
            Self::Left => "dock-left",
            Self::FarRight => "dock-far-right",
            Self::Right => "dock-right",
            Self::Bottom => "dock-bottom",
            Self::Main => "dock-main",
            Self::MainBottom => "dock-main-bottom",
        }
    }
}

/// Which corner of the scene view an overlay is anchored to.
///
/// Four, and no free position. An overlay someone can drag anywhere is an
/// overlay that ends up on top of another one, and the time spent tidying that
/// up is time not spent on the game. Anchoring also means an overlay stays put
/// when the window is resized, which a remembered absolute position does not.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Top left",
            Self::TopRight => "Top right",
            Self::BottomLeft => "Bottom left",
            Self::BottomRight => "Bottom right",
        }
    }

    /// Whether the corner is against the left edge of the scene view.
    pub const fn is_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }

    /// Whether overlays here stack upwards from the bottom.
    pub const fn is_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::TopLeft => "overlay-top-left",
            Self::TopRight => "overlay-top-right",
            Self::BottomLeft => "overlay-bottom-left",
            Self::BottomRight => "overlay-bottom-right",
        }
    }
}

/// Where a group of panels is: taking room, or covering the scene.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Docked: claims space, and the scene view gets what is left.
    Dock(Slot),
    /// Overlaid: floats over the scene view, anchored to one of its corners.
    Overlay(Corner),
}

impl Place {
    /// The centre, which is the one place that always holds something.
    pub const MAIN: Self = Self::Dock(Slot::Main);

    /// Every place, docks first in the order they claim space.
    ///
    /// The frame loop walks this, so the order here *is* the arrangement.
    /// Overlays come last because they are drawn over the scene the docks left,
    /// and cannot be positioned until it is known.
    pub fn all() -> impl Iterator<Item = Self> {
        Slot::ALL
            .into_iter()
            .map(Self::Dock)
            .chain(Corner::ALL.into_iter().map(Self::Overlay))
    }

    /// How big a place is before anyone has resized it.
    pub(super) const fn default_size(self) -> f32 {
        match self {
            Self::Dock(slot) => slot.default_size(),
            Self::Overlay(_) => 260.0,
        }
    }

    /// How tall an overlay is before anyone has resized it. Docks take the
    /// whole of their cross axis and have no answer here.
    pub(super) const fn default_height(self) -> f32 {
        match self {
            Self::Dock(_) => 0.0,
            Self::Overlay(_) => 240.0,
        }
    }

    pub const fn min_size(self) -> f32 {
        match self {
            Self::Dock(slot) => slot.min_size(),
            Self::Overlay(_) => 180.0,
        }
    }

    pub const fn is_overlay(self) -> bool {
        matches!(self, Self::Overlay(_))
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Dock(slot) => slot.id(),
            Self::Overlay(corner) => corner.id(),
        }
    }
}

/// Whether the window's own furniture takes room from the scene or floats over
/// it.
///
/// The distinction the first canvas arrangement missed. Moving two panels into
/// floating boxes while the title bar, the tab strip, the toolbar, the
/// inspector column and the status bar all still claimed their rows left the
/// scene occupying about half the window — a docked editor with two insets, not
/// a canvas. Canvas-first means *nothing* docks: the scene is the window, and
/// every control is drawn over it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Chrome {
    /// Bars and panels claim space; the scene gets what is left.
    #[default]
    Docked,
    /// Bars and panels float; the scene gets the whole window.
    Floating,
}

impl Chrome {
    pub const fn floats(self) -> bool {
        matches!(self, Self::Floating)
    }
}
