use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;
use sindri_core::{ApplyMode, ComponentSchemaRegistry, EntityId, SceneComponent};

use super::{Asked, Components, HeldEdits};

#[derive(Deserialize)]
#[allow(dead_code)]
struct Terrain {
    seed: u32,
    label: String,
    tint: [f32; 4],
    layers: Vec<u32>,
}

impl SceneComponent for Terrain {
    const TYPE_NAME: &'static str = "test.terrain";
}

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register_with_fields::<Terrain>(
            "Terrain",
            json!({ "seed": 0, "label": "", "tint": [1, 1, 1, 1], "layers": [] }),
        )
        .expect("registers");
    registry
        .apply_when::<Terrain>(ApplyMode::Manual, ["seed", "layers"])
        .expect("manual");
    registry
        .apply_when::<Terrain>(ApplyMode::Settled, ["label"])
        .expect("settled");
    registry
}

fn entity() -> EntityId {
    let mut world = sindri_core::World::default();
    world.spawn(sindri_core::EntityData::default())
}

fn stored() -> Components {
    Components::from([(
        "test.terrain".to_owned(),
        json!({ "seed": 1, "label": "a", "tint": [1, 1, 1, 1], "layers": [1, 2] }),
    )])
}

fn edit(components: &Components, change: impl FnOnce(&mut serde_json::Value)) -> Components {
    let mut edited = components.clone();
    change(edited.get_mut("test.terrain").expect("there"));
    edited
}

#[test]
fn instant_edits_go_and_manual_ones_wait_for_apply() {
    let registry = registry();
    let entity = entity();
    let mut held = HeldEdits::default();
    let now = Instant::now();
    let stored = stored();
    let shown = held.shown(entity, &stored);
    let edited = edit(&shown, |terrain| {
        terrain["seed"] = json!(10);
        terrain["tint"][0] = json!(0.5);
    });
    let target = held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &edited,
        true,
        Asked::default(),
        now,
    );
    // The tint went; the seed is held, and still shown as typed.
    assert_eq!(target["test.terrain"]["tint"][0], json!(0.5));
    assert_eq!(target["test.terrain"]["seed"], json!(1));
    assert_eq!(
        held.shown(entity, &target)["test.terrain"]["seed"],
        json!(10)
    );
    assert!(held.waiting(entity).contains("test.terrain"));

    // Something else changing the component meanwhile is kept.
    let stored = edit(&target, |terrain| terrain["label"] = json!("undone"));
    let shown = held.shown(entity, &stored);
    let applied = held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &shown,
        false,
        Asked {
            apply: Some("test.terrain"),
            revert: None,
        },
        now,
    );
    assert_eq!(applied["test.terrain"]["seed"], json!(10));
    assert_eq!(applied["test.terrain"]["label"], json!("undone"));
    assert!(held.waiting(entity).is_empty());
}

#[test]
fn settled_edits_wait_while_typing_and_go_when_the_person_stops() {
    let registry = registry();
    let entity = entity();
    let mut held = HeldEdits::default();
    let start = Instant::now();
    let stored = stored();
    let shown = held.shown(entity, &stored);
    let edited = edit(&shown, |terrain| terrain["label"] = json!("ab"));
    let settled = held.settled(start, false, true);
    let target = held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &edited,
        settled,
        Asked::default(),
        start,
    );
    assert_eq!(target["test.terrain"]["label"], json!("a"), "still typing");
    assert!(held.settling());
    assert!(!held.settled(start + Duration::from_millis(100), false, true));
    assert!(
        !held.settled(start + Duration::from_secs(1), true, false),
        "a drag"
    );
    assert!(held.settled(start, false, false), "left the field");
    let later = start + Duration::from_secs(1);
    assert!(held.settled(later, false, true), "paused");
    let shown = held.shown(entity, &target);
    let target = held.sort(
        &registry,
        entity,
        &target,
        &shown,
        &shown,
        true,
        Asked::default(),
        later,
    );
    assert_eq!(target["test.terrain"]["label"], json!("ab"));
    assert!(!held.settling());
}

#[test]
fn revert_forgets_and_typing_back_to_the_stored_value_holds_nothing() {
    let registry = registry();
    let entity = entity();
    let mut held = HeldEdits::default();
    let now = Instant::now();
    let stored = stored();
    let shown = held.shown(entity, &stored);
    let edited = edit(&shown, |terrain| terrain["layers"] = json!([1, 2, 3]));
    held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &edited,
        false,
        Asked::default(),
        now,
    );
    assert!(held.waiting(entity).contains("test.terrain"));
    let shown = held.shown(entity, &stored);
    held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &shown,
        false,
        Asked {
            apply: None,
            revert: Some("test.terrain"),
        },
        now,
    );
    assert_eq!(held.shown(entity, &stored), stored);

    let shown = held.shown(entity, &stored);
    let edited = edit(&shown, |terrain| terrain["seed"] = json!(5));
    held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &edited,
        false,
        Asked::default(),
        now,
    );
    let shown = held.shown(entity, &stored);
    let back = edit(&shown, |terrain| terrain["seed"] = json!(1));
    held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &back,
        false,
        Asked::default(),
        now,
    );
    assert!(held.waiting(entity).is_empty());
}

#[test]
fn moving_on_settles_what_was_left_and_keeps_what_waits_for_apply() {
    let registry = registry();
    let entity = entity();
    let mut held = HeldEdits::default();
    let now = Instant::now();
    let stored = stored();
    let shown = held.shown(entity, &stored);
    let edited = edit(&shown, |terrain| {
        terrain["label"] = json!("abc");
        terrain["seed"] = json!(9);
    });
    held.sort(
        &registry,
        entity,
        &stored,
        &shown,
        &edited,
        false,
        Asked::default(),
        now,
    );
    let settled = held.settle_others(None, |_| Some(stored.clone()));
    assert_eq!(settled.len(), 1);
    let (settled_entity, before, after) = &settled[0];
    assert_eq!(*settled_entity, entity);
    assert_eq!(before, &stored);
    assert_eq!(after["test.terrain"]["label"], json!("abc"));
    assert_eq!(
        after["test.terrain"]["seed"],
        json!(1),
        "manual edits stay held"
    );
    assert!(held.waiting(entity).contains("test.terrain"));
}
