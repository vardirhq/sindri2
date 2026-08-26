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
