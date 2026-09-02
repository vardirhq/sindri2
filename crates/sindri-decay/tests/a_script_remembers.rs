//! Persistence: a game keeping something between runs, and handling what it
//! finds when it comes back.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SaveDocument, SaveReadError, SaveStore,
    SaveValue, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn scripted(world: &mut World) -> EntityId {
    world.spawn(EntityData {
        name: Some("Progress".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "progress.decay", "script": "Progress" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

fn run(world: &mut World, source: &str, saves: &mut SaveStore) -> ScriptReport {
    let mut sources = ScriptSources::new();
    sources.insert("progress.decay", source);
    let input = InputState::default();
    Scripts::new().advance(
        world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0).with_saves(saves),
    )
}

fn recorded(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position
}

/// The acceptance test's ninth line: store something, come back, find it.
#[test]
fn what_a_run_stored_is_there_on_the_next_one() {
    let store_it = r#"
    script Progress {
        fn start() {
            Save.set_number("best_wave", 12.0);
            Save.set_flag("seen_intro", true);
        }
    }
    "#;
    let read_it = r#"
    script Progress {
        fn start() {
            this.transform.position.x = Save.number("best_wave", 0.0);
            if Save.flag("seen_intro", false) {
                this.transform.position.y = 1.0;
            }
        }
    }
    "#;

    // The first run, ending with what the host would write out.
    let mut first = World::default();
    scripted(&mut first);
    let mut saves = SaveStore::default();
    run(&mut first, store_it, &mut saves);
    assert!(saves.is_dirty(), "nothing was marked for writing");
    let written = saves.to_document();

    // A later run, opening what was written.
    let mut second = World::default();
    let entity = scripted(&mut second);
    let mut reopened = SaveStore::opened(Ok(Some(written)));
    let report = run(&mut second, read_it, &mut reopened);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let at = recorded(&second, entity);
    assert!((at[0] - 12.0).abs() < 1.0e-5, "{at:?}");
    assert!((at[1] - 1.0).abs() < 1.0e-5, "{at:?}");
}

#[test]
fn a_key_that_was_never_stored_is_the_fallback() {
    let source = r#"
    script Progress {
        fn start() {
            this.transform.position.x = Save.number("absent", 7.0);
            if Save.has("absent") { this.transform.position.y = 1.0; }
        }
    }
    "#;
    let mut world = World::default();
    let entity = scripted(&mut world);
    run(&mut world, source, &mut SaveStore::default());
    let at = recorded(&world, entity);
    assert!((at[0] - 7.0).abs() < 1.0e-5, "{at:?}");
    assert!(at[1].abs() < 1.0e-5, "has said yes to nothing");
}

/// A first run starts cheerfully; a damaged save is worth saying something
/// about. They must not look the same to a script.
#[test]
fn a_first_run_and_a_damaged_save_are_told_apart() {
    let source = r"
    script Progress {
        fn start() {
            if Save.is_new() { this.transform.position.x = 1.0; }
            if Save.is_damaged() { this.transform.position.y = 1.0; }
            if Save.is_from_newer() { this.transform.position.z = 1.0; }
        }
    }
    ";
    let cases = [
        (SaveStore::default(), [1.0, 0.0, 0.0]),
        (
            SaveStore::opened(Err(SaveReadError::Unreadable("torn".to_owned()))),
            [0.0, 1.0, 0.0],
        ),
        (
            SaveStore::opened(Ok(Some(SaveDocument {
                version: sindri_core::SAVE_FORMAT_VERSION + 1,
                values: std::collections::BTreeMap::new(),
            }))),
            [0.0, 0.0, 1.0],
        ),
    ];
    for (mut saves, expected) in cases {
        let mut world = World::default();
        let entity = scripted(&mut world);
        run(&mut world, source, &mut saves);
        let at = recorded(&world, entity);
        for axis in 0..3 {
            assert!((at[axis] - expected[axis]).abs() < 1.0e-5, "{at:?}");
        }
    }
}

/// "Reset my progress" is a real feature, and it has to survive the write.
#[test]
fn clearing_forgets_everything_and_needs_writing_out() {
    let source = r"
    script Progress {
        fn start() { Save.clear(); }
    }
    ";
    let mut saves = SaveStore::default();
    saves.set("best_wave", SaveValue::Number(30.0));
    saves.mark_written();

    let mut world = World::default();
    scripted(&mut world);
    run(&mut world, source, &mut saves);
    assert!(!saves.has("best_wave"));
    assert!(saves.is_dirty(), "a reset that never gets written out");
}

/// A game that stores its volume every frame should not keep a disk busy.
#[test]
fn storing_the_same_value_again_does_not_ask_for_a_write() {
    let source = r#"
    script Progress {
        fn update(dt: f32) { Save.set_number("volume", 0.5); }
    }
    "#;
    let mut world = World::default();
    scripted(&mut world);
    let mut saves = SaveStore::default();
    run(&mut world, source, &mut saves);
    assert!(saves.is_dirty(), "the first write was not noticed");
    saves.mark_written();
    for _ in 0..10 {
        run(&mut world, source, &mut saves);
    }
    assert!(
        !saves.is_dirty(),
        "the same value asked to be written again"
    );
}

/// A NaN written to a save comes back next run and poisons whatever reads it.
#[test]
fn a_value_that_is_not_a_number_is_refused() {
    let source = r#"
    script Progress {
        fn start() { Save.set_number("score", 0.0 / 0.0); }
    }
    "#;
    let mut world = World::default();
    scripted(&mut world);
    let mut saves = SaveStore::default();
    let report = run(&mut world, source, &mut saves);
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("not worth remembering"), "{message}");
    assert!(!saves.has("score"));
}

/// A game whose progress silently never persists should be heard about on the
/// first frame, not after someone has played for an hour.
#[test]
fn a_host_keeping_no_save_says_so() {
    let source = r#"
    script Progress {
        fn start() { Save.set_number("score", 1.0); }
    }
    "#;
    let mut world = World::default();
    scripted(&mut world);
    let mut sources = ScriptSources::new();
    sources.insert("progress.decay", source);
    let input = InputState::default();
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0),
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("not keeping a save"), "{message}");
}

/// The whole round trip through a real backend, which is what the acceptance
/// test actually asks for.
#[test]
fn a_save_survives_a_backend_it_was_written_through() {
    use sindri_platform::{MemorySaves, SaveBackend};

    let store_it = r#"
    script Progress {
        fn start() { Save.set_number("currency", 250.0); }
    }
    "#;
    let read_it = r#"
    script Progress {
        fn start() { this.transform.position.x = Save.number("currency", 0.0); }
    }
    "#;
    let mut backend = MemorySaves::new();

    let mut first = World::default();
    scripted(&mut first);
    let mut saves = SaveStore::opened(backend.read());
    run(&mut first, store_it, &mut saves);
    if saves.is_dirty() {
        backend.write(&saves.to_document()).expect("writable");
        saves.mark_written();
    }
    assert_eq!(backend.writes(), 1);

    let mut second = World::default();
    let entity = scripted(&mut second);
    let mut reopened = SaveStore::opened(backend.read());
    run(&mut second, read_it, &mut reopened);
    assert!((recorded(&second, entity)[0] - 250.0).abs() < 1.0e-5);
}
