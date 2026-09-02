//! Spawning: a script creating something that was not in the scene.
//!
//! The gap this closes was the first one in the way of a real action game. A
//! script could find, reach through, check, and remove another entity, and it
//! could not make one — so every enemy, bullet, pickup and effect a game would
//! ever have had to be placed by hand before the run began.
//!
//! Three things had to be true for it to be worth calling done, and each has a
//! test here: the entity is real and reachable on the frame it was made, its
//! script has *started* by the end of that frame, and a per-instance starting
//! value can be set before that script's first callback.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, PrefabDocument, SceneComponent, SceneEntity,
    SceneEntityId, Transform3D, World,
};
use sindri_decay::{
    PrefabSources, ScriptComponent, ScriptFailure, ScriptReport, ScriptSources, Scripts,
};
use sindri_platform::InputState;

const BULLET: &str = "prefabs/bullet.prefab.json";

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn id(name: &str) -> SceneEntityId {
    SceneEntityId::new(name).expect("a non-empty literal is a valid identity")
}

/// A prefab of one entity, optionally running a script of its own.
fn prefab(script: Option<&str>) -> PrefabDocument {
    let mut root = SceneEntity {
        name: Some("Bullet".to_owned()),
        transform_3d: Some(Transform3D::default()),
        ..SceneEntity::new(id("bullet"))
    };
    if let Some(script) = script {
        root.components.insert(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "bullet.decay", "script": script }),
        );
    }
    PrefabDocument::single(root)
}

/// A world with one scripted spawner that has `BULLET` authored on `bullet`.
fn world(spawner: &str, bullet: Option<&str>) -> (World, ScriptSources, PrefabSources) {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Spawner".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({
                "source": "spawner.decay",
                "script": "Spawner",
                "properties": { "bullet": BULLET }
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });

    let mut sources = ScriptSources::new();
    sources.insert("spawner.decay", spawner);
    if let Some(bullet) = bullet {
        sources.insert("bullet.decay", bullet);
    }
    let mut prefabs = PrefabSources::new();
    prefabs.insert(BULLET, prefab(bullet.map(|_| "Bullet")));
    (world, sources, prefabs)
}

fn advance(
    scripts: &mut Scripts,
    world: &mut World,
    sources: &ScriptSources,
    prefabs: &PrefabSources,
) -> ScriptReport {
    scripts.advance(
        world,
        &registry(),
        sources,
        prefabs,
        &InputState::default(),
        1.0 / 60.0,
    )
}

fn named<'a>(world: &'a World, name: &str) -> Vec<&'a EntityData> {
    world
        .entities()
        .filter(|(_, data)| data.name.as_deref() == Some(name))
        .map(|(_, data)| data)
        .collect()
}

#[test]
fn a_script_creates_the_entity_a_prefab_describes() {
    let (mut world, sources, prefabs) = world(
        r"
        script Spawner {
            @export let bullet: Prefab;
            fn start() {
                let shot = World.spawn(this.bullet);
                shot.transform.position.x = 12.0;
            }
        }
        ",
        None,
    );
    let report = advance(&mut Scripts::new(), &mut world, &sources, &prefabs);
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    // Reachable on the frame it was made: the write above landed, which it
    // could only do if the entity was really in the world by then.
    let bullets = named(&world, "Bullet");
    assert_eq!(bullets.len(), 1);
    assert!((bullets[0].transform_3d.expect("a transform").position[0] - 12.0).abs() < 1.0e-5);
}

#[test]
fn a_spawned_script_has_started_by_the_end_of_the_frame_that_made_it() {
    let (mut world, sources, prefabs) = world(
        r"
        script Spawner {
            @export let bullet: Prefab;
            var made: bool = false;
            fn update(dt: f32) {
                if !this.made {
                    this.made = true;
                    World.spawn(this.bullet);
                }
            }
        }
        ",
        // The bullet moves in its own update, so a bullet that has not started
        // is one that sat still for a frame.
        Some(
            r"
        script Bullet {
            fn start() { this.transform.position.y = 100.0; }
            fn update(dt: f32) { this.transform.position.x += 1.0; }
        }
        ",
        ),
    );
    let mut scripts = Scripts::new();
    let report = advance(&mut scripts, &mut world, &sources, &prefabs);
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    let bullets = named(&world, "Bullet");
    let position = bullets[0].transform_3d.expect("a transform").position;
    assert!(
        (position[1] - 100.0).abs() < 1.0e-5,
        "start did not run on the frame the bullet was made"
    );
    assert!(
        (position[0] - 1.0).abs() < 1.0e-5,
        "update did not run on the frame the bullet was made"
    );
}

