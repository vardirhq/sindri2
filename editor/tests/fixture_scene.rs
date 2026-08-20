//! What the editor does to a scene file, end to end.
//!
//! The unit tests beside `SceneFile` prove the operations in isolation against
//! a scene built for the occasion. These run the editor's real path against the
//! real fixture: open the file, load a world, edit it through the same command
//! history the interface uses, save, and open it again. Nothing here needs a
//! GPU — extraction is inspected directly, so a fixture that stopped rendering
//! fails here rather than in a screenshot.
//!
//! The fixture file itself is golden. Regenerate it deliberately with
//! `SINDRI_UPDATE_SCENE_FIXTURES=1 cargo test --package sindri-editor`.

use std::{fs, path::PathBuf};

use sindri_core::{
    CommandBuffer, CommandHistory, EntityId, SceneDocument, SceneEntityId, Transform3D,
    UnknownComponentPolicy, World, WorldCommand,
};
use sindri_editor::{fixture, scene_file::SceneFile};
use std::path::Path;

use sindri_render::{RenderStage, TextureId, Viewport};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings};

const UPDATE_ENV: &str = "SINDRI_UPDATE_SCENE_FIXTURES";
const VIEWPORT: Viewport = Viewport::new(512, 512);

/// Where the fixture's textures resolve, which is the directory it lives in.
const FIXTURE_ASSETS: &str = "assets";

fn document() -> SceneDocument {
    fixture::open()
        .expect("the fixture opens")
        .document()
        .clone()
}

/// The fixture copied somewhere a test may write to, with the copy already
/// open and loaded. The directory is returned because dropping it deletes it.
fn scratch() -> (tempfile::TempDir, PathBuf, SceneFile, World) {
    let directory = tempfile::tempdir().expect("a scratch directory");
    let path = directory.path().join("fixture.scene.json");
    fs::copy(fixture::path(), &path).expect("the fixture copies");
    let file = SceneFile::open(&path).expect("the copy opens");
    let world = World::from_scene(file.document())
        .expect("the fixture loads")
        .world;
    (directory, path, file, world)
}

/// The runtime handle of the entity the fixture calls `id`.
///
/// Handles belong to the world that minted them, so this always takes the world
/// the caller is about to edit rather than loading one of its own.
fn entity(world: &World, id: &str) -> EntityId {
    let wanted = SceneEntityId::new(id).expect("a fixture id is a valid scene id");
    let Some((entity, _)) = world
        .entities()
        .find(|(_, data)| data.source_id.as_ref() == Some(&wanted))
    else {
        panic!("the fixture has an entity called {id}");
    };
    entity
}

fn move_cube(world: &mut World, history: &mut CommandHistory, position: [f32; 3]) -> Transform3D {
    let moved = Transform3D {
        position,
        ..Transform3D::default()
    };
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetTransform3D {
        entity: entity(world, "cube"),
        transform: Some(moved),
    });
    history
        .apply(buffer.into_transaction("Move Cube"), world)
        .expect("the command applies");
    moved
}

/// The editor opens a scene carrying a component it has never heard of, keeps
/// the payload through an edit, and writes it back untouched.
///
/// It used to refuse such a scene, and from the command line it panicked before
/// the window opened. The format exists to carry exactly this: a project that
/// defines a component of its own is the normal case, not a broken file.
#[test]
fn a_scene_with_an_unknown_component_opens_and_keeps_it() {
    let directory = tempfile::tempdir().expect("a scratch directory");
    let path = directory.path().join("custom.scene.json");
    fs::write(
        &path,
        r#"{
  "format_version": 3,
  "entities": [
    { "id": "camera", "transform_3d": { "position": [3.0, 2.0, 4.0] },
      "components": { "sindri.camera": { "projection": "perspective",
        "target": [0.0, 0.0, 0.0], "up": [0.0, 1.0, 0.0],
        "vertical_fov_degrees": 45.0, "near": 0.1, "far": 100.0 } } },
    { "id": "player", "transform_3d": { "position": [1.0, 0.0, 0.0] },
      "components": { "game.health": { "hit_points": 12, "regen": "slow" } } }
  ]
}"#,
    )
    .expect("the scene is written");

    let mut file = SceneFile::open(&path).expect("the scene opens");
    let extractor = SceneExtractor::new().expect("the built-in components register");
    let mut world =
        sindri_editor::native::load_world(&extractor, file.document()).expect("and loads");

    let player = entity(&world, "player");
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetTransform3D {
        entity: player,
        transform: Some(Transform3D {
            position: [4.0, 0.0, 0.0],
            ..Transform3D::default()
        }),
    });
    CommandHistory::default()
        .apply(buffer.into_transaction("Move"), &mut world)
        .expect("editing an entity beside an unknown component is ordinary");

    file.save(&world).expect("the scene saves");
    let reopened = SceneFile::open(&path).expect("the scene opens again");
    let carried = reopened
        .document()
        .entity(&SceneEntityId::new("player").unwrap())
        .expect("the entity survived")
        .components
        .get("game.health")
        .expect("and so did the component the editor knows nothing about");
    assert_eq!(carried["hit_points"], 12);
    assert_eq!(carried["regen"], "slow");
}

