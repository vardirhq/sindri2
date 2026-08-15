//! Golden round-trip coverage for the serialized scene format.
//!
//! Each fixture is stored in canonical form, so the files themselves are the
//! golden output: a formatting or ordering change fails here instead of
//! quietly rewriting everyone's scenes. Regenerate them with
//! `SINDRI_UPDATE_SCENE_FIXTURES=1 cargo test --package sindri-core`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sindri_core::{SceneDocument, SceneEntityId, World};

const UPDATE_ENV: &str = "SINDRI_UPDATE_SCENE_FIXTURES";

fn fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Every fixture as a `(path, file name)` pair; the name keeps assertion
/// messages readable.
fn fixtures() -> Vec<(PathBuf, String)> {
    let directory = fixture_directory();
    let mut paths: Vec<PathBuf> = fs::read_dir(&directory)
        .expect("fixture directory is readable")
        .map(|entry| entry.expect("fixture entry is readable").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "expected scene fixtures in {}",
        directory.display()
    );
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .expect("fixture has a file name")
                .to_string_lossy()
                .into_owned();
            (path, name)
        })
        .collect()
}

fn read_fixture(path: &Path, name: &str) -> (SceneDocument, String) {
    let text = fs::read_to_string(path).expect("fixture is readable");
    let document = SceneDocument::from_json(&text)
        .unwrap_or_else(|error| panic!("{name} should parse and validate: {error}"));
    (document, text)
}

/// The checked-in fixtures are byte-for-byte canonical output.
#[test]
fn fixtures_are_stored_in_canonical_form() {
    let updating = std::env::var_os(UPDATE_ENV).is_some();
    for (path, name) in fixtures() {
        let (document, text) = read_fixture(&path, &name);
        let canonical = document.to_canonical_json().expect("canonical json");
        if updating {
            fs::write(&path, &canonical).expect("fixture is writable");
            continue;
        }
        assert_eq!(
            text, canonical,
            "{name} is not canonical; rerun with {UPDATE_ENV}=1"
        );
        assert!(document.is_canonical(), "{name} has unsorted entities");
        assert!(text.ends_with('\n'), "{name} must end with a newline");
    }
}

/// Loading a fixture into a world and saving it back reproduces the file.
#[test]
fn fixtures_round_trip_through_a_runtime_world() {
    for (path, name) in fixtures() {
        let (document, text) = read_fixture(&path, &name);
        let loaded = World::from_scene(&document)
            .unwrap_or_else(|error| panic!("{name} should load into a world: {error}"));

        assert_eq!(loaded.world.len(), document.entities.len(), "{name}");
        let saved = loaded
            .world
            .to_scene()
            .unwrap_or_else(|error| panic!("{name} should save from a world: {error}"));

        assert_eq!(saved, document, "{name} lost data through the world");
        assert_eq!(
            saved.to_canonical_json().expect("canonical json"),
            text,
            "{name} did not reproduce its own bytes"
        );
    }
}

/// Canonical serialization is a fixed point, so repeated saves stop producing diffs.
#[test]
fn canonical_serialization_is_idempotent() {
    for (path, name) in fixtures() {
        let (document, _) = read_fixture(&path, &name);
        let once = document.to_canonical_json().expect("canonical json");
        let reparsed = SceneDocument::from_json(&once).expect("canonical json reparses");
        let twice = reparsed.to_canonical_json().expect("canonical json");

        assert_eq!(once, twice, "{name} is not a serialization fixed point");
        assert_eq!(reparsed, document, "{name} changed while re-parsing");
    }
}

/// Editor state is namespaced and ignorable: removing it leaves the runtime scene intact.
#[test]
fn stripping_editor_metadata_preserves_the_runtime_scene() {
    for (path, name) in fixtures() {
        let (document, _) = read_fixture(&path, &name);
        let mut stripped = document.clone();
        stripped.strip_editor_metadata();
        stripped.validate().expect("stripped scene stays valid");

        assert_eq!(stripped.entities.len(), document.entities.len(), "{name}");
        for (before, after) in document.entities.iter().zip(&stripped.entities) {
            assert_eq!(before.id, after.id, "{name}");
            assert_eq!(before.parent, after.parent, "{name}");
            assert_eq!(before.name, after.name, "{name}");
            assert_eq!(before.transform_2d, after.transform_2d, "{name}");
            assert_eq!(before.transform_3d, after.transform_3d, "{name}");
            assert_eq!(before.components, after.components, "{name}");
            assert!(after.editor.is_empty(), "{name}");
        }
        assert!(stripped.metadata.editor.is_empty(), "{name}");
        assert_eq!(stripped.metadata.name, document.metadata.name, "{name}");
    }
}

/// Unregistered component payloads survive a full load and save unchanged.
#[test]
fn unknown_component_payloads_survive_a_save() {
    let name = "components.scene.json";
    let path = fixture_directory().join(name);
    let (document, _) = read_fixture(&path, name);
    let loaded = World::from_scene(&document).expect("fixture loads");
    let saved = loaded.world.to_scene().expect("fixture saves");

    let holder = SceneEntityId::new("unregistered").unwrap();
    let original = document
        .entity(&holder)
        .expect("fixture declares an unregistered component holder");
    let round_tripped = saved
        .entity(&holder)
        .expect("saved scene keeps the unregistered component holder");
    assert_eq!(original.components, round_tripped.components);
}