#[test]
fn a_starting_value_is_authored_before_the_spawned_script_first_runs() {
    let (mut world, sources, prefabs) = world(
        r#"
        script Spawner {
            @export let bullet: Prefab;
            var made: bool = false;
            fn update(dt: f32) {
                if !this.made {
                    this.made = true;
                    let shot = World.spawn(this.bullet);
                    World.set_property(shot, "speed", 7.0);
                }
            }
        }
        "#,
        Some(
            r"
        script Bullet {
            @export let speed: f32 = 1.0;
            fn start() { this.transform.position.x = this.speed; }
        }
        ",
        ),
    );
    let report = advance(&mut Scripts::new(), &mut world, &sources, &prefabs);
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    let bullets = named(&world, "Bullet");
    assert!(
        (bullets[0].transform_3d.expect("a transform").position[0] - 7.0).abs() < 1.0e-5,
        "the spawner's value did not reach the script's first callback"
    );
}

#[test]
fn authoring_a_property_on_a_running_script_is_refused_rather_than_ignored() {
    let (mut world, sources, prefabs) = world(
        r#"
        script Spawner {
            @export let bullet: Prefab;
            fn update(dt: f32) {
                World.set_property(this.entity, "bullet", "anything");
            }
        }
        "#,
        None,
    );
    let report = advance(&mut Scripts::new(), &mut world, &sources, &prefabs);
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(
        message.contains("already started"),
        "a write that changes nothing should say so: {message}"
    );
}

#[test]
fn a_prefab_field_the_scene_never_filled_in_is_named_rather_than_spawned() {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Spawner".to_owned()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "spawner.decay", "script": "Spawner" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "spawner.decay",
        r"
        script Spawner {
            @export let bullet: Prefab;
            fn start() { World.spawn(this.bullet); }
        }
        ",
    );

    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources,
        &PrefabSources::new(),
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(
        message.contains("has not authored"),
        "an unauthored prefab field should say so: {message}"
    );
    assert_eq!(world.len(), 1, "nothing was created");
}

#[test]
fn a_prefab_the_host_never_loaded_is_named_rather_than_spawned() {
    let (mut world, sources, _) = world(
        r"
        script Spawner {
            @export let bullet: Prefab;
            fn start() { World.spawn(this.bullet); }
        }
        ",
        None,
    );
    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources,
        &PrefabSources::new(),
    );
    assert!(
        report.failures.iter().any(
            |failure| matches!(failure, ScriptFailure::Runtime { error, .. }
                if error.contains("has not loaded"))
        ),
        "{:?}",
        report.failures
    );
}

#[test]
fn a_script_cannot_spawn_a_prefab_it_names_as_text() {
    // The restriction that makes prefab references loadable and checkable. A
    // literal would be invisible to the asset pipeline, so the analyzer refuses
    // it rather than the host discovering it on the frame it spawns.
    let (mut world, sources, prefabs) = world(
        r#"
        script Spawner {
            @export let bullet: Prefab;
            fn start() { World.spawn("prefabs/bullet.prefab.json"); }
        }
        "#,
        None,
    );
    let report = advance(&mut Scripts::new(), &mut world, &sources, &prefabs);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| matches!(failure, ScriptFailure::Compile { .. })),
        "a prefab named as text should not compile: {:?}",
        report.failures
    );
}

#[test]
fn a_spawn_cascade_that_does_not_settle_is_reported_rather_than_run() {
    // Each bullet spawns another bullet, forever. The rounds are bounded, so
    // the frame ends and says why rather than never ending.
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Spawner".to_owned()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({
                "source": "bullet.decay",
                "script": "Bullet",
                "properties": { "bullet": BULLET }
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "bullet.decay",
        r#"
        script Bullet {
            @export let bullet: Prefab;
            fn start() {
                let next = World.spawn(this.bullet);
                World.set_property(next, "bullet", "prefabs/bullet.prefab.json");
            }
        }
        "#,
    );
    let mut endless = SceneEntity {
        name: Some("Bullet".to_owned()),
        ..SceneEntity::new(id("bullet"))
    };
    endless.components.insert(
        ScriptComponent::TYPE_NAME.to_owned(),
        json!({ "source": "bullet.decay", "script": "Bullet" }),
    );
    let mut prefabs = PrefabSources::new();
    prefabs.insert(BULLET, PrefabDocument::single(endless));

    let report = advance(&mut Scripts::new(), &mut world, &sources, &prefabs);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| matches!(failure, ScriptFailure::SpawnCascade { .. })),
        "{:?}",
        report.failures
    );
}
