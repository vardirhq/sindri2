//! `sindri.ui.grid`: a parent that places its children in rows and columns.
//!
//! CSS grid, for the part a game UI uses. The grid names its column and row
//! tracks, each a fixed length, `auto` (as big as the largest thing in it) or
//! a share of what is left (`1fr`). A child can say where it starts and how
//! many tracks it spans; the rest flow into the next free cell, row by row,
//! as CSS's auto-placement does, adding rows as they need them.
//!
//! Where a flex layout is a line that wraps, a grid is a table that holds: an
//! inventory whose slots stay aligned whatever is in them, a settings screen
//! whose labels and controls share two columns.

mod place;
mod tracks;

use serde::Deserialize;
use sindri_core::SceneComponent;

pub use tracks::UiTrack;

use super::UiAlign;
pub(super) use place::{content_size, resolve};

/// Places an entity's active children in a grid of tracks.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiGridComponent {
    /// The column tracks, left to right: `"1fr"`, `"auto"`, or a length in
    /// overlay units such as `"0.4"`. None makes one column.
    #[serde(default)]
    pub columns: Vec<UiTrack>,
    /// The row tracks, top to bottom. Rows the children need past these are
    /// added `auto`, as CSS's implicit rows are.
    #[serde(default)]
    pub rows: Vec<UiTrack>,
    /// Empty space between columns and between rows, in overlay units.
    #[serde(default)]
    pub gap: [f32; 2],
    /// Where a child sits across its cell, and down it: `stretch` fills it,
    /// as CSS's default does.
    #[serde(default = "stretch")]
    pub justify_items: UiAlign,
    #[serde(default = "stretch")]
    pub align_items: UiAlign,
    /// Whether the grid sizes itself to its tracks, across and down.
    #[serde(default)]
    pub fit_content: [bool; 2],
}

const fn stretch() -> UiAlign {
    UiAlign::Stretch
}

impl Default for UiGridComponent {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            gap: [0.0; 2],
            justify_items: UiAlign::Stretch,
            align_items: UiAlign::Stretch,
            fit_content: [false; 2],
        }
    }
}

impl SceneComponent for UiGridComponent {
    const TYPE_NAME: &'static str = "sindri.ui.grid";
}

#[cfg(test)]
mod tests;
