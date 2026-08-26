//! Slicing a grid, and the names the cells get.

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