/// A Z lock declared in a file is respected by the path the interface writes
/// through, and survives being saved and opened again. The inspector takes the
/// Z drag away as well, but this is the half that holds when something other
/// than the inspector does the writing.
#[test]
fn a_locked_transform_refuses_a_move_and_keeps_its_lock_through_a_save() {
    let (_directory, path, mut file, mut world) = scratch();
    let cube = entity(&world, "cube");
    let mut history = CommandHistory::default();

    let locked = Transform3D {
        position: [0.0, 0.0, -4.0],
        z_locked: true,
        ..Transform3D::default()
    };
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetTransform3D {
        entity: cube,
        transform: Some(locked),
    });
    history
        .apply(buffer.into_transaction("Lock Cube"), &mut world)
        .expect("declaring a lock changes no layer");

    let mut flattened = locked;
    flattened.position[2] = 0.0;
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetTransform3D {
        entity: cube,
        transform: Some(flattened),
    });
    history
        .apply(buffer.into_transaction("Move Cube"), &mut world)
        .expect_err("a locked transform must refuse to leave its layer");

    file.save(&world).expect("the copy saves");
    let reopened = SceneFile::open(&path).expect("the copy opens again");
    let saved = World::from_scene(reopened.document())
        .expect("the saved scene loads")
        .world;
    let transform = saved
        .get(entity(&saved, "cube"))
        .and_then(|data| data.transform_3d)
        .expect("the cube kept its transform");
    assert!(transform.z_locked, "the lock is part of the scene");
    assert_eq!(
        transform.position[2].to_bits(),
        (-4.0_f32).to_bits(),
        "and the refused move never happened"
    );
}

/// The fixture's whole reason to exist. If it stops holding one of each, every
/// assertion below still passes while testing something narrower.
#[test]
fn the_fixture_is_one_cube_one_sprite_and_the_cameras_they_need() {
    let document = document();
    let count = |type_name: &str| {
        document
            .entities
            .iter()
            .filter(|entity| entity.components.contains_key(type_name))
            .count()
    };

    assert_eq!(count("sindri.mesh"), 1, "the fixture is one cube");
    assert_eq!(count("sindri.sprite"), 1, "the fixture is one sprite");
    assert_eq!(
        count("sindri.camera"),
        2,
        "a mesh needs a world camera and a sprite resolves its anchor against \
         an overlay camera, so the minimum is two rather than one"
    );
    assert_eq!(document.entities.len(), 4, "and nothing else");
}

/// `Reject` is the strong claim: every component in the fixture is one the
/// engine understands, rather than JSON that happens to parse.
#[test]
fn every_component_in_the_fixture_matches_a_built_in_schema() {
    SceneExtractor::new()
        .expect("the built-in components register")
        .validate(&document(), UnknownComponentPolicy::Reject)
        .expect("the fixture uses only components the engine understands");
}

#[test]
fn the_fixture_extracts_to_one_mesh_pass_and_one_sprite_pass() {
    let world = World::from_scene(&document())
        .expect("the fixture loads")
        .world;
    // Bound from what the fixture names, so the test does not carry its own
    // idea of which textures the scene uses.
    let mut textures = TextureBindings::new();
    for (index, reference) in sindri_scene::referenced_textures(&world).iter().enumerate() {
        let id = u32::try_from(index).expect("a fixture's textures fit a u32") + 1;
        textures.bind(reference.as_str(), TextureId::new(id));
    }

    let frame = SceneExtractor::new()
        .expect("the built-in components register")
        .extract(&world, VIEWPORT, CameraView::default(), &textures)
        .expect("the fixture extracts");

    let stages: Vec<RenderStage> = frame.passes().iter().map(|pass| pass.stage).collect();
    assert_eq!(stages, [RenderStage::Opaque3d, RenderStage::Overlay]);
}

