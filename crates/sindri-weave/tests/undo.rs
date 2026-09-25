//! Styling a live world in place draws what styling a copy draws, and
//! leaves nothing behind once undone.

use sindri_core::{EntityData, SceneDocument, World};
use sindri_weave::{PresentationWorld, Presenter, UiStates};
use weave::{Viewport, parse};

const SCENE: &str = r#"{
    "format_version": 10,
    "metadata": { "name": "undo" },
    "entities": [
        { "id": "panel", "name": "panel",
          "components": {
              "sindri.ui.shape": { "kind": "rect", "anchor": "center" },
              "sindri.ui.layout": { "direction": "column" } } },
        { "id": "label", "name": "label", "parent": "panel",
          "transform_3d": { "scale": [0.5, 0.1, 1.0] },
          "components": {
              "sindri.ui.text": { "text": "Hi", "font": "f.ttf", "font_size": 0.05 } } },
        { "id": "level", "name": "level",
          "components": { "game.tiles": { "cells": [1, 2, 3, 4] } } }
    ]
}"#;

const STYLE: &str = r"
    #panel { width: 400px; height: 300px; padding: 20px; background: #102030;
             box-shadow: 0 4px 12px #000; flex-wrap: wrap; }
    text { color: #ffffff; margin: 4px; flex: 1; }
    .late { width: 222px; height: 111px; background: #a04020; }
    * { x: 0; }
";

fn entities(world: &World) -> Vec<sindri_core::EntityData> {
    world.entities().map(|(_, data)| data.clone()).collect()
}

fn load() -> World {
    let document = SceneDocument::from_json(SCENE).expect("scene parses");
    World::from_scene(&document).expect("scene loads").world
}

fn entity(world: &World, name: &str) -> sindri_core::EntityId {
    world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some(name))
        .map(|(entity, _)| entity)
        .expect("the entity is there")
}

fn spawn_late(world: &mut World) -> sindri_core::EntityId {
    let mut data = EntityData {
        name: Some("late".to_owned()),
        ..EntityData::default()
    };
    data.components.insert(
        "weave.style".to_owned(),
        serde_json::json!({ "classes": ["late"] }),
    );
    data.components.insert(
        "sindri.ui.shape".to_owned(),
        serde_json::json!({ "kind": "rect", "anchor": "center" }),
    );
    world.spawn(data)
}

const VIEWPORT: Viewport = Viewport {
    width: 1_200.0,
    height: 800.0,
};

#[test]
fn settling_styles_the_world_as_a_copy_would_be_styled() {
    let authored = load();
    let sheet = parse(STYLE).expect("Weave parses");
    let copied = PresentationWorld::resolve(&authored, &sheet, VIEWPORT).expect("styles");
    let mut live = authored.clone();
    Presenter::new()
        .settle(&mut live, &[sheet], VIEWPORT)
        .expect("settles");
    assert_eq!(entities(&live), entities(copied.world()));
}

#[test]
fn a_hover_is_laid_over_the_settled_world_and_taken_off_again() {
    let authored = load();
    let sheet = parse(&format!(
        "{STYLE} #panel:hover {{ background: #ff0000; width: 500px; }}"
    ))
    .expect("Weave parses");
    let mut live = authored.clone();
    let mut presenter = Presenter::new();
    presenter
        .settle(&mut live, std::slice::from_ref(&sheet), VIEWPORT)
        .expect("settles");
    let settled = entities(&live);

    let mut states = UiStates::new();
    states.insert(entity(&live, "panel"), weave::States::HOVER);
    let copied = PresentationWorld::resolve_with_states(&authored, &sheet, VIEWPORT, &states)
        .expect("styles");
    let undo = presenter
        .present_over(&mut live, std::slice::from_ref(&sheet), VIEWPORT, &states)
        .expect("lays the hover over");
    assert_eq!(entities(&live), entities(copied.world()));

    undo.undo(&mut live);
    assert_eq!(entities(&live), settled);
}

/// A script that writes a styled value keeps it, as an inline style beats a
/// stylesheet: the slider fill that sizes itself to the slider's value.
#[test]
fn a_value_a_script_wrote_is_not_styled_back() {
    let mut live = load();
    let sheet = parse(STYLE).expect("Weave parses");
    let mut presenter = Presenter::new();
    presenter
        .settle(&mut live, std::slice::from_ref(&sheet), VIEWPORT)
        .expect("settles");
    let label = entity(&live, "label");
    live.get_mut(label)
        .and_then(|data| data.transform_3d.as_mut())
        .expect("a transform")
        .scale[1] = 0.01;

    let mut states = UiStates::new();
    states.insert(entity(&live, "panel"), weave::States::HOVER);
    let undo = presenter
        .present_over(&mut live, std::slice::from_ref(&sheet), VIEWPORT, &states)
        .expect("presents");
    let drawn = live
        .get(label)
        .and_then(|data| data.transform_3d)
        .expect("a transform");
    assert!((drawn.scale[1] - 0.01).abs() < 1.0e-6, "{drawn:?}");
    undo.undo(&mut live);
}

#[test]
fn a_spawned_element_is_settled_before_an_idle_draw() {
    let mut live = load();
    let sheet = parse(STYLE).expect("Weave parses");
    let mut presenter = Presenter::new();
    presenter
        .settle(&mut live, std::slice::from_ref(&sheet), VIEWPORT)
        .expect("settles");

    let late = spawn_late(&mut live);
    let expected = PresentationWorld::resolve(&live, &sheet, VIEWPORT).expect("styles the spawn");
    let expected_late = expected.world().get(late).expect("late entity").clone();

    let undo = presenter
        .present_over(
            &mut live,
            std::slice::from_ref(&sheet),
            VIEWPORT,
            &UiStates::new(),
        )
        .expect("settles the spawn without hover");
    assert_eq!(live.get(late), Some(&expected_late));

    undo.undo(&mut live);
    assert_eq!(live.get(late), Some(&expected_late));
}

#[test]
fn settling_a_spawn_does_not_overwrite_an_existing_script_value() {
    let mut live = load();
    let sheet = parse(STYLE).expect("Weave parses");
    let mut presenter = Presenter::new();
    presenter
        .settle(&mut live, std::slice::from_ref(&sheet), VIEWPORT)
        .expect("settles");
    let label = entity(&live, "label");
    live.get_mut(label)
        .and_then(|data| data.transform_3d.as_mut())
        .expect("a transform")
        .scale[1] = 0.01;
    spawn_late(&mut live);

    let undo = presenter
        .present_over(&mut live, &[sheet], VIEWPORT, &UiStates::new())
        .expect("settles the spawn");
    let existing = live
        .get(label)
        .and_then(|data| data.transform_3d)
        .expect("a transform");
    assert!((existing.scale[1] - 0.01).abs() < 1.0e-6, "{existing:?}");
    undo.undo(&mut live);
}

#[test]
fn nothing_hovered_and_nothing_easing_costs_nothing() {
    let mut live = load();
    let sheet = parse(STYLE).expect("Weave parses");
    let mut presenter = Presenter::new();
    presenter
        .settle(&mut live, std::slice::from_ref(&sheet), VIEWPORT)
        .expect("settles");
    let settled = entities(&live);
    let undo = presenter
        .present_over(&mut live, &[sheet], VIEWPORT, &UiStates::new())
        .expect("presents");
    assert_eq!(entities(&live), settled);
    undo.undo(&mut live);
}

#[test]
fn a_failed_styling_leaves_the_world_as_it_was() {
    let authored = load();
    let good = parse(STYLE).expect("parses");
    let bad = parse("#panel:hover { flex-grow: -1; }").expect("parses");
    let mut live = authored.clone();
    let mut presenter = Presenter::new();
    presenter
        .settle(&mut live, &[good.clone(), bad.clone()], VIEWPORT)
        .expect("settles without the hover");
    let settled = entities(&live);
    let mut states = UiStates::new();
    states.insert(entity(&live, "panel"), weave::States::HOVER);
    let error = presenter.present_over(&mut live, &[good, bad], VIEWPORT, &states);
    assert!(error.is_err());
    assert_eq!(entities(&live), settled);
}
