//! Slicing an image into named sprites, on the image.
//!
//! The sheet is a property of the picture, so this is where a picture is shown
//! and cut. What it edits is the sidecar beside the texture — `tiles.png` is
//! sliced by `tiles.sheet.json` — and nothing else in the project has to be told
//! about it, because a sheet's ID is derived from its texture's.
//!
//! The image is decoded on the CPU and handed to egui rather than going through
//! the renderer's `TextureRegistry`. That registry exists to draw a *scene*, and
//! a picture of an asset nothing in the scene references has no business in it.

use std::path::{Path, PathBuf};

use eframe::egui;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::{AssetId, SHEET_FORMAT_VERSION, SheetGrid, SpriteSheetDocument};

/// The texture being sliced, its picture, and the slice as it is being edited.
pub struct Slicer {
    path: PathBuf,
    /// The sheet's own path, which is where a save goes.
    sheet: PathBuf,
    /// The decoded picture, held for egui to draw. `None` when the file is not
    /// an image this build can read, which is a thing to say rather than an
    /// empty panel.
    image: Option<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
    /// The image's own size, so the grid can be drawn over it truthfully.
    size: (u32, u32),
    pub columns: u32,
    pub rows: u32,
    /// Pixels of border around the whole grid.
    pub margin: [u32; 2],
    /// Pixels between neighbouring cells, belonging to no sprite.
    pub spacing: [u32; 2],
    /// The cell being named, picked on the image.
    ///
    /// One at a time, and that is what makes a large sheet workable: a field
    /// per cell is fine at four and unusable at two hundred and fifty-six,
    /// which is the same wall a list of forty-nine floor tiles hit.
    pub selected: u32,
    /// One name per cell, row-major. Empty means "call it by its index", which
    /// is what an unnamed cell resolves to anyway.
    pub names: Vec<String>,
    /// What went wrong reading or writing, for the panel to show.
    pub problem: Option<String>,
}

impl Slicer {
    /// Opens `texture` for slicing, reading the sheet beside it if there is one.
    ///
    /// A texture with no sheet starts as a one-by-one grid rather than as
    /// nothing: the whole image is a legitimate slice, and it means pressing
    /// Save on an untouched panel produces something valid.
    pub fn open(texture: &Path) -> Self {
        let sheet = sheet_path(texture);
        let mut slicer = Self {
            path: texture.to_path_buf(),
            sheet,
            image: None,
            texture: None,
            size: (0, 0),
            columns: 1,
            rows: 1,
            margin: [0, 0],
            spacing: [0, 0],
            selected: 0,
            names: Vec::new(),
            problem: None,
        };
        slicer.read_image();
        slicer.read_sheet();
        slicer
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What the texture is called, which is the panel's heading.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }

    pub const fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The picture, uploaded to egui on first sight and kept.
    pub fn texture(&mut self, context: &egui::Context) -> Option<&egui::TextureHandle> {
        if self.texture.is_none()
            && let Some(image) = self.image.take()
        {
            self.texture = Some(context.load_texture(
                self.path.to_string_lossy(),
                image,
                // Nearest, because a sheet is usually pixel art and a slicer
                // that blurs the thing being cut is showing you the wrong
                // picture of it.
                egui::TextureOptions::NEAREST,
            ));
        }
        self.texture.as_ref()
    }

    /// How many cells the current grid holds.
    pub const fn cells(&self) -> u32 {
        self.columns.saturating_mul(self.rows)
    }

    /// What cell `index` is called, falling back to its index.
    pub fn name_of(&self, index: u32) -> String {
        self.names
            .get(index as usize)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| index.to_string())
    }

