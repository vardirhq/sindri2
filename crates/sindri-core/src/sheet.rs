//! A sliced image: named parts of one texture, described beside it.
//!
//! The problem this solves is duplication. Before it, three components each
//! said how a sheet was cut — a sprite carried a raw rect, an animation carried
//! a grid and cell numbers, a tilemap carried a second grid and more cell
//! numbers — so the same image used twice declared its layout twice and nothing
//! made the two agree. A sheet is a property of the image, not of whoever draws
//! it, so it belongs beside the image and is said once.
//!
//! A sheet document sits at a derived ID: `textures/tiles.png` is sliced by
//! `textures/tiles.sheet.json`. Derived rather than declared, because a scene
//! naming its sheets would be a fourth place that can disagree.
//!
//! Nothing here knows what a `UvRect` is — that is `sindri-render`'s, and this
//! crate does not depend on it. Rects are stored as they are authored and
//! checked where they are used, which is the same arrangement `SceneDocument`
//! has with the components it carries.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::AssetId;

#[cfg(test)]
mod tests;

/// The version this runtime writes and understands.
pub const SHEET_FORMAT_VERSION: u32 = 1;

/// The suffix that turns a texture's ID into its sheet's ID.
const SHEET_SUFFIX: &str = ".sheet.json";

/// One image, cut into named parts.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SpriteSheetDocument {
    pub format_version: u32,
    /// A uniform slice, which is how a sheet is almost always cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<SheetGrid>,
    /// Parts that are not on the grid, named explicitly.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sprites: BTreeMap<String, [f32; 4]>,
    /// Editor-only state, carried through untouched — the same arrangement a
    /// scene has. Where a slicer remembers what it was last showing.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub editor: BTreeMap<String, Value>,
}

/// A uniform slice of a sheet, in cells.
///
/// The grid *generates* names rather than being stored alongside a list of
/// rects it agrees with: a cell's rect is derivable from its column and row, so
/// storing both would be the same duplication one level down.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SheetGrid {
    pub columns: u32,
    pub rows: u32,
    /// The image this grid was cut against, in pixels.
    ///
    /// Needed only when the grid has a margin or spacing, because those are
    /// pixel measurements and a rect is a fraction. A grid that divides an image
    /// edge to edge does not need it, and does not carry it.
    ///
    /// Recorded rather than read from the texture so a sheet stays a document
    /// that can be understood on its own — the same reason a scene carries its
    /// own format version. A sheet whose size disagrees with its image is a
    /// thing worth reporting, and reporting it needs the claim written down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    /// Pixels of border around the whole grid, before the first cell.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub margin: [u32; 2],
    /// Pixels between neighbouring cells, belonging to no sprite.
    ///
    /// Sheets are packed with gutters so that filtering cannot bleed one frame
    /// into the next, and a slicer that cannot say so can only cut sheets that
    /// were exported without them.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spacing: [u32; 2],
    /// What to call each cell, row-major from the top-left.
    ///
    /// A cell no name is given for is called by its own index, so a sheet that
    /// has been sliced but not named is still usable — `#3` is a worse name
    /// than `#idle` and a much better one than nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
}

/// Serde hands `skip_serializing_if` a reference, which is why this takes one.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(value: &[u32; 2]) -> bool {
    value[0] == 0 && value[1] == 0
}

impl SheetGrid {
    /// A grid that divides an image edge to edge, with no border or gutters.
    #[must_use]
    pub const fn edge_to_edge(columns: u32, rows: u32) -> Self {
        Self {
            columns,
            rows,
            size: None,
            margin: [0, 0],
            spacing: [0, 0],
            names: Vec::new(),
        }
    }

    /// How many cells the grid holds.
    #[must_use]
    pub const fn cells(&self) -> u32 {
        self.columns.saturating_mul(self.rows)
    }

    /// What cell `index` is called.
    #[must_use]
    pub fn name_of(&self, index: u32) -> String {
        self.names
            .get(index as usize)
            .filter(|name| !name.is_empty())
            .map_or_else(|| index.to_string(), Clone::clone)
    }

    /// The normalized rect of cell `index`, or `None` when it is off the grid.
    ///
    /// Computed in `f64` and narrowed once, so the last cell of a sheet that
    /// does not divide evenly in binary lands on the edge rather than a hair
    /// past it — the same reasoning `UvRect::cell` is written with, kept here
    /// because this crate cannot reach that one.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn rect_of(&self, index: u32) -> Option<[f32; 4]> {
        if self.columns == 0 || self.rows == 0 || index >= self.cells() {
            return None;
        }
        let column = f64::from(index % self.columns);
        let row = f64::from(index / self.columns);

