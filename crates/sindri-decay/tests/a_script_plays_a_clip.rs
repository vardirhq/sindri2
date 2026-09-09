//! Animation: a script naming a clip, and being told when it ended.
//!
//! The gap this closes is that the engine advanced clips and the editor
//! authored them, and no script could reach either — so an animation could be
//! made and previewed and never driven by a game. Which clip plays is the
//! authored half and goes into the world; where it has got to is derived and
//! lives beside it. Both halves are exercised here.
//!
//! The animations are advanced by hand rather than by a session driver, for the
//! reason the physics test gives: this crate must not depend on a scene-side
//! driver to give a script the surface.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::{SpriteAnimationComponent, SpriteAnimations};

const STEP: f32 = 1.0 / 60.0;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
        .register::<SpriteAnimationComponent>("Sprite Animation")
        .expect("sindri.animation.sprite registers");
    registry
}

/// An entity with two clips: one that loops and one that ends.
///
/// Frame times are one step each, so a clip's length in frames is its length in
/// advances and a test can say exactly when it finishes.
fn animated(world: &mut World, script: &str, playing: &serde_json::Value) -> EntityId {
    world.spawn(EntityData {
        name: Some("Hero".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [
            (
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "hero.decay", "script": script }),
            ),
            (
                SpriteAnimationComponent::TYPE_NAME.to_owned(),
                json!({
                    "playing": playing.clone(),
                    "speed": 1.0,
                    "clips": {
                        "walk": {
                            "frames": ["walk-0", "walk-1"],
                            "seconds_per_frame": STEP,
                            "looping": true
                        },
                        "die": {
                            "frames": ["die-0", "die-1"],
                            "seconds_per_frame": STEP,
                            "looping": false
                        }
                    }
                }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

fn sources(script: &str) -> ScriptSources {
    let mut sources = ScriptSources::new();
    sources.insert("hero.decay", script);
    sources
}

/// One fixed step: the scripts run, then the animations move — the order every
/// host uses, and the order the surface's reads are written against.
fn step(
    scripts: &mut Scripts,
    world: &mut World,
    sources: &ScriptSources,
    animations: &mut SpriteAnimations,
) -> ScriptReport {
    let input = InputState::default();
    let components = registry();
    let report = scripts.advance(
        world,
        &components,
        ScriptFrame::new(sources, &input, STEP).with_animations(animations),
    );
    animations
        .advance(world, &components, STEP)
        .expect("the clips advance");
    report
}

fn playing(world: &World, entity: EntityId) -> Option<String> {
    world
        .get(entity)?
        .components
        .get(SpriteAnimationComponent::TYPE_NAME)?
        .get("playing")?
        .as_str()
        .map(str::to_owned)
}

/// Naming a clip is a change to the world, and the advance that follows starts
/// it. This is the call the whole domain was waiting on.
#[test]
fn a_script_plays_a_clip() {
    let script = r#"
    script Hero {
        fn start() {
            Animation.play(this.entity, "walk");
        }
    }
    "#;
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!(null));
    let mut animations = SpriteAnimations::new();

    let report = step(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut animations,
    );

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(playing(&world, entity).as_deref(), Some("walk"));
    // On its second frame, not its first: a frame here lasts exactly one step,
    // and the advance that starts a clip is the same advance that moves it on.
    assert_eq!(animations.sprite(entity), Some("walk-1"));
}

/// Stopping puts the sprite back to whatever its own reference names, which is
/// what an entity with clips authored and none selected should look like.
#[test]
fn a_script_stops_a_clip() {
    let script = r"
    script Hero {
        fn start() {
            Animation.stop(this.entity);
        }
    }
    ";
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!("walk"));
    let mut animations = SpriteAnimations::new();

    let report = step(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut animations,
    );

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(playing(&world, entity), None);
    assert_eq!(animations.sprite(entity), None, "nothing is playing");
}

/// The read that makes a one-shot usable: without it a script can start a death
/// animation and never learn that it ended.
#[test]
fn a_script_is_told_when_a_clip_finished() {
    // The position is the observable: a script that learns its clip ended has
    // to be able to act on it, and moving is something the test can see.
    let script = r"
    script Hero {
        fn update(dt: f32) {
            if Animation.is_finished(this.entity) {
                this.transform.position.x = 7.0;
            }
        }
    }
    ";
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!("die"));
    let mut animations = SpriteAnimations::new();
    let mut scripts = Scripts::new();
    let sources = sources(script);

    // Two frames of a two-frame clip reach its end; the third is the one a
    // script sees it as finished on, because a read answers for the advance
    // that already happened.
    for _ in 0..2 {
        step(&mut scripts, &mut world, &sources, &mut animations);
    }
    assert!(animations.is_finished(entity), "the clip has ended");

    let report = step(&mut scripts, &mut world, &sources, &mut animations);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let moved = world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .map(|transform| transform.position[0]);
    assert_eq!(
        moved,
        Some(7.0),
        "the script saw the clip finish and acted on it"
    );
}

/// Starting a finished one-shot again, which is what `restart` is for: naming
/// the clip is not a change to the world, so nothing else would notice.
#[test]
fn a_script_restarts_a_finished_clip() {
    let script = r"
    script Hero {
        fn update(dt: f32) {
            if Animation.is_finished(this.entity) {
                Animation.restart(this.entity);
            }
        }
    }
    ";
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!("die"));
    let mut animations = SpriteAnimations::new();
    let mut scripts = Scripts::new();
    let sources = sources(script);

    for _ in 0..2 {
        step(&mut scripts, &mut world, &sources, &mut animations);
    }
    assert!(animations.is_finished(entity), "the clip reached its end");

    // The script sees it finished, replays it, and the advance in the same step
    // moves the restarted clip on by one frame.
    step(&mut scripts, &mut world, &sources, &mut animations);
    assert!(
        !animations.is_finished(entity),
        "playing it again put it back to the start"
    );
}

/// A clip a script names is one the scene authored. Naming something else is a
/// mistake worth hearing about rather than a sprite that quietly stops moving.
#[test]
fn a_clip_the_entity_does_not_have_is_reported() {
    let script = r#"
    script Hero {
        fn start() {
            Animation.play(this.entity, "swim");
        }
    }
    "#;
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!(null));
    let mut animations = SpriteAnimations::new();
    let input = InputState::default();
    let components = registry();
    let sources = sources(script);

    let report = Scripts::new().advance(
        &mut world,
        &components,
        ScriptFrame::new(&sources, &input, STEP).with_animations(&mut animations),
    );
    assert!(report.failures.is_empty(), "naming it is not the failure");
    assert_eq!(playing(&world, entity).as_deref(), Some("swim"));

    // The advance is where a clip that does not exist is found, and it says so
    // rather than leaving the sprite on an arbitrary frame.
    assert!(
        animations.advance(&world, &components, STEP).is_err(),
        "a clip the component does not hold is reported"
    );
}