    /// Where each cell sits in the image, as fractions.
    ///
    /// Asked of the document rather than worked out again here, so the preview
    /// draws exactly what a scene will read — a slicer whose picture and whose
    /// output are computed separately is a slicer that can lie.
    pub fn cell_rects(&self) -> Vec<[f32; 4]> {
        let document = self.document();
        let Some(grid) = document.grid.as_ref() else {
            return Vec::new();
        };
        (0..grid.cells())
            .map(|index| grid.rect_of(index).unwrap_or([0.0, 0.0, 0.0, 0.0]))
            .collect()
    }

    /// Keeps the selection on a cell that exists.
    pub const fn clamp_selection(&mut self) {
        let cells = self.cells();
        if cells == 0 {
            self.selected = 0;
        } else if self.selected >= cells {
            self.selected = cells - 1;
        }
    }

    /// The cells that were given a name, as `(index, name)`.
    ///
    /// What the panel lists instead of every cell: a sheet of two hundred and
    /// fifty-six is mostly cells nobody needed to name, and those already have
    /// an answer.
    pub fn named(&self) -> Vec<(u32, &str)> {
        self.names
            .iter()
            .enumerate()
            .filter(|(_, name)| !name.is_empty())
            .filter_map(|(index, name)| Some((u32::try_from(index).ok()?, name.as_str())))
            .collect()
    }

    /// Resizes the name list to the grid, keeping what was already typed.
    ///
    /// Changing the grid is how a slice is found, so names survive it: dropping
    /// them on every drag of the columns field would make naming the last thing
    /// anyone dares do.
    pub fn fit_names(&mut self) {
        self.names.resize(self.cells() as usize, String::new());
    }

    /// The document this slice would write.
    pub fn document(&self) -> SpriteSheetDocument {
        // Trailing unnamed cells are not written: they resolve to their index
        // either way, and a file full of empty strings is a file that looks
        // like it means something.
        let mut names: Vec<String> = self.names.clone();
        while names.last().is_some_and(String::is_empty) {
            names.pop();
        }
        let measured = self.margin != [0, 0] || self.spacing != [0, 0];
        SpriteSheetDocument {
            format_version: SHEET_FORMAT_VERSION,
            grid: Some(SheetGrid {
                columns: self.columns,
                rows: self.rows,
                // Recorded only when the grid is measured in pixels, so an
                // edge-to-edge slice stays the same file it was before margins
                // existed.
                size: measured.then_some([self.size.0, self.size.1]),
                margin: self.margin,
                spacing: self.spacing,
                names,
            }),
            ..SpriteSheetDocument::default()
        }
    }

    /// Writes the sidecar, or says why it could not.
    ///
    /// Checked before writing, so a slice that would not load is refused here
    /// rather than becoming a file that breaks every scene using it.
    pub fn save(&mut self) -> bool {
        let document = self.document();
        if let Err(error) = document.rects() {
            self.problem = Some(error.to_string());
            return false;
        }
        let json = match serde_json::to_string_pretty(&document) {
            Ok(json) => format!("{json}\n"),
            Err(error) => {
                self.problem = Some(error.to_string());
                return false;
            }
        };
        match std::fs::write(&self.sheet, json) {
            Ok(()) => {
                self.problem = None;
                true
            }
            Err(error) => {
                self.problem = Some(format!("{}: {error}", self.sheet.display()));
                false
            }
        }
    }

    /// Whether a sheet exists on disk for this texture.
    pub fn is_sliced(&self) -> bool {
        self.sheet.exists()
    }

    fn read_image(&mut self) {
        let Ok(bytes) = std::fs::read(&self.path) else {
            self.problem = Some(format!("{} could not be read", self.name()));
            return;
        };
        let Ok(id) = AssetId::new(self.name()) else {
            return;
        };
        match TextureAssetDecoder.decode(AssetBytes::new(id, bytes)) {
            Ok(asset) => {
                self.size = (asset.width(), asset.height());
                self.image = Some(egui::ColorImage::from_rgba_unmultiplied(
                    [asset.width() as usize, asset.height() as usize],
                    asset.rgba8(),
                ));
            }
            Err(error) => self.problem = Some(error.to_string()),
        }
    }

