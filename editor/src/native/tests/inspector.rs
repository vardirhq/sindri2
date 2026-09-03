//! Editing a component that is already there, and the commands it turns into.

use std::collections::BTreeMap;

use serde_json::Value;
use sindri_core::{CommandHistory, Transform3D};

use super::super::editing::find_by_source_id;
use super::super::inspector_panel::draft::{
    EntityDraft, IdentityRefusal, component_commands, draft_commands, identity_commands,
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

/// Renaming a grid takes every occupant that names it with it.
///
/// A stable ID is a reference, not a label: `sindri.grid.occupant` names the
/// grid it stands on by one. Renaming a grid without rewriting its occupants
/// would leave every piece pointing at an entity that no longer exists — the
/// scene would still open, and nothing would be on the board.
#[test]
fn renaming_a_grid_repoints_the_pieces_standing_on_it() {
    use sindri_core::{CommandHistory, EntityData, SceneEntityId, World};

    let mut world = World::default();
    let grid = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("game-object-1").unwrap()),
        ..EntityData::default()
    });
    let piece = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("orb-1").unwrap()),
        components: BTreeMap::from([(
            GRID_OCCUPANT_COMPONENT.to_owned(),
            serde_json::json!({ "grid": "game-object-1", "footprint": [[0, 0]] }),
        )]),
        ..EntityData::default()
    });

    let buffer = identity_commands(&world, grid, "floor").expect("a free ID");
    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Change stable ID"), &mut world)
        .unwrap();

    assert_eq!(
        world
            .get(grid)
            .unwrap()
            .source_id
            .as_ref()
            .unwrap()
            .as_str(),
        "floor"
    );
    assert_eq!(
        world.get(piece).unwrap().components[GRID_OCCUPANT_COMPONENT]["grid"],
        "floor",
        "the piece has to follow the board it stands on"
    );
    // Fields the editor has never heard of ride along, because the stored
    // payload is what gets written back rather than a decoded component.
    assert_eq!(
        world.get(piece).unwrap().components[GRID_OCCUPANT_COMPONENT]["footprint"],
        serde_json::json!([[0, 0]])
    );

    history.undo(&mut world).unwrap();
    assert_eq!(
        world.get(piece).unwrap().components[GRID_OCCUPANT_COMPONENT]["grid"],
        "game-object-1",
        "and one undo puts both back"
    );
}

/// An ID the scene cannot use produces no commands, and says which fault it is.
///
/// Refused here rather than by the command layer because the inspector's draft
/// is committed every frame: a command refused once would be refused again on
/// the next frame, and the console would fill with the same line.
#[test]
fn an_unusable_stable_id_is_refused_before_it_is_written() {
    use sindri_core::{EntityData, SceneEntityId, World};

    let mut world = World::default();
    let first = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("player").unwrap()),
        ..EntityData::default()
    });
    let second = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("floor").unwrap()),
        ..EntityData::default()
    });

    assert_eq!(
        identity_commands(&world, second, "player").err(),
        Some(IdentityRefusal::Taken)
    );
    assert_eq!(
        identity_commands(&world, second, "   ").err(),
        Some(IdentityRefusal::Empty)
    );
    assert!(
        identity_commands(&world, first, "player")
            .expect("its own ID is not a collision")
            .is_empty(),
        "and asking for the ID it already has is nothing to do"
    );
}