/// Every texture the fixture names is one the editor can bind, so opening it
/// does not greet you with the missing-texture checker.
#[test]
fn every_texture_the_fixture_names_can_actually_be_drawn() {
    let document = document();
    let named: Vec<&str> = document
        .entities
        .iter()
        .filter_map(|entity| {
            entity
                .components
                .get("sindri.mesh")
                .or_else(|| entity.components.get("sindri.sprite"))
        })
        .filter_map(|payload| payload.get("texture")?.as_str())
        .collect();
    assert!(!named.is_empty(), "the fixture draws something");

    for texture in &named {
        // Either the engine generates it, or it is a file sitting beside the
        // scene. Nothing else resolves, and a fixture that opened showing the
        // missing-texture checker would be a poor thing to start the editor in.
        if sindri_scene::PROCEDURAL_TEXTURES
            .iter()
            .any(|procedural| procedural.reference == *texture)
        {
            continue;
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE_ASSETS)
            .join(texture);
        assert!(
            path.is_file(),
            "{texture} is neither generated nor a file at {}",
            path.display()
        );
    }
}

#[test]
fn the_fixture_file_is_canonical() {
    let path = fixture::path();
    let stored = fs::read_to_string(&path).expect("the fixture is readable");
    let canonical = SceneDocument::from_json(&stored)
        .expect("the fixture parses")
        .to_canonical_json()
        .expect("the fixture serializes");

    if stored != canonical && std::env::var_os(UPDATE_ENV).is_some() {
        fs::write(&path, &canonical).expect("the fixture is writable");
        return;
    }

    assert_eq!(
        stored, canonical,
        "the fixture is not in canonical form; regenerate it with \
         {UPDATE_ENV}=1 cargo test --package sindri-editor"
    );
}

/// The whole point of a scene being a file: an edit made in the editor is still
/// there next time. This goes through `CommandHistory` rather than writing the
/// world directly, because that is the path the interface takes.
#[test]
fn an_edit_made_through_a_command_survives_a_save_and_reopen() {
    let (_directory, path, mut file, mut world) = scratch();
    let moved = move_cube(
        &mut world,
        &mut CommandHistory::default(),
        [1.5, -2.0, 3.25],
    );

    file.save(&world).expect("the scene saves");

    let reopened = SceneFile::open(&path).expect("the saved scene reopens");
    let saved = reopened
        .document()
        .entities
        .iter()
        .find(|entity| entity.id.as_str() == "cube")
        .expect("the cube is still in the file");
    assert_eq!(saved.transform_3d, Some(moved));
}

/// Undo is only trustworthy if it lands exactly on what the file holds rather
/// than somewhere near it. Canonical bytes make "exactly" checkable.
#[test]
fn undoing_every_edit_saves_the_file_back_as_it_was() {
    let (_directory, path, mut file, mut world) = scratch();
    let original = fs::read_to_string(&path).expect("the copy is readable");

    let mut history = CommandHistory::default();
    move_cube(&mut world, &mut history, [9.0, 9.0, 9.0]);
    history.undo(&mut world).expect("undo applies");

    file.save(&world).expect("the scene saves");
    assert_eq!(
        fs::read_to_string(&path).expect("the saved scene is readable"),
        original,
        "an edit and its undo should leave the file byte for byte unchanged"
    );
}

/// Saving an untouched scene has to be a no-op on disk, or opening the editor
/// and pressing save would rewrite files nobody edited.
#[test]
fn saving_the_untouched_fixture_leaves_the_file_identical() {
    let (_directory, path, mut file, world) = scratch();
    let original = fs::read_to_string(&path).expect("the copy is readable");

    file.save(&world).expect("the scene saves");

    assert_eq!(
        fs::read_to_string(&path).expect("the saved scene is readable"),
        original
    );
}

#[test]
fn reloading_throws_away_what_was_never_saved() {
    let (_directory, _path, mut file, mut world) = scratch();
    let before = file.document().clone();

    move_cube(&mut world, &mut CommandHistory::default(), [4.0, 0.0, 0.0]);

    file.reload().expect("the scene reloads");
    assert_eq!(
        file.document(),
        &before,
        "reload should have gone back to what the file holds"
    );
}
