//! What a sprite sheet accepts, and the names it produces.

use super::{SHEET_FORMAT_VERSION, SheetError, SheetGrid, SpriteSheetDocument, sheet_id_for};
use crate::{AssetId, SpriteRef};

fn grid(columns: u32, rows: u32, names: &[&str]) -> SpriteSheetDocument {
    SpriteSheetDocument {
        format_version: SHEET_FORMAT_VERSION,
        grid: Some(SheetGrid {
            columns,
            rows,
            names: names.iter().map(|name| (*name).to_owned()).collect(),
            ..SheetGrid::edge_to_edge(columns, rows)
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
