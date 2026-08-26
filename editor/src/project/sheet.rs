//! Sprite sheets, and the sprites and texture that go with one.

use std::path::{Path, PathBuf};

use sindri_core::SpriteSheetDocument;

use super::kind::AssetKind;

/// What a sheet's file is called, relative to the texture it slices.
///
/// The same rule `sheet_id_for` applies to asset IDs, spelled here in terms of
/// paths on disk. Two spellings of one rule is a thing to watch, but the
/// browser walks a directory and does not have asset IDs to hand.
pub(super) const SHEET_SUFFIX: &str = ".sheet.json";

/// The texture a sheet slices, when one sits beside it.
///
/// A sheet names its texture by stem rather than by extension, so this asks the
/// directory which image is there rather than guessing at `.png`.
pub fn sliced_texture_beside(sheet: &Path) -> Option<PathBuf> {
    let name = sheet.file_name()?.to_str()?;
    let stem = name.strip_suffix(SHEET_SUFFIX)?;
    let directory = sheet.parent()?;
    std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .find_map(|entry| {
            let path = entry.path();
            let matches = path.file_stem().and_then(|found| found.to_str()) == Some(stem)
                && AssetKind::of_file(&path.to_string_lossy()) == AssetKind::Texture;
            matches.then_some(path)
        })
}

/// The sprites the sheet beside `texture` names, or nothing.
///
/// A sheet that will not parse yields no sprites rather than an error: the
/// browser's job is to list a directory, and a broken sidecar is something the
/// slicer shows and fixes, not something that should empty the panel.
pub fn sprites_beside(texture: &Path) -> Vec<String> {
    let Some(stem) = texture.file_stem().and_then(|stem| stem.to_str()) else {
        return Vec::new();
    };
    let sheet = texture.with_file_name(format!("{stem}{SHEET_SUFFIX}"));
    let Ok(json) = std::fs::read_to_string(sheet) else {
        return Vec::new();
    };
    SpriteSheetDocument::from_json(&json)
        .ok()
        .and_then(|document| document.rects().ok())
        .map(|rects| rects.into_keys().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::ProjectTree;
    use super::*;

    /// A sliced image carries its parts, so the browser can show them where a
    /// person looks for them: under the image, not loose in the directory.
    #[test]
    fn a_sliced_texture_carries_the_sprites_its_sheet_names() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        fs::write(directory.path().join("tiles.png"), []).expect("writable");
        fs::write(
            directory.path().join("tiles.sheet.json"),
            r#"{ "format_version": 1,
                 "grid": { "columns": 2, "rows": 1, "names": ["light", "dark"] } }"#,
        )
        .expect("writable");

        fs::write(directory.path().join("a.scene.json"), "{}").expect("writable");
        let tree = ProjectTree::beside(Some(&directory.path().join("a.scene.json")));
        let texture = tree
            .entries()
            .iter()
            .find(|entry| entry.name == "tiles.png")
            .expect("the texture is listed");
        assert_eq!(texture.sprites, vec!["dark".to_owned(), "light".to_owned()]);

        assert!(
            tree.entries()
                .iter()
                .all(|entry| entry.kind != AssetKind::Sheet),
            "the sheet is shown as its texture's sprites, so listing the file \
             as well would say the same thing twice"
        );
    }

    /// An orphaned sheet *is* listed, because a sidecar cutting up an image
    /// nobody can find is exactly what a browser that hides files would let you
    /// never notice.
    #[test]
    fn a_sheet_with_no_texture_is_still_listed() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        fs::write(
            directory.path().join("gone.sheet.json"),
            r#"{ "format_version": 1, "grid": { "columns": 1, "rows": 1 } }"#,
        )
        .expect("writable");

        fs::write(directory.path().join("a.scene.json"), "{}").expect("writable");
        let tree = ProjectTree::beside(Some(&directory.path().join("a.scene.json")));
        assert!(
            tree.entries()
                .iter()
                .any(|entry| entry.kind == AssetKind::Sheet && entry.name == "gone.sheet.json"),
            "a sheet slicing nothing is worth seeing"
        );
    }

    /// A sheet that will not parse leaves its texture unsliced rather than
    /// emptying the panel: listing a directory is the browser's job, and a
    /// broken sidecar is the slicer's to show.
    #[test]
    fn a_broken_sheet_leaves_its_texture_unsliced() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        fs::write(directory.path().join("tiles.png"), []).expect("writable");
        fs::write(directory.path().join("tiles.sheet.json"), "{ not json").expect("writable");

        fs::write(directory.path().join("a.scene.json"), "{}").expect("writable");
        let tree = ProjectTree::beside(Some(&directory.path().join("a.scene.json")));
        let texture = tree
            .entries()
            .iter()
            .find(|entry| entry.name == "tiles.png")
            .expect("the texture is still listed");
        assert!(texture.sprites.is_empty());
    }
}
