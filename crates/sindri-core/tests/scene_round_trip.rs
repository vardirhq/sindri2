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

use sindri_core::{
    CommandBuffer, CommandHistory, SceneDocument, SceneEntityId, Transform3D, World, WorldCommand,
};

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

/// The editor's exit gate in miniature: edit a transform through a command,
/// save, reopen, and get the edit back.
#[test]
fn an_edited_transform_survives_a_save_and_reopen() {
    let name = "hierarchy.scene.json";
    let path = fixture_directory().join(name);
    let (document, original_text) = read_fixture(&path, name);

    let loaded = World::from_scene(&document).expect("fixture loads");
    let mut world = loaded.world;
    let torso = loaded.entity_map[&SceneEntityId::new("torso").unwrap()];

    let edited = Transform3D {
        position: [0.0, 3.5, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 2.0, 1.0],
        ..Transform3D::default()
    };
    let mut history = CommandHistory::default();
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetTransform3D {
        entity: torso,
        transform: Some(edited),
    });
    history
        .apply(buffer.into_transaction("Move torso"), &mut world)
        .expect("the edit applies");

    let saved_text = world
        .to_scene()
        .expect("edited world saves")
        .to_canonical_json()
        .expect("canonical json");
    assert_ne!(saved_text, original_text, "the edit should change the file");

    let reopened = SceneDocument::from_json(&saved_text).expect("saved scene reopens");
    let reloaded = World::from_scene(&reopened).expect("saved scene loads");
    let reloaded_torso = reloaded.entity_map[&SceneEntityId::new("torso").unwrap()];
    assert_eq!(
        reloaded.world.get(reloaded_torso).unwrap().transform_3d,
        Some(edited)
    );

    // Undoing restores the authored file byte for byte.
    history.undo(&mut world).expect("the edit reverses");
    assert_eq!(
        world
            .to_scene()
            .expect("restored world saves")
            .to_canonical_json()
            .expect("canonical json"),
        original_text
    );
}

/// A declared Z lock is part of the scene, and a transform that declares
/// nothing writes nothing — which is why adding the field moved no version and
/// left every stored fixture byte for byte what it was.
#[test]
fn a_locked_transform_saves_what_it_declared_and_nothing_else() {
    let json = r#"{
        "format_version": 6,
        "entities": [
            { "id": "background", "transform_3d": {
                "position": [0.0, 0.0, -50.0], "z_locked": true } },
            { "id": "player", "transform_3d": { "position": [1.0, 0.0, 0.0] } }
        ]
    }"#;
    let document = SceneDocument::from_json(json).expect("the scene parses");
    let loaded = World::from_scene(&document).expect("it loads");
    let saved = loaded.world.to_scene().expect("it saves");

    let locked = |document: &SceneDocument, id: &str| {
        document
            .entity(&SceneEntityId::new(id).unwrap())
            .and_then(|entity| entity.transform_3d)
            .map(|transform| transform.z_locked)
    };
    assert_eq!(locked(&saved, "background"), Some(true));
    assert_eq!(locked(&saved, "player"), Some(false));

    let canonical = saved.to_canonical_json().expect("canonical json");
    assert!(
        canonical.contains("\"z_locked\": true"),
        "a declared lock must survive the round trip: {canonical}"
    );
    assert_eq!(
        canonical.matches("z_locked").count(),
        1,
        "a transform that declares nothing must write nothing: {canonical}"
    );
}

/// Each format change, checked against documents written before it.
///
/// Format 2 collapsed the separate 2D transform into the one 3D transform, and
/// format 3 replaced the sprite's authored sorting depth with the Z it already
/// had. These are the fixtures as they were actually stored at each version,
/// kept so the upgrades are exercised against real files rather than documents
/// written to suit the test.
mod migration {
    use sindri_core::{SceneMigrator, Transform3D};

    use super::{PathBuf, fs};

