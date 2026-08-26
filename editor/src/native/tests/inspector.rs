//! Inspector edits, and the commands they turn into.

use std::collections::BTreeMap;

use serde_json::Value;
use sindri_core::{CommandHistory, Transform3D};

use crate::animation;

use super::super::editing::find_by_source_id;
use super::super::inspector_panel::draft::{
    EntityDraft, ProjectDefaults, addable_components, component_commands, component_default,
    draft_commands,
};
use super::super::*;
use super::support::*;

#[test]
fn an_untouched_draft_produces_no_commands() {
    let world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let draft = EntityDraft::from(world.get(entity).unwrap());
    assert!(draft_commands(entity, &draft.clone(), &draft).is_empty());
}

#[test]
fn inspector_edits_reach_the_world_and_undo_cleanly() {
    let mut world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let original = EntityDraft::from(world.get(entity).unwrap());

    let mut draft = original.clone();
    draft.name = "Renamed Cube".to_owned();
    draft.transform_3d = Some(Transform3D {
        position: [1.0, 2.0, 3.0],
        ..draft.transform_3d.unwrap_or_default()
    });

    let buffer = draft_commands(entity, &original, &draft);
    assert_eq!(buffer.len(), 2);

    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Edit entity"), &mut world)
        .unwrap();
    let edited = world.get(entity).unwrap();
    assert_eq!(edited.name.as_deref(), Some("Renamed Cube"));
    assert_eq!(edited.transform_3d, draft.transform_3d);

    history.undo(&mut world).unwrap();
    assert_eq!(EntityDraft::from(world.get(entity).unwrap()), original);
}

/// The whole point: a component edit reaches the world through the command
/// layer, and undo puts it back. Until this existed, a component was a
/// read-only label and every value was set by editing the scene file.
#[test]
fn a_component_edit_reaches_the_world_and_undoes_cleanly() {
    let mut world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let original = world.get(entity).unwrap().components.clone();

    let mut draft = original.clone();
    draft
        .get_mut("sindri.mesh")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("layer".to_owned(), serde_json::json!(4));

    let (buffer, refused) = component_commands(entity, &original, &draft, extractor().components());
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(buffer.len(), 1);

    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Edit components"), &mut world)
        .unwrap();
    assert_eq!(
        world.get(entity).unwrap().components["sindri.mesh"]["layer"],
        serde_json::json!(4)
    );

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(entity).unwrap().components, original);
}

/// An edit that would stop a component decoding never becomes a command.
/// The payload is written back exactly as stored, so letting it through
/// would produce a scene the engine refuses to open — discovered at the
/// next launch rather than at the field being edited.
#[test]
fn an_edit_that_breaks_a_schema_is_refused_rather_than_written() {
    let world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let original = world.get(entity).unwrap().components.clone();

    let mut draft = original.clone();
    draft
        .get_mut("sindri.mesh")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("primitive".to_owned(), serde_json::json!("dodecahedron"));

    let (buffer, refused) = component_commands(entity, &original, &draft, extractor().components());
    assert!(buffer.is_empty(), "nothing is written");
    assert_eq!(refused.len(), 1, "and the author is told why");
    assert!(refused[0].contains("sindri.mesh"), "{refused:?}");
}

/// A component nothing understands is still editable, which is what the
/// preserve policy promises and could not previously deliver.
#[test]
fn an_unknown_component_can_still_be_edited() {
    let world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let original: BTreeMap<String, Value> =
        [("game.health".to_owned(), serde_json::json!({ "hp": 3 }))]
            .into_iter()
            .collect();
    let mut draft = original.clone();
    draft.get_mut("game.health").unwrap()["hp"] = serde_json::json!(5);

    let (buffer, refused) = component_commands(entity, &original, &draft, extractor().components());
    assert!(
        refused.is_empty(),
        "nothing is known about its shape, so nothing is claimed"
    );
    assert_eq!(buffer.len(), 1);
}

