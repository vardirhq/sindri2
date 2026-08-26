//! Opening, saving, and throwing away a scene.

use std::path::PathBuf;

use sindri_core::{CommandBuffer, CommandHistory, SceneDocument, Transform3D, WorldCommand};
use sindri_cube::DemoScene;

use super::super::editing::find_by_source_id;
use super::super::inspector_panel::draft::{EntityDraft, draft_commands};
use super::super::scene_io::{load_world, open_scene_for};
use super::super::unsaved::Discarding;
use super::support::*;

#[test]
fn the_embedded_scene_loads_into_a_runtime_world() {
    let document = DemoScene::authored_document().unwrap();
    let world = load_world(&extractor(), &document).expect("the demo scene loads");
    assert_eq!(
        world.len(),
        document.entities.len(),
        "loading the embedded scene should preserve every authored entity"
    );
    assert_eq!(
        world
            .entities()
            .filter(|(_, data)| data.components.contains_key("sindri.camera"))
            .count(),
        1,
        "the demo has one authored world camera; screen-space UI needs no camera entity"
    );
    assert!(find_by_source_id(&world, "checker-cube").is_some());
    assert!(find_by_source_id(&world, "not-an-entity").is_none());
}

#[test]
fn edits_survive_a_save_and_reload_of_the_real_scene() {
    let mut world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let original = EntityDraft::from(world.get(entity).unwrap());
    let mut draft = original.clone();
    draft.transform_3d = Some(Transform3D {
        position: [0.0, 1.5, 0.0],
        ..original.transform_3d.unwrap_or_default()
    });

    CommandHistory::default()
        .apply(
            draft_commands(entity, &original, &draft).into_transaction("Move"),
            &mut world,
        )
        .unwrap();

    let saved = world.to_scene().unwrap().to_canonical_json().unwrap();
    let reopened = load_world(&extractor(), &SceneDocument::from_json(&saved).unwrap()).unwrap();
    let reloaded = find_by_source_id(&reopened, "checker-cube").unwrap();
    assert_eq!(
        reopened.get(reloaded).unwrap().transform_3d,
        draft.transform_3d
    );
}

/// The editor reopens where it was left, and a path on the command line
/// still wins — the most deliberate thing anyone can say about which scene
/// to open should not be overruled by a choice made last week.
#[test]
fn the_remembered_scene_is_reopened_unless_one_was_named() {
    let directory = tempfile::tempdir().unwrap();
    let write = |name: &str| {
        let path = directory.path().join(name);
        std::fs::write(
            &path,
            DemoScene::authored_document()
                .unwrap()
                .to_canonical_json()
                .unwrap(),
        )
        .unwrap();
        path.display().to_string()
    };
    let remembered = write("remembered.scene.json");
    let named = write("named.scene.json");

    let (file, error) = open_scene_for(None, Some(&remembered));
    assert_eq!(error, None);
    assert_eq!(file.label(), "remembered.scene.json");

    let (file, error) = open_scene_for(Some(&named), Some(&remembered));
    assert_eq!(error, None);
    assert_eq!(
        file.label(),
        "named.scene.json",
        "an argument outranks what was remembered"
    );
}

/// A project can move or be deleted between launches. Refusing to open
/// anything because of that would make a remembered path a liability.
#[test]
fn a_remembered_scene_that_is_gone_says_so_rather_than_opening_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("gone.scene.json");
    let (_, error) = open_scene_for(None, Some(&missing.display().to_string()));
    let error = error.expect("a scene that is not there is worth saying");
    assert!(error.contains("gone.scene.json"), "{error}");
}

/// The marker means the file and the world differ, not that something was
/// touched. Undoing back to the saved state is being back at it.
#[test]
fn undoing_back_to_the_saved_state_is_not_unsaved_work() {
    let mut world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let mut history = CommandHistory::default();
    let saved_revision = history.revision();

    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetTransform3D {
        entity,
        transform: Some(Transform3D {
            position: [3.0, 0.0, 0.0],
            ..Transform3D::default()
        }),
    });
    history
        .apply(buffer.into_transaction("Move"), &mut world)
        .unwrap();
    assert_ne!(
        history.revision(),
        saved_revision,
        "an edit is unsaved work"
    );

    history.undo(&mut world).unwrap();
    assert_eq!(history.revision(), saved_revision, "and undoing it is not");
}

/// Every discarding action asks a question naming what it will do, so the
/// dialog cannot say "discard?" about closing the window.
#[test]
fn each_discarding_action_says_what_it_is_about_to_do() {
    for action in [
        Discarding::OpenAnother,
        Discarding::OpenPath(PathBuf::from("other.scene.json")),
        Discarding::Reload,
        Discarding::Reset,
        Discarding::Close,
    ] {
        assert!(action.question().ends_with('?'), "{action:?} must ask");
        assert!(!action.verb().is_empty(), "{action:?} needs a button");
    }
    assert_ne!(
        Discarding::Close.question(),
        Discarding::Reload.question(),
        "closing and reloading are different losses"
    );
}
