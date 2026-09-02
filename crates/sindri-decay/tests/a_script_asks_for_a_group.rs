//! Queries: a script asking the world about more than one entity.
//!
//! `World.find` names one entity by the name a scene gave it, which is the
//! wrong shape for a game that makes its enemies as it goes: they have no
//! authored names, and there are hundreds of them. A tag says what an entity
//! *is*, and a query over tags answers with all of them at once.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, TagsComponent, Transform3D,
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
}

fn tagged(world: &mut World, tags: &[&str], y: f32) -> EntityId {
    world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [0.0, y, 0.0],
            ..Transform3D::default()
        }),
        components: [(TagsComponent::TYPE_NAME.to_owned(), json!({ "tags": tags }))]
            .into_iter()
            .collect(),
        ..EntityData::default()
    })
}

/// A world with a scripted observer and whatever else the test puts in it.
fn observer(world: &mut World, script: &str) -> ScriptSources {
    world.spawn(EntityData {
        name: Some("Observer".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({
                "source": "observer.decay",
                "script": "Observer",
                "properties": { "tag": "enemy" }
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("observer.decay", script);
    sources
}

fn advance(world: &mut World, sources: &ScriptSources) -> ScriptReport {
    Scripts::new().advance(
        world,
        &registry(),
        ScriptFrame::new(sources, &InputState::default(), 1.0 / 60.0),
    )
}

/// The count a script wrote to the board, which is how a test reads a number
/// out of a script that has nowhere else to put one.
fn counted(report: &ScriptReport) -> Option<f64> {
    report
        .printed
        .first()
        .and_then(|message| message.message.parse().ok())
}

const COUNT: &str = r#"
script Observer {
    @export let tag: String = "";
    fn start() {
        print(World.with_tag(this.tag).len);
    }
}
"#;

#[test]
fn a_query_answers_with_every_entity_carrying_the_tag() {
    let mut world = World::default();
    tagged(&mut world, &["enemy"], 1.0);
    tagged(&mut world, &["enemy", "flying"], 2.0);
    tagged(&mut world, &["pickup"], 3.0);
    let sources = observer(&mut world, COUNT);

    let report = advance(&mut world, &sources);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(counted(&report), Some(2.0));
}

#[test]
fn a_tag_nothing_carries_answers_with_an_empty_group() {
    let mut world = World::default();
    tagged(&mut world, &["pickup"], 1.0);
    let sources = observer(&mut world, COUNT);

    let report = advance(&mut world, &sources);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(counted(&report), Some(0.0));
}

/// The filter every other walk of the world uses. An entity switched off takes
/// no part in anything else, and a query answering with one would be the odd
/// one out.
#[test]
fn a_switched_off_entity_is_not_in_the_answer() {
    let mut world = World::default();
    let off = tagged(&mut world, &["enemy"], 1.0);
    tagged(&mut world, &["enemy"], 2.0);
    world.get_mut(off).expect("just spawned").disabled = true;
    let sources = observer(&mut world, COUNT);

    let report = advance(&mut world, &sources);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(counted(&report), Some(1.0));
}

#[test]
fn a_script_reaches_through_every_entity_a_query_answers_with() {
    let mut world = World::default();
    let first = tagged(&mut world, &["enemy"], 10.0);
    let second = tagged(&mut world, &["enemy"], 20.0);
    let sources = observer(
        &mut world,
        r#"
        script Observer {
            @export let tag: String = "";
            fn update(dt: f32) {
                for enemy in World.with_tag(this.tag) {
                    enemy.transform.position.y -= 1.0;
                }
            }
        }
        "#,
    );

    let report = advance(&mut world, &sources);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    for (entity, expected) in [(first, 9.0), (second, 19.0)] {
        let position = world
            .get(entity)
            .expect("still there")
            .transform_3d
            .expect("a transform")
            .position;
        assert!((position[1] - expected).abs() < 1.0e-5, "{position:?}");
    }
}

/// The order is allocation order, which is deterministic. A game should not
/// depend on it for meaning, but a run should reproduce and a test should be
/// stable, and both need it to be the same order twice.
#[test]
fn the_answer_is_in_a_deterministic_order() {
    let mut world = World::default();
    tagged(&mut world, &["enemy"], 1.0);
    tagged(&mut world, &["enemy"], 2.0);
    tagged(&mut world, &["enemy"], 3.0);
    let sources = observer(
        &mut world,
        r#"
        script Observer {
            @export let tag: String = "";
            fn start() {
                let enemies: Array<Entity> = World.with_tag(this.tag);
                var total: f32 = 0.0;
                var weight: f32 = 1.0;
                for enemy in enemies {
                    total += enemy.transform.position.y * weight;
                    weight *= 10.0;
                }
                print(total);
            }
        }
        "#,
    );

    let report = advance(&mut world, &sources);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    // 1*1 + 2*10 + 3*100, which only holds if the walk is in spawn order.
    assert_eq!(counted(&report), Some(321.0));
}

/// A query is a snapshot of handles, so a script that despawns while walking
/// one is holding handles to entities that are gone. `World.exists` is how it
/// asks, and reaching through one without asking is an error rather than a
/// silent skip.
#[test]
fn a_handle_to_something_despawned_mid_walk_answers_exists_with_false() {
    let mut world = World::default();
    tagged(&mut world, &["enemy"], 1.0);
    tagged(&mut world, &["enemy"], 2.0);
    let sources = observer(
        &mut world,
        r#"
        script Observer {
            @export let tag: String = "";
            fn start() {
                let enemies: Array<Entity> = World.with_tag(this.tag);
                for enemy in enemies {
                    World.despawn(enemy);
                }
                var alive: f32 = 0.0;
                for enemy in enemies {
                    if World.exists(enemy) {
                        alive += 1.0;
                    }
                }
                print(alive);
            }
        }
        "#,
    );

    let report = advance(&mut world, &sources);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(counted(&report), Some(0.0));
}

/// A payload that is not a set of tags is an authoring mistake, and reading it
/// by hand would silently skip the entity instead of saying so.
#[test]
fn a_tags_payload_that_cannot_be_read_is_reported() {
    let mut world = World::default();
    world.spawn(EntityData {
        components: [(TagsComponent::TYPE_NAME.to_owned(), json!({ "tags": 7 }))]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    let sources = observer(&mut world, COUNT);

    let report = advance(&mut world, &sources);
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("could not be read"), "{message}");
}