/// Add Component offers what the entity lacks and the registry can create,
/// and nothing else.
#[test]
fn add_component_offers_only_what_it_can_actually_add() {
    let extractor = extractor();
    let present: BTreeMap<String, Value> = [("sindri.mesh".to_owned(), serde_json::json!({}))]
        .into_iter()
        .collect();
    let offered: Vec<String> = addable_components(
        extractor.components(),
        &present,
        ProjectDefaults {
            font: Some("fonts/Inter.ttf"),
            ..ProjectDefaults::default()
        },
    )
    .into_iter()
    .map(|metadata| metadata.type_name)
    .collect();

    assert!(
        !offered.contains(&"sindri.mesh".to_owned()),
        "not one it already has"
    );
    assert!(offered.contains(&"sindri.sprite".to_owned()));
    assert!(
        !offered.contains(&animation::TYPE_NAME.to_owned()),
        "and not one with no sensible blank, which the engine would refuse"
    );
    assert!(
        !offered.contains(&UI_TEXT_COMPONENT.to_owned()),
        "and not one from the other space: a mesh is in the world, and the UI \
         is not somewhere a world entity can also be"
    );
}

/// The two families are offered to the entities they belong on, and an entity
/// carrying nothing yet is offered both because it has not chosen.
#[test]
fn the_menu_offers_one_space_or_the_other() {
    let extractor = extractor();
    let offered = |present: &BTreeMap<String, Value>| -> Vec<String> {
        addable_components(
            extractor.components(),
            present,
            ProjectDefaults {
                font: Some("fonts/Inter.ttf"),
                ..ProjectDefaults::default()
            },
        )
        .into_iter()
        .map(|metadata| metadata.type_name)
        .collect()
    };

    let empty = offered(&BTreeMap::new());
    assert!(empty.contains(&"sindri.sprite".to_owned()));
    assert!(empty.contains(&UI_TEXT_COMPONENT.to_owned()));

    let hud: BTreeMap<String, Value> = [(UI_IMAGE_COMPONENT.to_owned(), serde_json::json!({}))]
        .into_iter()
        .collect();
    let hud = offered(&hud);
    assert!(hud.contains(&UI_TEXT_COMPONENT.to_owned()));
    assert!(
        !hud.contains(&"sindri.sprite".to_owned()),
        "a HUD element is not also a thing in the world"
    );
}

/// Every default the registry offers has to produce a component the engine
/// accepts, or Add Component is a button that breaks a scene.
#[test]
fn every_offered_default_is_one_the_engine_accepts() {
    let extractor = extractor();
    let components = extractor.components();
    let project = ProjectDefaults {
        font: Some("fonts/Inter.ttf"),
        sprite: Some("idle"),
        grid: Some("floor"),
        audio: Some("audio/pickup.wav"),
        script: Some(("scripts/spin.decay", "Spin")),
    };
    for metadata in addable_components(components, &BTreeMap::new(), project) {
        let payload = component_default(components, &metadata.type_name, project)
            .expect("it was offered, so it has one");
        components
            .validate_payload(&metadata.type_name, &payload)
            .unwrap_or_else(|error| {
                panic!(
                    "the default for {} does not decode: {error}",
                    metadata.type_name
                )
            });
    }
}

#[test]
fn text_is_addable_only_when_the_project_has_a_font() {
    let extractor = extractor();
    let components = extractor.components();
    let present = BTreeMap::new();

    assert!(
        !addable_components(components, &present, ProjectDefaults::default())
            .iter()
            .any(|metadata| metadata.type_name == UI_TEXT_COMPONENT)
    );

    let payload = component_default(
        components,
        UI_TEXT_COMPONENT,
        ProjectDefaults {
            font: Some("fonts/Inter.ttf"),
            ..ProjectDefaults::default()
        },
    )
    .expect("a project font completes a valid text component");
    assert_eq!(payload["font"], "fonts/Inter.ttf");
    assert_eq!(payload["text"], "Text");
    // The whole component, not the two fields the editor used to write: a text
    // added here and one authored by hand are now the same component.
    assert_eq!(payload["font_size"], 24.0);
    assert_eq!(payload["anchor"], "center");
    assert_eq!(payload["layer"], 0);
    components
        .validate_payload(UI_TEXT_COMPONENT, &payload)
        .unwrap();
}

#[test]
fn sprite_animation_is_addable_only_with_a_named_sheet_sprite() {
    let extractor = extractor();
    let components = extractor.components();
    let present = BTreeMap::new();

    assert!(
        !addable_components(components, &present, ProjectDefaults::default())
            .iter()
            .any(|metadata| metadata.type_name == animation::TYPE_NAME)
    );

    let payload = component_default(
        components,
        animation::TYPE_NAME,
        ProjectDefaults {
            sprite: Some("idle"),
            ..ProjectDefaults::default()
        },
    )
    .expect("a named sheet sprite completes a valid animation component");
    assert_eq!(
        payload["clips"]["clip"]["frames"],
        serde_json::json!(["idle"])
    );
    assert_eq!(payload["playing"], "clip");
    components
        .validate_payload(animation::TYPE_NAME, &payload)
        .unwrap();
}