        // The plain case needs no image size, and does not ask for one: a grid
        // that divides an image edge to edge is the same fractions whatever the
        // image turns out to be.
        if is_zero(&self.margin) && is_zero(&self.spacing) {
            let width = 1.0 / f64::from(self.columns);
            let height = 1.0 / f64::from(self.rows);
            return Some([
                (column * width) as f32,
                (row * height) as f32,
                width as f32,
                height as f32,
            ]);
        }

        let [image_width, image_height] = self.size?;
        let axis =
            |image: u32, count: u32, margin: u32, spacing: u32, at: f64| -> Option<(f64, f64)> {
                let image = f64::from(image);
                if image <= 0.0 {
                    return None;
                }
                let gaps = f64::from(count.saturating_sub(1)) * f64::from(spacing);
                let usable = image - 2.0 * f64::from(margin) - gaps;
                if usable <= 0.0 {
                    return None;
                }
                let cell = usable / f64::from(count);
                let start = f64::from(margin) + at * (cell + f64::from(spacing));
                Some((start / image, cell / image))
            };
        let (x, width) = axis(
            image_width,
            self.columns,
            self.margin[0],
            self.spacing[0],
            column,
        )?;
        let (y, height) = axis(
            image_height,
            self.rows,
            self.margin[1],
            self.spacing[1],
            row,
        )?;
        Some([x as f32, y as f32, width as f32, height as f32])
    }
}

impl SpriteSheetDocument {
    /// A uniform sheet of `columns` by `rows`, with cells named by index.
    #[must_use]
    pub fn from_grid(columns: u32, rows: u32) -> Self {
        Self {
            format_version: SHEET_FORMAT_VERSION,
            grid: Some(SheetGrid::edge_to_edge(columns, rows)),
            sprites: BTreeMap::new(),
            editor: BTreeMap::new(),
        }
    }

    /// Parses a sheet, rejecting a version this runtime does not write.
    pub fn from_json(json: &str) -> Result<Self, SheetError> {
        let document: Self = serde_json::from_str(json).map_err(|error| SheetError::Json {
            message: error.to_string(),
        })?;
        document.validate()?;
        Ok(document)
    }

    /// Every named part of the image, grid cells and explicit rects together.
    ///
    /// Named rather than indexed, because the whole point is that a scene says
    /// `#floor` and not `#7`: a name survives a re-slice that moves the cell,
    /// and an index does not.
    pub fn rects(&self) -> Result<BTreeMap<String, [f32; 4]>, SheetError> {
        let mut rects = BTreeMap::new();
        if let Some(grid) = &self.grid {
            if grid.columns == 0 || grid.rows == 0 {
                return Err(SheetError::EmptyGrid {
                    columns: grid.columns,
                    rows: grid.rows,
                });
            }
            if grid.names.len() > grid.cells() as usize {
                return Err(SheetError::TooManyNames {
                    names: grid.names.len(),
                    cells: grid.cells() as usize,
                });
            }
            // A grid measured in pixels cannot be turned into fractions without
            // knowing the image, and saying so beats every cell coming out as
            // nothing.
            if (!is_zero(&grid.margin) || !is_zero(&grid.spacing)) && grid.size.is_none() {
                return Err(SheetError::MeasuredWithoutSize);
            }
            for index in 0..grid.cells() {
                let name = grid.name_of(index);
                let rect = grid
                    .rect_of(index)
                    .ok_or(SheetError::CellDoesNotFit { index })?;
                if rects.insert(name.clone(), rect).is_some() {
                    return Err(SheetError::DuplicateName(name));
                }
            }
        }
        for (name, rect) in &self.sprites {
            if rects.insert(name.clone(), *rect).is_some() {
                return Err(SheetError::DuplicateName(name.clone()));
            }
        }
        if rects.is_empty() {
            return Err(SheetError::Empty);
        }
        Ok(rects)
    }

    fn validate(&self) -> Result<(), SheetError> {
        if self.format_version != SHEET_FORMAT_VERSION {
            return Err(SheetError::UnsupportedVersion {
                found: self.format_version,
                supported: SHEET_FORMAT_VERSION,
            });
        }
        self.rects().map(|_| ())
    }
}

