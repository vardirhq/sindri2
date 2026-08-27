//! Component payloads and parentage, set and put back.

use serde_json::json;

use crate::{CommandHistory, WorldCommand};

use super::support::{edit, world_with_two_entities};

#[test]
fn components_round_trip_through_set_and_remove() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();

    history
        .apply(
            edit(
                "Add health",
                vec![WorldCommand::SetComponent {
                    entity,
                    type_name: "game.health".into(),
                    payload: json!({ "current": 3 }),
                }],
            ),
            &mut world,
        )
        .unwrap();
    history
        .apply(
            edit(
                "Remove health",
                vec![WorldCommand::RemoveComponent {
                    entity,
                    type_name: "game.health".into(),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert!(world.get(entity).unwrap().components.is_empty());

    history.undo(&mut world).unwrap();
    assert_eq!(
        world.get(entity).unwrap().components["game.health"],
        json!({ "current": 3 })
    );
    history.undo(&mut world).unwrap();
    assert!(world.get(entity).unwrap().components.is_empty());
}

#[test]
fn reparenting_is_reversible() {
    let (mut world, parent, child) = world_with_two_entities();
    let mut history = CommandHistory::default();

    history
        .apply(
            edit(
                "Reparent",
                vec![WorldCommand::SetParent {
                    entity: child,
                    parent: Some(parent),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_eq!(world.get(child).unwrap().parent, Some(parent));
    assert_eq!(world.get(parent).unwrap().children, vec![child]);

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(child).unwrap().parent, None);
    assert!(world.get(parent).unwrap().children.is_empty());
}

/// The editor-only map is saved with the document, so writing to it undoes
/// like everything else that is — and removing an entry puts back the value
/// that was there rather than a `null`.
#[test]
fn an_editor_entry_round_trips_through_set_and_remove() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();

    history
        .apply(
            edit(
                "Order it first",
                vec![WorldCommand::SetEditorEntry {
                    entity,
                    key: "order".into(),
                    value: Some(json!(0)),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_eq!(world.get(entity).unwrap().editor["order"], json!(0));

    history
        .apply(
            edit(
                "Forget the order",
                vec![WorldCommand::SetEditorEntry {
                    entity,
                    key: "order".into(),
                    value: None,
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert!(!world.get(entity).unwrap().editor.contains_key("order"));

    history.undo(&mut world).unwrap();
    assert_eq!(
        world.get(entity).unwrap().editor["order"],
        json!(0),
        "undoing a removal puts the value back, not a null"
    );
    history.undo(&mut world).unwrap();
    assert!(
        !world.get(entity).unwrap().editor.contains_key("order"),
        "and undoing the first write leaves no entry at all"
    );
}
