//! `World.find` prefers an entity that is taking part in the scene.
//!
//! A world can hold more than one scene, so a game with two places can have a
//! `Player` in each, and only one of them is anywhere the player is. A lookup
//! that reached the switched-off one would drive a player nobody can see.
//!
//! It prefers rather than filters, which is the whole subtlety. A switched-off
//! screen gets looked up by name all over Orbital's title and pause flow
//! precisely so that it can be switched back on — so a lookup that skipped what
//! is out of play would break every one of those.

use serde_json::json;
use sindri_core::{ComponentSchemaRegistry, EntityData, SceneComponent, Transform3D, World};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// Spawns two entities sharing `name`, the first of them switched off, then
/// reports which one a script found by moving it.
fn found_of_two(name: &str, first_disabled: bool, second_disabled: bool) -> Option<bool> {
    let mut world = World::default();
    let first = world.spawn(EntityData {
        name: Some(name.to_owned()),
        transform_3d: Some(Transform3D::default()),
        disabled: first_disabled,
        ..EntityData::default()
    });
    let second = world.spawn(EntityData {
        name: Some(name.to_owned()),
        transform_3d: Some(Transform3D::default()),
        disabled: second_disabled,
        ..EntityData::default()
    });
    world.spawn(EntityData {
        name: Some("Seeker".to_owned()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({
                "source": "seeker.decay",
                "script": "Seeker",
                "properties": { "wanted": name },
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "seeker.decay",
        r#"
        script Seeker {
            @export let wanted: String = "";
            fn update(dt: f32) {
                let it = World.find(this.wanted);
                if it != null { it.transform.position.x = 5.0; }
            }
        }
        "#,
    );
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    assert!(report.failures.is_empty(), "{report:#?}");
    let moved = |entity| {
        world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .is_some_and(|transform| transform.position[0] > 1.0)
    };
    match (moved(first), moved(second)) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

#[test]
fn the_one_taking_part_in_the_scene_wins() {
    assert_eq!(
        found_of_two("Player", true, false),
        Some(false),
        "the switched-on second entity should have been found"
    );
    assert_eq!(
        found_of_two("Player", false, true),
        Some(true),
        "and the switched-on first one, when it is the first"
    );
}

/// The pattern Orbital's whole title and pause flow is built on: a screen is
/// switched off and then looked up by name so it can be switched back on.
#[test]
fn something_switched_off_is_still_found_when_it_is_all_there_is() {
    let mut world = World::default();
    let hud = world.spawn(EntityData {
        name: Some("Hud".to_owned()),
        transform_3d: Some(Transform3D::default()),
        disabled: true,
        ..EntityData::default()
    });
    world.spawn(EntityData {
        name: Some("Director".to_owned()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "director.decay", "script": "Director" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "director.decay",
        r#"
        script Director {
            fn update(dt: f32) {
                let hud = World.find("Hud");
                if hud != null { World.set_active(hud, true); }
            }
        }
        "#,
    );
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    assert!(report.failures.is_empty(), "{report:#?}");
    assert!(
        world.is_active(hud),
        "a switched-off screen has to stay findable, or nothing can switch it on"
    );
}

/// A parent's switch governs its children, so a scene's whole contents are out
/// of play together — which is what makes one switch enough to leave a place.
#[test]
fn something_under_a_switched_off_parent_loses_to_something_in_play() {
    let mut world = World::default();
    let root = world.spawn(EntityData {
        name: Some("Farm".to_owned()),
        disabled: true,
        ..EntityData::default()
    });
    let indoors = world.spawn(EntityData {
        name: Some("Player".to_owned()),
        transform_3d: Some(Transform3D::default()),
        ..EntityData::default()
    });
    let outdoors = world.spawn(EntityData {
        name: Some("Player".to_owned()),
        transform_3d: Some(Transform3D::default()),
        ..EntityData::default()
    });
    world
        .set_parent(outdoors, Some(root))
        .expect("the outdoor player belongs to the farm");
    world.spawn(EntityData {
        name: Some("Seeker".to_owned()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "seeker.decay", "script": "Seeker" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "seeker.decay",
        r#"
        script Seeker {
            fn update(dt: f32) {
                let it = World.find("Player");
                if it != null { it.transform.position.x = 5.0; }
            }
        }
        "#,
    );
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    assert!(report.failures.is_empty(), "{report:#?}");
    let moved = |entity| {
        world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .is_some_and(|transform| transform.position[0] > 1.0)
    };
    assert!(moved(indoors), "the player in play should have been found");
    assert!(
        !moved(outdoors),
        "the one under the switched-off farm should not have been"
    );
}