    fn read_sheet(&mut self) {
        let Ok(json) = std::fs::read_to_string(&self.sheet) else {
            self.fit_names();
            return;
        };
        match SpriteSheetDocument::from_json(&json) {
            Ok(document) => {
                if let Some(grid) = document.grid {
                    self.columns = grid.columns;
                    self.rows = grid.rows;
                    self.margin = grid.margin;
                    self.spacing = grid.spacing;
                    self.names = grid.names;
                }
                self.fit_names();
            }
            // A sheet that will not parse is shown as a problem with the grid
            // left at its default, so the panel is a place to fix it rather
            // than a place that refuses to open.
            Err(error) => {
                self.problem = Some(error.to_string());
                self.fit_names();
            }
        }
    }
}

/// Where the sheet for `texture` lives, by the same rule `sheet_id_for` applies
/// to asset IDs.
fn sheet_path(texture: &Path) -> PathBuf {
    let stem = texture
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    texture.with_file_name(format!("{stem}.sheet.json"))
}

#[cfg(test)]
mod tests {
    use super::{Slicer, sheet_path};
    use std::{fs, path::Path};

    #[test]
    fn a_sheet_sits_beside_its_texture() {
        assert_eq!(
            sheet_path(Path::new("/a/textures/tiles.png")),
            Path::new("/a/textures/tiles.sheet.json")
        );
    }

    /// Opening an unsliced image offers the whole thing as one cell, so Save on
    /// an untouched panel writes something valid rather than nothing.
    #[test]
    fn an_unsliced_image_starts_as_one_cell() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let texture = directory.path().join("tiles.png");
        fs::write(&texture, []).expect("writable");

        let slicer = Slicer::open(&texture);
        assert_eq!((slicer.columns, slicer.rows), (1, 1));
        assert!(!slicer.is_sliced());
    }

    /// Changing the grid keeps the names already typed, because changing the
    /// grid is how a slice is found.
    #[test]
    fn resizing_the_grid_keeps_the_names_already_typed() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let texture = directory.path().join("tiles.png");
        fs::write(&texture, []).expect("writable");

        let mut slicer = Slicer::open(&texture);
        slicer.columns = 2;
        slicer.fit_names();
        slicer.names[0] = "floor".to_owned();
        slicer.columns = 4;
        slicer.fit_names();
        assert_eq!(slicer.names[0], "floor", "a name survives a re-slice");
        assert_eq!(slicer.names.len(), 4);
    }

    /// A slice is written, read back, and names the cells it said it would.
    #[test]
    fn a_saved_slice_reads_back_as_itself() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let texture = directory.path().join("tiles.png");
        fs::write(&texture, []).expect("writable");

        let mut slicer = Slicer::open(&texture);
        slicer.columns = 2;
        slicer.rows = 1;
        slicer.fit_names();
        slicer.names[0] = "light".to_owned();
        slicer.names[1] = "dark".to_owned();
        assert!(slicer.save(), "the slice writes: {:?}", slicer.problem);

        let reopened = Slicer::open(&texture);
        assert!(reopened.is_sliced());
        assert_eq!((reopened.columns, reopened.rows), (2, 1));
        assert_eq!(reopened.name_of(0), "light");
        assert_eq!(reopened.name_of(1), "dark");
    }

    /// An unnamed cell is called by its index, and the file does not carry a
    /// list of empty strings to say so.
    #[test]
    fn unnamed_cells_are_called_by_their_index() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let texture = directory.path().join("tiles.png");
        fs::write(&texture, []).expect("writable");

        let mut slicer = Slicer::open(&texture);
        slicer.columns = 3;
        slicer.fit_names();
        slicer.names[0] = "first".to_owned();
        assert!(slicer.save(), "the slice writes");
        assert_eq!(slicer.name_of(2), "2");

        let written = fs::read_to_string(directory.path().join("tiles.sheet.json"))
            .expect("the sheet is readable");
        assert!(
            written.matches("\"\"").count() <= 1,
            "trailing unnamed cells are not written as empty strings: {written}"
        );
    }

    /// A slice that would not load is refused at the point of saving rather
    /// than becoming a file that breaks every scene using it.
    #[test]
    fn a_slice_that_would_not_load_is_refused() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let texture = directory.path().join("tiles.png");
        fs::write(&texture, []).expect("writable");

        let mut slicer = Slicer::open(&texture);
        slicer.columns = 2;
        slicer.fit_names();
        slicer.names[0] = "same".to_owned();
        slicer.names[1] = "same".to_owned();
        assert!(!slicer.save(), "two cells with one name is not a sheet");
        assert!(slicer.problem.is_some());
        assert!(
            !directory.path().join("tiles.sheet.json").exists(),
            "and nothing was written"
        );
    }
}

