//! Randomness: a run whose numbers come from its seed.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, Rng, SceneComponent, TagsComponent, Transform3D,
    World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
        .register::<TagsComponent>("Tags")
        .expect("sindri.tags registers");
    registry
}

fn scripted(world: &mut World) -> EntityId {
    world.spawn(EntityData {
        name: Some("Spawner".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "spawner.decay", "script": "Spawner" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

fn run(world: &mut World, source: &str, rng: &mut Rng, scripts: &mut Scripts) -> ScriptReport {
    let mut sources = ScriptSources::new();
    sources.insert("spawner.decay", source);
    let input = InputState::default();
    scripts.advance(
        world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0).with_random(rng),
    )
}

/// Records each frame's number into the transform, so a run is a sequence.
const RECORD: &str = r"
script Spawner {
    fn update(dt: f32) {
        this.transform.position.x = Random.value();
    }
}
";

fn sequence(seed: u64, frames: usize) -> Vec<f32> {
    let mut world = World::default();
    let entity = scripted(&mut world);
    let mut rng = Rng::from_seed(seed);
    let mut scripts = Scripts::new();
    (0..frames)
        .map(|_| {
            run(&mut world, RECORD, &mut rng, &mut scripts);
            world
                .get(entity)
                .expect("there")
                .transform_3d
                .expect("a transform")
                .position[0]
        })
        .collect()
}

/// The whole promise: a seed is a run.
#[test]
fn the_same_seed_replays_the_same_run() {
    assert_eq!(sequence(42, 60), sequence(42, 60));
}

#[test]
fn a_different_seed_is_a_different_run() {
    assert_ne!(sequence(1, 60), sequence(2, 60));
}

/// A stream that did not move would hand out the same number for ever.
#[test]
fn the_stream_moves_between_frames() {
    let drawn = sequence(9, 30);
    let first = drawn[0];
    assert!(
        drawn.iter().any(|value| (value - first).abs() > 1.0e-6),
        "every frame drew {first}"
    );
    assert!(drawn.iter().all(|value| (0.0..1.0).contains(value)));
}

#[test]
fn a_range_stays_inside_its_ends() {
    let source = r"
    script Spawner {
        fn update(dt: f32) {
            this.transform.position.x = Random.range(-8.0, 8.0);
        }
    }
    ";
    let mut world = World::default();
    let entity = scripted(&mut world);
    let mut rng = Rng::from_seed(3);
    let mut scripts = Scripts::new();
    for _ in 0..500 {
        run(&mut world, source, &mut rng, &mut scripts);
        let x = world
            .get(entity)
            .expect("there")
            .transform_3d
            .expect("a transform")
            .position[0];
        assert!((-8.0..8.0).contains(&x), "{x}");
    }
}

/// "A number from 1 to 6" means six outcomes.
#[test]
fn a_whole_number_reaches_both_of_its_ends() {
    let source = r"
    script Spawner {
        fn update(dt: f32) {
            this.transform.position.x = Random.int(1.0, 6.0);
        }
    }
    ";
    let mut world = World::default();
    let entity = scripted(&mut world);
    let mut rng = Rng::from_seed(4);
    let mut scripts = Scripts::new();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..500 {
        run(&mut world, source, &mut rng, &mut scripts);
        let value = world
            .get(entity)
            .expect("there")
            .transform_3d
            .expect("a transform")
            .position[0];
        assert!(
            (value - value.round()).abs() < 1.0e-5,
            "{value} is not whole"
        );
        #[allow(clippy::cast_possible_truncation)]
        seen.insert(value.round() as i32);
    }
    assert_eq!(
        seen,
        (1..=6).collect(),
        "a die that does not roll every face"
    );
}

/// Choosing from a group is most of what a game wants randomness for, and Decay
/// has no indexing to do it with.
#[test]
fn a_script_picks_one_of_a_group() {
    let source = r#"
    script Spawner {
        fn update(dt: f32) {
            let enemies = World.with_tag("enemy");
            if enemies.len > 0.0 {
                let chosen = Random.pick(enemies);
                chosen.transform.position.z = chosen.transform.position.z + 1.0;
            }
        }
    }
    "#;
    let mut world = World::default();
    scripted(&mut world);
    let enemies: Vec<EntityId> = (0..4)
        .map(|_| {
            world.spawn(EntityData {
                transform_3d: Some(Transform3D::default()),
                components: [(
                    TagsComponent::TYPE_NAME.to_owned(),
                    json!({ "tags": ["enemy"] }),
                )]
                .into_iter()
                .collect(),
                ..EntityData::default()
            })
        })
        .collect();

    let mut rng = Rng::from_seed(5);
    let mut scripts = Scripts::new();
    for _ in 0..400 {
        run(&mut world, source, &mut rng, &mut scripts);
    }
    let picked: Vec<f32> = enemies
        .iter()
        .map(|entity| {
            world
                .get(*entity)
                .expect("there")
                .transform_3d
                .expect("a transform")
                .position[2]
        })
        .collect();
    assert!(
        picked.iter().all(|count| *count > 50.0),
        "some of the group were never chosen: {picked:?}"
    );
    let total: f32 = picked.iter().sum();
    assert!((total - 400.0).abs() < 1.0e-3, "{total}");
}

/// Picking from nothing is refused rather than answered with an entity that is
/// not there.
#[test]
fn picking_from_nothing_is_refused() {
    let source = r#"
    script Spawner {
        fn update(dt: f32) {
            let none = World.with_tag("absent");
            Random.pick(none);
        }
    }
    "#;
    let mut world = World::default();
    scripted(&mut world);
    let report = run(&mut world, source, &mut Rng::default(), &mut Scripts::new());
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("nothing to pick from"), "{message}");
}

/// A spawner that never spawns anywhere is worth hearing about.
#[test]
fn a_range_that_runs_backwards_is_refused() {
    let source = r"
    script Spawner {
        fn update(dt: f32) { Random.range(8.0, -8.0); }
    }
    ";
    let mut world = World::default();
    scripted(&mut world);
    let report = run(&mut world, source, &mut Rng::default(), &mut Scripts::new());
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("runs backwards"), "{message}");
}

/// A game that wants a different run each time seeds itself from something it
/// knows, because the engine has nothing to offer.
#[test]
fn a_script_can_start_a_run_from_a_seed_it_chose() {
    let source = r"
    script Spawner {
        @export let run_seed: f32 = 0.0;
        fn start() { Random.seed(this.run_seed); }
        fn update(dt: f32) { this.transform.position.x = Random.value(); }
    }
    ";
    let drawn = |seed: f32| {
        let mut world = World::default();
        let entity = world.spawn(EntityData {
            transform_3d: Some(Transform3D::default()),
            components: [(
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({
                    "source": "spawner.decay",
                    "script": "Spawner",
                    "properties": { "run_seed": seed }
                }),
            )]
            .into_iter()
            .collect(),
            ..EntityData::default()
        });
        // The same host stream every time, so only the script's seed differs.
        let mut rng = Rng::default();
        let mut scripts = Scripts::new();
        (0..10)
            .map(|_| {
                run(&mut world, source, &mut rng, &mut scripts);
                world
                    .get(entity)
                    .expect("there")
                    .transform_3d
                    .expect("a transform")
                    .position[0]
            })
            .collect::<Vec<f32>>()
    };
    assert_eq!(drawn(77.0), drawn(77.0), "the same seed is the same run");
    assert_ne!(drawn(77.0), drawn(78.0), "the seed did nothing");
}

/// A game whose waves never vary because nothing is seeding them should hear
/// about it on the first frame.
#[test]
fn a_host_running_no_stream_says_so() {
    let mut world = World::default();
    scripted(&mut world);
    let mut sources = ScriptSources::new();
    sources.insert("spawner.decay", RECORD);
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
    assert!(message.contains("not running a random stream"), "{message}");
}