    fn legacy(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/legacy")
            .join(name);
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()))
    }

    /// The strongest claim available: each stored older fixture migrates to
    /// exactly the current fixture stored beside it. Not "it parses" — the same
    /// document, byte for byte once canonicalised. A version 1 file runs the
    /// whole chain, so the steps are checked composed as well as alone.
    #[test]
    fn an_older_scene_migrates_to_the_stored_current_fixture() {
        for (before, after) in [
            ("hierarchy.v1.json", "hierarchy.scene.json"),
            ("components.v1.json", "components.scene.json"),
            ("hierarchy.v2.json", "hierarchy.scene.json"),
            ("components.v2.json", "components.scene.json"),
            ("minimal.v2.json", "minimal.scene.json"),
            ("hierarchy.v3.json", "hierarchy.scene.json"),
            ("components.v3.json", "components.scene.json"),
            ("minimal.v3.json", "minimal.scene.json"),
        ] {
            let migrated = super::SceneDocument::from_json_migrated(
                &legacy(before),
                &SceneMigrator::builtin(),
            )
            .unwrap_or_else(|error| panic!("{before} should migrate: {error}"));
            assert_eq!(migrated.format_version, sindri_core::SCENE_FORMAT_VERSION);

            let current = fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(after),
            )
            .expect("the current fixture is readable");
            assert_eq!(
                migrated.to_canonical_json().unwrap(),
                current,
                "{before} did not migrate to {after}"
            );
        }
    }

    /// Format 3's arithmetic: a screen sprite's sorting depth becomes the Z it
    /// is sorted by, negated, because the overlay camera looks down the axis
    /// from `+Z` and a greater depth meant further away. The stack comes out in
    /// the order it went in.
    #[test]
    fn a_sorting_depth_becomes_the_z_that_replaced_it() {
        let json = r#"{
            "format_version": 2,
            "entities": [
                { "id": "near", "transform_3d": { "position": [1.0, 2.0, 0.0] },
                  "components": { "sindri.sprite": { "texture": "b", "depth": 1.0 } } },
                { "id": "far",
                  "components": { "sindri.sprite": { "texture": "b", "depth": 4.0 } } },
                { "id": "unsorted",
                  "components": { "sindri.sprite": { "texture": "b" } } }
            ]
        }"#;
        let document =
            super::SceneDocument::from_json_migrated(json, &SceneMigrator::builtin()).unwrap();
        let z = |id: &str| {
            document
                .entity(&sindri_core::SceneEntityId::new(id).unwrap())
                .and_then(|entity| entity.transform_3d)
                .map(|transform| transform.position[2])
        };
        assert_eq!(z("near"), Some(-1.0));
        assert_eq!(
            z("far"),
            Some(-4.0),
            "an entity with no transform gains one"
        );
        assert_eq!(z("unsorted"), None, "a sprite with no depth is left alone");
        for entity in &document.entities {
            let sprite = &entity.components["sindri.sprite"];
            assert!(
                sprite.get("depth").is_none(),
                "the depth field must not survive the upgrade: {sprite}"
            );
        }
    }

    /// A world-space sprite's Z already placed it, and now orders it too, so
    /// its depth is dropped rather than written over the position. Moving it
    /// would be the one thing a migration must never do quietly.
    #[test]
    fn a_world_space_sprite_keeps_the_z_it_was_authored_at() {
        let json = r#"{
            "format_version": 2,
            "entities": [{
                "id": "prop",
                "transform_3d": { "position": [0.0, 0.0, -7.0] },
                "components": { "sindri.sprite": {
                    "texture": "b", "space": "world", "depth": 2.0 } }
            }]
        }"#;
        let document =
            super::SceneDocument::from_json_migrated(json, &SceneMigrator::builtin()).unwrap();
        let entity = &document.entities[0];
        // Exactly the authored number, bit for bit: "close enough" is not the
        // claim, since the point is that nothing touched it.
        assert_eq!(
            entity.transform_3d.unwrap().position[2].to_bits(),
            (-7.0_f32).to_bits()
        );
        assert!(entity.components["sindri.sprite"].get("depth").is_none());
    }

    /// The arithmetic, checked rather than assumed: a quarter turn about Z at
    /// (1.5, -2.25) with a non-uniform 2D scale lands on the Z = 0 plane with
    /// the angle as a quaternion and a Z scale of 1.
    #[test]
    fn a_flat_transform_becomes_a_transform_on_the_zero_plane() {
        let json = r#"{
            "format_version": 1,
            "entities": [{
                "id": "shadow",
                "transform_2d": {
                    "position": [1.5, -2.25],
                    "rotation_radians": 1.5707963,
                    "scale": [3.0, 0.25]
                }
            }]
        }"#;
        let document =
            super::SceneDocument::from_json_migrated(json, &SceneMigrator::builtin()).unwrap();
        let moved = document.entities[0].transform_3d.expect("it gained one");

        let quarter_turn = (std::f32::consts::FRAC_PI_4).sin();
        let close = |left: f32, right: f32| (left - right).abs() < 1.0e-6;
        assert!(close(moved.position[0], 1.5) && close(moved.position[1], -2.25));
        assert!(
            close(moved.position[2], 0.0),
            "it belongs on the Z = 0 plane"
        );
        assert!(close(moved.rotation[2], quarter_turn) && close(moved.rotation[3], quarter_turn));
        assert!(close(moved.rotation[0], 0.0) && close(moved.rotation[1], 0.0));
        assert!(
            close(moved.scale[0], 3.0) && close(moved.scale[1], 0.25),
            "the two authored scale components carry over unchanged"
        );
        assert!(close(moved.scale[2], 1.0), "and Z gains an identity scale");
    }

    /// An entity holding both described positions in two different spaces, so
    /// there is no merge of them that is reliably the same scene. Saying so
    /// beats silently preferring one and moving something.
    #[test]
    fn an_entity_with_both_transforms_is_refused_by_name() {
        let json = r#"{
            "format_version": 1,
            "entities": [{
                "id": "confused",
                "transform_2d": { "position": [1.0, 2.0] },
                "transform_3d": { "position": [9.0, 9.0, 9.0] }
            }]
        }"#;
        let error = super::SceneDocument::from_json_migrated(json, &SceneMigrator::builtin())
            .expect_err("it cannot be resolved");
        assert!(error.to_string().contains("confused"), "{error}");
    }

    #[test]
    fn a_default_transform_2d_survives_as_an_identity() {
        let json =
            r#"{ "format_version": 1, "entities": [{ "id": "plain", "transform_2d": {} }] }"#;
        let document =
            super::SceneDocument::from_json_migrated(json, &SceneMigrator::builtin()).unwrap();
        assert_eq!(
            document.entities[0].transform_3d,
            Some(Transform3D::default())
        );
    }
}