/// `play` is idempotent, and it has to be.
///
/// A script says what state it is in every frame — `if moving { play("walk") }`
/// is the natural way to write it, and is how Gather's player drives its walk
/// cycle. A `play` that started the clip again would hold such a clip on its
/// first frame for ever, which is a walk animation that never walks. This is
/// the test that caught it.
#[test]
fn playing_the_clip_already_playing_does_not_restart_it() {
    let script = r#"
    script Hero {
        fn update(dt: f32) {
            Animation.play(this.entity, "walk");
        }
    }
    "#;
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!(null));
    let mut animations = SpriteAnimations::new();
    let mut scripts = Scripts::new();
    let sources = sources(script);

    // Four steps of a two-frame looping clip. Told to play on every one of
    // them, it still has to be advancing.
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..4 {
        step(&mut scripts, &mut world, &sources, &mut animations);
        if let Some(sprite) = animations.sprite(entity) {
            seen.insert(sprite.to_owned());
        }
    }
    assert_eq!(
        seen.len(),
        2,
        "a clip told to play every frame still runs, and showed {seen:?}"
    );
}

/// Saying again what the world already says is not a write.
///
/// A script names its state every frame, so most of these calls repeat the last
/// one, and a scene must not be touched for that.
#[test]
fn a_call_that_changes_nothing_leaves_the_payload_alone() {
    let script = r#"
    script Hero {
        fn update(dt: f32) {
            Animation.play(this.entity, "walk");
        }
    }
    "#;
    let mut world = World::default();
    let entity = animated(&mut world, "Hero", &json!("walk"));
    let before = world
        .get(entity)
        .and_then(|data| data.components.get(SpriteAnimationComponent::TYPE_NAME))
        .cloned();
    let mut animations = SpriteAnimations::new();

    step(
        &mut Scripts::new(),
        &mut world,
        &sources(script),
        &mut animations,
    );

    let after = world
        .get(entity)
        .and_then(|data| data.components.get(SpriteAnimationComponent::TYPE_NAME))
        .cloned();
    assert_eq!(before, after, "the payload is byte for byte what it was");
}
