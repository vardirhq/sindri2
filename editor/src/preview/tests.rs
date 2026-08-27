//! Which files the editor will show, and what it shows of them.

use std::path::{Path, PathBuf};

use super::{MAX_BYTES, TextPreview, is_readable};

/// The language's own source files are the ones this exists for.
///
/// The browser listed `.decay` files and could do nothing with any of them, in
/// an engine whose headline capability is scripting.
#[test]
fn a_script_is_something_the_editor_will_show() {
    for name in [
        "scripts/spin.decay",
        "level.scene.json",
        "tiles.sheet.json",
        "README",
        "Cargo.toml",
    ] {
        assert!(
            is_readable(Path::new(name)),
            "{name} is text and should be readable"
        );
    }
}

/// Reading a texture as text would show a wall of mojibake convincingly.
///
/// What an image, a font and an audio clip need is a preview of their own — a
/// picture, a rendered sample, a play button — and offering the wrong one is
/// worse than the row that admitted it could do nothing.
#[test]
fn a_file_that_is_not_text_is_left_alone() {
    for name in [
        "textures/orb.png",
        "fonts/Inter.ttf",
        "audio/pickup.wav",
        "meshes/hull.glb",
        "bundle.wasm",
        "archive.zip",
    ] {
        assert!(
            !is_readable(Path::new(name)),
            "{name} must not be shown as text"
        );
    }
}

#[test]
fn a_file_is_read_and_named() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("spin.decay");
    std::fs::write(&path, "container Spin {\n    fn tick() {}\n}\n").unwrap();

    let preview = TextPreview::open(&path);
    assert_eq!(preview.name(), "spin.decay");
    assert_eq!(preview.path(), path);
    assert!(preview.body().unwrap().contains("container Spin"));
    assert_eq!(preview.lines(), 3);
    assert!(!preview.truncated());
}

/// A long file is shown to its cut and says so, rather than stalling the frame
/// it was opened on.
#[test]
fn a_long_file_is_cut_and_admits_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("huge.decay");
    std::fs::write(&path, "x".repeat(MAX_BYTES * 2)).unwrap();

    let preview = TextPreview::open(&path);
    assert!(preview.truncated());
    assert_eq!(preview.body().unwrap().len(), MAX_BYTES);
}

/// A file that cannot be read says why, where the text would be.
#[test]
fn a_file_that_will_not_open_says_so() {
    let preview = TextPreview::open(&PathBuf::from("/definitely/not/here.decay"));
    assert!(preview.body().is_err());
    assert_eq!(preview.lines(), 0);
}