#[cfg(test)]
mod packing_tests {
    use super::Slicer;
    use std::fs;

    fn slicer() -> (tempfile::TempDir, Slicer) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let texture = directory.path().join("atlas.png");
        fs::write(&texture, []).expect("writable");
        let slicer = Slicer::open(&texture);
        (directory, slicer)
    }

    /// A margin and a gutter are pixels, so the sheet has to record the image
    /// they were measured against — and an edge-to-edge slice must not start
    /// recording one, or every existing sheet would gain a field.
    #[test]
    fn a_measured_slice_records_the_image_it_was_cut_against() {
        let (_directory, mut slicer) = slicer();
        assert!(
            slicer.document().grid.expect("a grid").size.is_none(),
            "an edge-to-edge slice is the same file it always was"
        );

        slicer.margin = [2, 2];
        assert!(
            slicer.document().grid.expect("a grid").size.is_some(),
            "a measured slice cannot be read without the size"
        );
    }

    /// The selection is a cell, so it cannot survive a grid that no longer has
    /// it.
    #[test]
    fn the_selection_stays_on_a_cell_that_exists() {
        let (_directory, mut slicer) = slicer();
        slicer.columns = 8;
        slicer.rows = 8;
        slicer.fit_names();
        slicer.selected = 63;

        slicer.columns = 2;
        slicer.rows = 2;
        slicer.fit_names();
        slicer.clamp_selection();
        assert_eq!(slicer.selected, 3, "the last cell of the smaller grid");
    }

    /// The panel lists what was named rather than every cell, which is what
    /// makes a sheet of two hundred and fifty-six workable.
    #[test]
    fn only_named_cells_are_listed() {
        let (_directory, mut slicer) = slicer();
        slicer.columns = 16;
        slicer.rows = 16;
        slicer.fit_names();
        assert_eq!(slicer.cells(), 256);
        assert!(slicer.named().is_empty(), "nothing is named yet");

        slicer.names[7] = "coin".to_owned();
        slicer.names[200] = "door".to_owned();
        assert_eq!(slicer.named(), vec![(7, "coin"), (200, "door")]);
    }

    /// The preview draws the rects the document produces, so what is shown and
    /// what a scene reads cannot drift apart.
    #[test]
    fn the_preview_draws_what_the_document_produces() {
        let (_directory, mut slicer) = slicer();
        slicer.columns = 4;
        slicer.rows = 1;
        slicer.fit_names();

        let rects = slicer.cell_rects();
        assert_eq!(rects.len(), 4);
        let document = slicer.document();
        let produced = document
            .grid
            .as_ref()
            .expect("a grid")
            .rect_of(2)
            .expect("cell two is on the grid");
        assert!(
            rects[2]
                .iter()
                .zip(produced.iter())
                .all(|(drawn, read)| (drawn - read).abs() < f32::EPSILON),
            "the preview drew {:?} where a scene reads {produced:?}",
            rects[2]
        );
    }
}
