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
        SpriteSheetDocument {
            format_version: SHEET_FORMAT_VERSION,
            grid: Some(SheetGrid {
                columns: self.columns,
                rows: self.rows,
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
