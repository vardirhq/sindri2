//! Effects: a script throwing flecks that are not entities.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::{EffectBurstComponent, Effects2d, SceneExtractor};

fn registry() -> ComponentSchemaRegistry {
    let mut registry = SceneExtractor::new()
        .expect("the builtin components register")
        .components()
        .clone();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// An entity with a script and an authored burst.
fn thrower(world: &mut World, at: [f32; 2], count: u32) -> EntityId {
    world.spawn(EntityData {
        name: Some("Bullet".to_owned()),
        transform_3d: Some(Transform3D {
            position: [at[0], at[1], 0.0],
            ..Transform3D::default()
        }),
        components: [
            (
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "bullet.decay", "script": "Bullet" }),
            ),
            (
                EffectBurstComponent::TYPE_NAME.to_owned(),
                json!({
                    "texture": "sindri:white",
                    "count": count,
                    "lifetime": 0.5,
                    "speed": 4.0
                }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

fn run(world: &mut World, source: &str, effects: &mut Effects2d) -> ScriptReport {
    let mut sources = ScriptSources::new();
    sources.insert("bullet.decay", source);
    let input = InputState::default();
    Scripts::new().advance(
        world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0).with_effects(effects),
    )
}

const THROW: &str = r"
script Bullet {
    fn start() { Effects.burst(this.entity); }
}
";

#[test]
fn a_script_throws_a_burst() {
    let mut world = World::default();
    thrower(&mut world, [0.0, 0.0], 12);
    let mut effects = Effects2d::default();
    let report = run(&mut world, THROW, &mut effects);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(effects.live(), 12);
}

/// A burst is thrown where the thing that threw it was.
#[test]
fn a_burst_lands_where_the_entity_is() {
    let mut world = World::default();
    thrower(&mut world, [3.0, -2.0], 4);
    let mut effects = Effects2d::default();
    run(&mut world, THROW, &mut effects);
    for fleck in effects.flecks() {
        assert!((fleck.position[0] - 3.0).abs() < 1.0e-5, "{fleck:?}");
        assert!((fleck.position[1] + 2.0).abs() < 1.0e-5, "{fleck:?}");
    }
}

/// The usual shape: throw a burst, then remove what threw it. The flecks are
/// already in the pool and owe the entity nothing.
#[test]
fn flecks_outlive_the_entity_that_threw_them() {
    let source = r"
    script Bullet {
        fn start() {
            Effects.burst(this.entity);
            World.despawn(this.entity);
        }
    }
    ";
    let mut world = World::default();
    let entity = thrower(&mut world, [1.0, 1.0], 8);
    let mut effects = Effects2d::default();
    let report = run(&mut world, source, &mut effects);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(world.get(entity).is_none(), "the thrower survived");
    assert_eq!(effects.live(), 8, "the flecks went with it");
}

/// An explosion where something *used to be*.
#[test]
fn a_burst_can_be_thrown_somewhere_else() {
    let source = r"
    script Bullet {
        fn start() { Effects.burst_at(this.entity, -5.0, 7.0); }
    }
    ";
    let mut world = World::default();
    thrower(&mut world, [0.0, 0.0], 3);
    let mut effects = Effects2d::default();
    run(&mut world, source, &mut effects);
    for fleck in effects.flecks() {
        assert!((fleck.position[0] + 5.0).abs() < 1.0e-5, "{fleck:?}");
        assert!((fleck.position[1] - 7.0).abs() < 1.0e-5, "{fleck:?}");
    }
}

/// A game that wants to turn itself down can see that it should.
#[test]
fn a_burst_reports_how_many_it_actually_made() {
    let source = r"
    script Bullet {
        fn start() { this.transform.position.z = Effects.burst(this.entity); }
    }
    ";
    let mut world = World::default();
    let entity = thrower(&mut world, [0.0, 0.0], 20);
    // Room for six, so the rest cannot be made.
    let mut effects = Effects2d::with_capacity(6);
    run(&mut world, source, &mut effects);
    let made = world
        .get(entity)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position[2];
    assert!((made - 20.0).abs() < 1.0e-5, "made {made}");
    assert_eq!(effects.live(), 6, "the pool grew past its capacity");
    assert!(effects.overflowed() > 0);
}

#[test]
fn a_script_can_ask_how_many_flecks_are_alive() {
    let source = r"
    script Bullet {
        fn start() {
            Effects.burst(this.entity);
            this.transform.position.z = Effects.live();
        }
    }
    ";
    let mut world = World::default();
    let entity = thrower(&mut world, [0.0, 0.0], 9);
    let mut effects = Effects2d::default();
    run(&mut world, source, &mut effects);
    let live = world
        .get(entity)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position[2];
    assert!((live - 9.0).abs() < 1.0e-5, "{live}");
}

/// An entity with no burst authored on it.
#[test]
fn an_entity_that_authors_no_burst_is_named() {
    let mut world = World::default();
    world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "bullet.decay", "script": "Bullet" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut effects = Effects2d::default();
    let report = run(&mut world, THROW, &mut effects);
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("authors no burst"), "{message}");
}

/// A game whose explosions never appear because nothing is drawing them should
/// hear about it.
#[test]
fn a_host_running_no_effects_says_so() {
    let mut world = World::default();
    thrower(&mut world, [0.0, 0.0], 4);
    let mut sources = ScriptSources::new();
    sources.insert("bullet.decay", THROW);
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
    assert!(message.contains("not running any effects"), "{message}");
}

/// Turning an explosion up must not change which enemies spawn.
#[test]
fn throwing_flecks_does_not_disturb_the_runs_numbers() {
    let source = r"
    script Bullet {
        fn update(dt: f32) {
            Effects.burst(this.entity);
            this.transform.position.z = Random.value();
        }
    }
    ";
    let drawn = |count: u32| {
        let mut world = World::default();
        let entity = thrower(&mut world, [0.0, 0.0], count);
        let mut effects = Effects2d::default();
        let mut rng = sindri_core::Rng::from_seed(11);
        let mut scripts = Scripts::new();
        let mut sources = ScriptSources::new();
        sources.insert("bullet.decay", source);
        let input = InputState::default();
        (0..8)
            .map(|_| {
                scripts.advance(
                    &mut world,
                    &registry(),
                    ScriptFrame::new(&sources, &input, 1.0 / 60.0)
                        .with_effects(&mut effects)
                        .with_random(&mut rng),
                );
                world
                    .get(entity)
                    .expect("there")
                    .transform_3d
                    .expect("a transform")
                    .position[2]
            })
            .collect::<Vec<f32>>()
    };
    assert_eq!(
        drawn(4),
        drawn(64),
        "a bigger explosion changed the run's numbers"
    );
}
