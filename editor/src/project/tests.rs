//! What the walk finds, and where it stops.

use std::fs;

use super::*;

fn project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("demo.scene.json"), "{}").unwrap();
    fs::write(root.join("settings.json"), "{}").unwrap();
    fs::write(root.join(".hidden"), "").unwrap();
    fs::create_dir(root.join("textures")).unwrap();
    fs::write(root.join("textures/badge.png"), "").unwrap();
    fs::write(root.join("textures/tiles.png"), "").unwrap();
    fs::create_dir(root.join("scripts")).unwrap();
    fs::write(root.join("scripts/scene.rs"), "").unwrap();
    fs::write(root.join("scripts/spin.decay"), "").unwrap();
    fs::create_dir(root.join("fonts")).unwrap();
    fs::write(root.join("fonts/Inter.ttf"), "").unwrap();
    directory
}

fn names(entries: &[&ProjectEntry]) -> Vec<String> {
    entries.iter().map(|entry| entry.name.clone()).collect()
}

/// The whole point: the browser shows the project, not eight fixed rows.
#[test]
fn the_browser_reads_the_directory_the_scene_lives_in() {
    let directory = project();
    let tree = ProjectTree::beside(Some(&directory.path().join("demo.scene.json")));

    assert_eq!(tree.error(), None);
    assert_eq!(
        names(&tree.matching("")),
        [
            "demo.scene.json",
            "fonts",
            "Inter.ttf",
            "scripts",
            "scene.rs",
            "spin.decay",
            "settings.json",
            "textures",
            "badge.png",
            "tiles.png",
        ],
        "children follow their parent, and each level is sorted by name"
    );
    assert!(
        !names(&tree.matching(""))
            .iter()
            .any(|name| name == ".hidden"),
        "a dot file belongs to the tooling, not the project"
    );
}

/// The search box accepted typing and filtered nothing, which is worse than
/// a button that visibly does nothing.
#[test]
fn the_search_box_filters_what_is_shown() {
    let directory = project();
    let tree = ProjectTree::rooted(directory.path());

    assert_eq!(names(&tree.matching("png")), ["badge.png", "tiles.png"]);
    assert_eq!(names(&tree.matching("BADGE")), ["badge.png"]);
    assert!(
        tree.matching("nothing here").is_empty(),
        "a search that matches nothing shows nothing rather than everything"
    );
    assert!(
        names(&tree.matching("text")).is_empty(),
        "a search lists files, not the folders they are in"
    );
}

/// A row says what it is, and a scene is the one thing the editor can open.
#[test]
fn a_row_knows_what_kind_of_file_it_is() {
    let directory = project();
    let tree = ProjectTree::rooted(directory.path());
    let kind = |name: &str| {
        tree.entries()
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.kind)
    };

    assert_eq!(kind("demo.scene.json"), Some(AssetKind::Scene));
    assert_eq!(
        kind("settings.json"),
        Some(AssetKind::Other),
        "only a scene file is a scene, or a row offers to open something the editor cannot"
    );
    assert_eq!(kind("badge.png"), Some(AssetKind::Texture));
    assert_eq!(kind("scene.rs"), Some(AssetKind::Script));
    // The engine's own language, which the browser listed as a plain file
    // until it was named here.
    assert_eq!(kind("spin.decay"), Some(AssetKind::Script));
    assert_eq!(kind("Inter.ttf"), Some(AssetKind::Font));
    assert_eq!(kind("textures"), Some(AssetKind::Folder));
}

#[test]
fn fonts_are_project_relative_asset_references() {
    let directory = project();
    let tree = ProjectTree::rooted(directory.path());

    assert_eq!(tree.fonts(), ["fonts/Inter.ttf"]);
}

#[test]
fn a_texture_exposes_the_sprites_named_by_its_sheet() {
    let directory = project();
    fs::write(
        directory.path().join("textures/tiles.sheet.json"),
        r#"{
          "format_version": 1,
          "grid": { "columns": 2, "rows": 1, "names": ["idle", "walk"] }
        }"#,
    )
    .unwrap();
    let tree = ProjectTree::rooted(directory.path());

    assert_eq!(
        tree.sprites_for_texture("textures/tiles.png"),
        ["idle", "walk"]
    );
}

/// A detached scene has no directory to show, and says so rather than
/// showing the last project or a made-up one.
#[test]
fn a_scene_with_no_file_has_no_project_to_browse() {
    let tree = ProjectTree::beside(None);
    assert_eq!(tree.root(), None);
    assert_eq!(tree.label(), "No project");
    assert!(tree.entries().is_empty());
    assert_eq!(tree.error(), None);
}

/// A directory that cannot be read is reported, not drawn as empty.
#[test]
fn an_unreadable_directory_names_itself() {
    let directory = tempfile::tempdir().unwrap();
    let tree = ProjectTree::rooted(&directory.path().join("not-here"));
    let error = tree.error().expect("a missing directory is an error");
    assert!(error.contains("not-here"), "{error}");
    assert!(tree.entries().is_empty());
}

/// The walk stops rather than reading a source tree to draw thirty rows.
#[test]
fn a_deep_tree_stops_and_says_it_stopped() {
    let directory = tempfile::tempdir().unwrap();
    let mut deep = directory.path().to_path_buf();
    for level in 0..(MAX_DEPTH + 2) {
        deep = deep.join(format!("level{level}"));
        fs::create_dir(&deep).unwrap();
    }
    let tree = ProjectTree::rooted(directory.path());
    assert!(tree.truncated(), "the walk has to admit it stopped");
    assert_eq!(tree.entries().len(), MAX_DEPTH);
}
