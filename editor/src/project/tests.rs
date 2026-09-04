//! What the walk finds, and where it stops.

use std::fs;

use super::*;

fn project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("demo.scene.json"), "{}").unwrap();
    fs::write(root.join("settings.json"), "{}").unwrap();
    fs::write(root.join("drifter.prefab.json"), "{}").unwrap();
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
            "drifter.prefab.json",
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
    // Its own kind, not "File". Listed as a plain file, a project's prefabs
    // look like blobs the editor does not understand -- which is how the
    // acceptance project's every enemy appeared.
    assert_eq!(kind("drifter.prefab.json"), Some(AssetKind::Prefab));
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
fn fonts_are_asset_references_the_loader_resolves() {
    let directory = project();
    let tree = ProjectTree::rooted(directory.path());

    assert_eq!(tree.fonts(), ["fonts/Inter.ttf"]);
}

/// A project that keeps its scene under `assets/` — which the companion game
/// does — resolves references against that directory, not against the project
/// root the browser is rooted at.
///
/// The whole reason this exists: the inspector offered `assets/textures/orb.png`
/// and marked the `textures/orb.png` the scene actually loads as missing, so
/// accepting the offer made the sprite disappear.
fn nested_project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("Cargo.toml"), "").unwrap();
    fs::write(root.join("index.html"), "").unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();
    fs::create_dir(root.join("assets")).unwrap();
    fs::write(root.join("assets/gather.scene.json"), "{}").unwrap();
    fs::create_dir(root.join("assets/textures")).unwrap();
    fs::write(root.join("assets/textures/orb.png"), "").unwrap();
    fs::create_dir(root.join("assets/scripts")).unwrap();
    fs::write(root.join("assets/scripts/orb.decay"), "").unwrap();
    fs::create_dir(root.join("assets/fonts")).unwrap();
    fs::write(root.join("assets/fonts/Inter.ttf"), "").unwrap();
    fs::create_dir(root.join("assets/audio")).unwrap();
    fs::write(root.join("assets/audio/background.wav"), "").unwrap();
    directory
}

#[test]
fn references_are_resolved_against_the_scene_rather_than_the_root() {
    let directory = nested_project();
    let root = directory.path();
    let tree = ProjectTree::rooted_as(root, "Gather").resolving_at(Some(&root.join("assets")));

    assert_eq!(tree.textures(), ["textures/orb.png"]);
    assert_eq!(tree.scripts(), ["scripts/orb.decay"]);
    assert_eq!(tree.fonts(), ["fonts/Inter.ttf"]);
    assert_eq!(tree.audio(), ["audio/background.wav"]);
    assert_eq!(
        tree.assets_root(),
        Some(root.join("assets").as_path()),
        "and the tree says which directory those are relative to"
    );
    assert!(
        tree.keeps_more_than_assets(),
        "a project with a src/ beside its assets has more than the browser needs to list"
    );
}

/// A file no scene can name is left out of every picker rather than offered
/// under a path that will not load.
#[test]
fn what_the_loader_cannot_reach_is_not_offered() {
    let directory = nested_project();
    let root = directory.path();
    let tree = ProjectTree::rooted(root).resolving_at(Some(&root.join("assets")));
    let referenced = |name: &str| {
        tree.entries()
            .iter()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.reference.clone())
    };

    assert_eq!(referenced("orb.png"), Some("textures/orb.png".to_owned()));
    assert_eq!(
        referenced("main.rs"),
        None,
        "a source file outside the assets"
    );
    assert_eq!(referenced("Cargo.toml"), None);
    assert_eq!(
        referenced("textures"),
        None,
        "and nothing references a folder"
    );
    assert!(
        !tree
            .scripts()
            .iter()
            .any(|script| script.contains("main.rs")),
        "the script picker offers what a scene can run, not what is on disk"
    );
}

/// The layout the editor creates: the scene sits at the root, so a reference
/// and a path below the root are the same string and nothing has to be told
/// anything.
#[test]
fn a_flat_project_resolves_against_its_own_root() {
    let directory = project();
    let root = directory.path();
    let tree = ProjectTree::rooted(root).resolving_at(Some(root));

    assert_eq!(tree.assets_root(), Some(root));
    assert!(
        !tree.keeps_more_than_assets(),
        "there is no second listing to offer, so the browser does not offer one"
    );
    assert_eq!(
        tree.textures(),
        ["textures/badge.png", "textures/tiles.png"]
    );
}

/// A directory outside the tree would leave every file unreferenceable, which
/// is a worse answer than the root the tree already had.
#[test]
fn an_asset_root_outside_the_project_is_ignored() {
    let directory = project();
    let elsewhere = tempfile::tempdir().unwrap();
    let tree = ProjectTree::rooted(directory.path()).resolving_at(Some(elsewhere.path()));

    assert_eq!(tree.assets_root(), Some(directory.path()));
    assert_eq!(tree.fonts(), ["fonts/Inter.ttf"]);
}

/// The folder pane lists what the listing it sits beside can navigate, at the
/// depth that listing draws it: a project's own `src/` is not a folder of
/// assets, and `assets/textures` is one indent from the assets rather than two
/// from the root.
#[test]
fn the_folder_pane_follows_the_listing_it_belongs_to() {
    let directory = nested_project();
    let root = directory.path();
    let assets = root.join("assets");
    let tree = ProjectTree::rooted(root).resolving_at(Some(&assets));
    let listed = |within: Option<&Path>| {
        tree.folders_in(within)
            .into_iter()
            .map(|(entry, depth)| (entry.name.clone(), depth))
            .collect::<Vec<(String, usize)>>()
    };

    assert_eq!(
        listed(Some(&assets)),
        [
            ("audio".to_owned(), 0),
            ("fonts".to_owned(), 0),
            ("scripts".to_owned(), 0),
            ("textures".to_owned(), 0),
        ]
    );
    assert_eq!(
        listed(None),
        [
            ("assets".to_owned(), 0),
            ("audio".to_owned(), 1),
            ("fonts".to_owned(), 1),
            ("scripts".to_owned(), 1),
            ("textures".to_owned(), 1),
            ("src".to_owned(), 0),
        ],
        "and the whole project still lists every folder in it"
    );
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
