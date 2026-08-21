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
    /// What to call each cell, row-major from the top-left.
    ///
    /// A cell no name is given for is called by its own index, so a sheet that
    /// has been sliced but not named is still usable — `#3` is a worse name
    /// than `#idle` and a much better one than nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
}

impl SheetGrid {
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
        let width = 1.0 / f64::from(self.columns);
        let height = 1.0 / f64::from(self.rows);
        let column = f64::from(index % self.columns);
        let row = f64::from(index / self.columns);
        Some([
            (column * width) as f32,
            (row * height) as f32,
            width as f32,
            height as f32,
        ])
    }
}

impl SpriteSheetDocument {
    /// A uniform sheet of `columns` by `rows`, with cells named by index.
    #[must_use]
    pub fn from_grid(columns: u32, rows: u32) -> Self {
        Self {
            format_version: SHEET_FORMAT_VERSION,
            grid: Some(SheetGrid {
                columns,
                rows,
                names: Vec::new(),
            }),
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
            for index in 0..grid.cells() {
                let name = grid.name_of(index);
                let rect = grid
                    .rect_of(index)
                    .expect("an index below the grid's own cell count is on the grid");
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
    #[error("sheet is not valid json: {message}")]
    Json { message: String },
}

#[cfg(test)]
mod tests {
    use super::{SHEET_FORMAT_VERSION, SheetError, SheetGrid, SpriteSheetDocument, sheet_id_for};
    use crate::{AssetId, SpriteRef};

    fn grid(columns: u32, rows: u32, names: &[&str]) -> SpriteSheetDocument {
        SpriteSheetDocument {
            format_version: SHEET_FORMAT_VERSION,
            grid: Some(SheetGrid {
                columns,
                rows,
                names: names.iter().map(|name| (*name).to_owned()).collect(),
            }),
            ..SpriteSheetDocument::default()
        }
    }

    /// A cell nobody named is called by its index, so a sheet that has been
    /// sliced but not named is still usable.
    #[test]
    fn an_unnamed_cell_is_called_by_its_index() {
        let sheet = grid(2, 2, &["floor"]);
        let rects = sheet.rects().expect("a 2x2 grid names four cells");
        assert_eq!(rects.len(), 4);
        assert!(rects.contains_key("floor"), "the named cell keeps its name");
        for index in ["1", "2", "3"] {
            assert!(
                rects.contains_key(index),
                "cell {index} falls back to its index"
            );
        }
    }

    /// Cells run row-major from the top-left, the same order everything else in
    /// this engine reads a grid in.
    #[test]
    fn cells_run_row_major_from_the_top_left() {
        let sheet = grid(2, 2, &[]);
        let rects = sheet.rects().expect("a 2x2 grid slices");
        let same = |left: [f32; 4], right: [f32; 4]| {
            left.iter()
                .zip(right.iter())
                .all(|(left, right)| (left - right).abs() < f32::EPSILON)
        };
        assert!(
            same(rects["0"], [0.0, 0.0, 0.5, 0.5]),
            "cell 0 is the top left"
        );
        assert!(
            same(rects["1"], [0.5, 0.0, 0.5, 0.5]),
            "cell 1 is to its right"
        );
        assert!(
            same(rects["2"], [0.0, 0.5, 0.5, 0.5]),
            "cell 2 begins the next row"
        );
        assert!(
            same(rects["3"], [0.5, 0.5, 0.5, 0.5]),
            "cell 3 is the bottom right"
        );
    }

    /// Two sprites with one name is a sheet that cannot say which it means.
    #[test]
    fn a_repeated_name_is_rejected() {
        let mut sheet = grid(2, 1, &["floor", "floor"]);
        assert!(matches!(sheet.rects(), Err(SheetError::DuplicateName(name)) if name == "floor"));

        // Including when an explicit rect collides with a grid cell's name.
        sheet = grid(2, 1, &["floor", "wall"]);
        sheet
            .sprites
            .insert("floor".to_owned(), [0.0, 0.0, 1.0, 1.0]);
        assert!(matches!(sheet.rects(), Err(SheetError::DuplicateName(name)) if name == "floor"));
    }

    #[test]
    fn a_sheet_naming_nothing_is_rejected() {
        let sheet = SpriteSheetDocument {
            format_version: SHEET_FORMAT_VERSION,
            ..SpriteSheetDocument::default()
        };
        assert_eq!(sheet.rects(), Err(SheetError::Empty));
    }

    #[test]
    fn a_version_this_runtime_does_not_write_is_refused() {
        let json = r#"{ "format_version": 99, "grid": { "columns": 1, "rows": 1 } }"#;
        assert!(matches!(
            SpriteSheetDocument::from_json(json),
            Err(SheetError::UnsupportedVersion { found: 99, .. })
        ));
    }

    /// A sheet's ID is derived from its texture's, so nothing has to declare
    /// the pairing and nothing can get it wrong.
    #[test]
    fn a_sheets_id_comes_from_its_textures() {
        let texture = AssetId::new("textures/tiles.png").expect("a valid id");
        assert_eq!(
            sheet_id_for(&texture)
                .expect("a texture has a sheet id")
                .as_str(),
            "textures/tiles.sheet.json"
        );
        let sheet = AssetId::new("textures/tiles.sheet.json").expect("a valid id");
        assert_eq!(
            sheet_id_for(&sheet),
            None,
            "a sheet has no sheet of its own"
        );
    }

    /// The fragment splits off before the path is validated, so `#` stays
    /// rejected inside an asset ID while still naming a sprite.
    #[test]
    fn a_reference_splits_into_a_path_and_a_name() {
        let reference = SpriteRef::parse("textures/tiles.png#floor").expect("parses");
        assert_eq!(reference.texture(), "textures/tiles.png");
        assert_eq!(reference.sprite(), Some("floor"));
        assert_eq!(
            reference
                .sheet()
                .expect("a fragment needs a sheet")
                .as_str(),
            "textures/tiles.sheet.json"
        );
        assert_eq!(reference.to_string(), "textures/tiles.png#floor");

        let whole = SpriteRef::parse("textures/badge.png").expect("parses");
        assert_eq!(whole.sprite(), None);
        assert_eq!(
            whole.sheet(),
            None,
            "a reference to a whole image needs no sheet, so an unsliced texture is never asked for one"
        );
    }

    /// A generated texture is not a file, and the colon that makes it
    /// un-parseable as an asset ID is what says so. It still has to parse as a
    /// reference, because a scene may draw one.
    #[test]
    fn a_generated_texture_is_a_reference_without_an_asset() {
        let reference = SpriteRef::parse("procedural:checkerboard").expect("parses");
        assert_eq!(reference.texture(), "procedural:checkerboard");
        assert_eq!(reference.asset(), None, "nothing loads a generated texture");
        assert_eq!(reference.sheet(), None);
    }

    #[test]
    fn a_reference_with_nothing_after_the_hash_is_refused() {
        assert!(SpriteRef::parse("textures/tiles.png#").is_err());
        assert!(SpriteRef::parse("textures/tiles.png#a#b").is_err());
    }
}