/// The sheet that slices `texture`, by the one naming rule.
///
/// `textures/tiles.png` is sliced by `textures/tiles.sheet.json`. A texture
/// whose ID already ends in the suffix is not a texture, and gets `None` rather
/// than a sheet of a sheet.
#[must_use]
pub fn sheet_id_for(texture: &AssetId) -> Option<AssetId> {
    let path = texture.as_str();
    if path.ends_with(SHEET_SUFFIX) {
        return None;
    }
    let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    AssetId::new(format!("{stem}{SHEET_SUFFIX}")).ok()
}

/// What is wrong with a sheet, named specifically enough to fix.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SheetError {
    #[error("sheet format version {found} is not supported (this runtime writes {supported})")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("a sheet must name at least one sprite")]
    Empty,
    #[error("a sheet grid of {columns}x{rows} has no cells")]
    EmptyGrid { columns: u32, rows: u32 },
    #[error("the grid names {names} cells but only has {cells}")]
    TooManyNames { names: usize, cells: usize },
    #[error("two sprites in one sheet are both called `{0}`")]
    DuplicateName(String),
    #[error(
        "a grid with a margin or spacing measures in pixels, so it must record the size of the image it was cut against"
    )]
    MeasuredWithoutSize,
    #[error("cell {index} does not fit the grid it was cut from")]
    CellDoesNotFit { index: u32 },
    #[error("sheet is not valid json: {message}")]
    Json { message: String },
}

#[cfg(test)]
mod margin_tests {
    use super::{SHEET_FORMAT_VERSION, SheetError, SheetGrid, SpriteSheetDocument};

    fn packed() -> SpriteSheetDocument {
        // A 512-pixel sheet of sixteen columns with a two-pixel border and
        // four-pixel gutters: 512 - 4 - 15*4 = 448, so cells are 28 wide.
        SpriteSheetDocument {
            format_version: SHEET_FORMAT_VERSION,
            grid: Some(SheetGrid {
                columns: 16,
                rows: 16,
                size: Some([512, 512]),
                margin: [2, 2],
                spacing: [4, 4],
                names: Vec::new(),
            }),
            ..SpriteSheetDocument::default()
        }
    }

    /// The case a slicer without gutters cannot cut: sheets are packed with
    /// gaps so filtering cannot bleed one frame into the next.
    #[test]
    fn a_gutter_is_left_out_of_every_cell() {
        let sheet = packed();
        let rects = sheet.rects().expect("a packed grid slices");
        let close = |left: f32, right: f32| (left - right).abs() < 1.0e-5;

        let first = rects["0"];
        assert!(close(first[0], 2.0 / 512.0), "the border is skipped");
        assert!(close(first[2], 28.0 / 512.0), "the gutters are not drawn");

        let second = rects["1"];
        assert!(
            close(second[0], (2.0 + 28.0 + 4.0) / 512.0),
            "the next cell starts a gutter past the last, and is at {}",
            second[0]
        );

        // The last cell ends exactly on the far border, which is the property
        // that says the arithmetic closed rather than drifted.
        let last = rects["255"];
        assert!(
            close(last[0] + last[2], (512.0 - 2.0) / 512.0),
            "the last column ends on the border, and ends at {}",
            last[0] + last[2]
        );
    }

    /// A grid with no margin and no spacing needs no image size, because the
    /// fractions do not depend on one.
    #[test]
    fn an_edge_to_edge_grid_needs_no_size() {
        let sheet = SpriteSheetDocument::from_grid(4, 1);
        assert!(sheet.grid.as_ref().expect("a grid").size.is_none());
        assert!(sheet.rects().is_ok());
    }

    /// One measured in pixels does, and says so rather than yielding nothing.
    #[test]
    fn a_measured_grid_without_a_size_is_refused() {
        let mut sheet = packed();
        sheet.grid.as_mut().expect("a grid").size = None;
        assert_eq!(sheet.rects(), Err(SheetError::MeasuredWithoutSize));
    }

    /// Gutters wider than the image leave no cells, which is a mistake worth a
    /// message rather than an empty sheet.
    #[test]
    fn spacing_that_leaves_no_room_is_refused() {
        let mut sheet = packed();
        sheet.grid.as_mut().expect("a grid").spacing = [64, 64];
        assert!(matches!(
            sheet.rects(),
            Err(SheetError::CellDoesNotFit { .. })
        ));
    }
}