#[test]
fn a_drag_run_collapses_into_one_undo_step() {
    let mut world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let original = EntityDraft::from(world.get(entity).unwrap());
    let mut history = CommandHistory::default();

    for step in [1.0_f32, 2.0, 3.0] {
        let mut draft = original.clone();
        draft.transform_3d = Some(Transform3D {
            position: [step, 0.0, 0.0],
            ..original.transform_3d.unwrap_or_default()
        });
        history
            .apply(
                draft_commands(entity, &original, &draft)
                    .into_transaction("Edit entity")
                    .merging(format!("inspector:{}", entity.index())),
                &mut world,
            )
            .unwrap();
    }

    history.undo(&mut world).unwrap();
    assert_eq!(EntityDraft::from(world.get(entity).unwrap()), original);
    assert!(!history.can_undo());
}

/// The two components Gather is mostly made of, and neither could be added.
///
/// Both were registered without a default payload, so `component_default`
/// answered `None` and Add Component filtered them out — which meant the
/// editor could not put a script on an entity at all, in an engine whose
/// headline capability is scripting. Neither has an honest blank, because
/// neither the source nor the clip is something the engine can invent; what it
/// has is a project sitting beside the scene.
#[test]
fn a_script_and_a_clip_are_addable_once_the_project_holds_one() {
    let extractor = extractor();
    let components = extractor.components();
    let present = BTreeMap::new();

    let bare: Vec<String> = addable_components(components, &present, ProjectDefaults::default())
        .into_iter()
        .map(|metadata| metadata.type_name)
        .collect();
    assert!(
        !bare.contains(&SCRIPT_COMPONENT.to_owned()),
        "a project with no script source has nothing to point one at"
    );
    assert!(!bare.contains(&AUDIO_SOURCE_COMPONENT.to_owned()));

    let project = ProjectDefaults {
        audio: Some("audio/pickup.wav"),
        script: Some(("scripts/spin.decay", "Spin")),
        ..ProjectDefaults::default()
    };
    let offered: Vec<String> = addable_components(components, &present, project)
        .into_iter()
        .map(|metadata| metadata.type_name)
        .collect();
    assert!(offered.contains(&SCRIPT_COMPONENT.to_owned()));
    assert!(offered.contains(&AUDIO_SOURCE_COMPONENT.to_owned()));

    let script = component_default(components, SCRIPT_COMPONENT, project).unwrap();
    assert_eq!(script["source"], "scripts/spin.decay");
    assert_eq!(
        script["script"], "Spin",
        "and a container, because a source alone runs nothing"
    );
    assert_eq!(script["enabled"], true);
    components
        .validate_payload(SCRIPT_COMPONENT, &script)
        .unwrap();

    let clip = component_default(components, AUDIO_SOURCE_COMPONENT, project).unwrap();
    assert_eq!(clip["clip"], "audio/pickup.wav");
    assert_eq!(clip["volume"], 1.0);
    components
        .validate_payload(AUDIO_SOURCE_COMPONENT, &clip)
        .unwrap();
}

/// A component added by a button and the same one authored by hand have to be
/// the same component.
///
/// The audit's finding, as a test: the editor's `sindri.ui.text` wrote two
/// fields, so its panel drew two rows where Gather's title drew seven, and a
/// tilemap made here had no `projection` and could never be isometric.
#[test]
fn an_added_component_has_every_field_the_authored_one_has() {
    let extractor = extractor();
    let components = extractor.components();
    let project = ProjectDefaults {
        font: Some("fonts/Inter.ttf"),
        ..ProjectDefaults::default()
    };

    let text = component_default(components, UI_TEXT_COMPONENT, project).unwrap();
    let text = text.as_object().expect("an object");
    for field in [
        "text",
        "font",
        "font_size",
        "line_height",
        "color",
        "anchor",
        "layer",
    ] {
        assert!(text.contains_key(field), "UI Text is missing {field}");
    }

    let map = component_default(components, crate::tilemap::TYPE_NAME, project).unwrap();
    let map = map.as_object().expect("an object");
    for field in ["projection", "tile_size", "tint", "layer"] {
        assert!(
            map.contains_key(field),
            "Tilemap is missing {field}; without projection it can never be isometric"
        );
    }
}
