//! Changing the identity a scene stores an entity under.

use crate::{
    CommandError, CommandHistory, EntityData, SceneEntityId, World, WorldCommand, WorldError,
};

use super::support::edit;

fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("a literal ID is not empty")
}

fn world_with(ids: &[&str]) -> World {
    let mut world = World::default();
    for value in ids {
        world.spawn(EntityData {
            source_id: Some(id(value)),
            ..EntityData::default()
        });
    }
    world
}

fn handle(world: &World, value: &str) -> crate::EntityId {
    world
        .entities()
        .find(|(_, data)| data.source_id.as_ref().map(SceneEntityId::as_str) == Some(value))
        .map(|(entity, _)| entity)
        .expect("the world holds this ID")
}

/// A stable ID can be set, and setting it undoes.
///
/// It is what the file keys an entity by, what a parent link names, and what
/// sibling order is derived from — and until this command existed nothing
/// could change one, so a scene made in the editor was `game-object-1`,
/// `game-object-2`, and a shipped scene's `player` and `orb-1` were
/// unreachable.
#[test]
fn a_stable_id_can_be_changed_and_changed_back() {
    let mut world = world_with(&["game-object-1"]);
    let entity = handle(&world, "game-object-1");
    let mut history = CommandHistory::default();

    history
        .apply(
            edit(
                "Rename identity",
                vec![WorldCommand::SetSourceId {
                    entity,
                    source_id: Some(id("player")),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_eq!(
        world
            .get(entity)
            .unwrap()
            .source_id
            .as_ref()
            .map(SceneEntityId::as_str),
        Some("player")
    );

    history.undo(&mut world).unwrap();
    assert_eq!(
        world
            .get(entity)
            .unwrap()
            .source_id
            .as_ref()
            .map(SceneEntityId::as_str),
        Some("game-object-1")
    );
}

/// Two entities cannot share one identity.
///
/// Not a cosmetic collision: `sindri.grid.occupant` names a grid by stable ID,
/// a parent link names one, and `to_scene` keys the file by one. The command
/// refuses before it writes, so a rejected transaction leaves the world exactly
/// as it found it.
#[test]
fn a_stable_id_already_in_use_is_refused() {
    let mut world = world_with(&["player", "floor"]);
    let floor = handle(&world, "floor");

    let refused = CommandHistory::default().apply(
        edit(
            "Rename identity",
            vec![WorldCommand::SetSourceId {
                entity: floor,
                source_id: Some(id("player")),
            }],
        ),
        &mut world,
    );

    assert!(matches!(
        refused,
        Err(CommandError::Rejected {
            source: WorldError::DuplicateSourceId(_),
            ..
        })
    ));
    assert_eq!(
        world
            .get(floor)
            .unwrap()
            .source_id
            .as_ref()
            .map(SceneEntityId::as_str),
        Some("floor"),
        "a refused command must not have written anything"
    );
}

/// Keeping an entity's own ID is not a collision with itself.
#[test]
fn an_entity_may_be_given_the_id_it_already_has() {
    let mut world = world_with(&["player"]);
    let player = handle(&world, "player");
    assert!(
        CommandHistory::default()
            .apply(
                edit(
                    "Rename identity",
                    vec![WorldCommand::SetSourceId {
                        entity: player,
                        source_id: Some(id("player")),
                    }],
                ),
                &mut world,
            )
            .is_ok()
    );
}

/// The scene's own name goes through the history like everything else.
///
/// Not because renaming a scene is dangerous, but because an editor decides
/// whether a document is unsaved by watching the history — a change made
/// outside it is one the editor does not know it has, and would let someone
/// close the window on.
#[test]
fn the_scene_name_is_an_ordinary_undoable_edit() {
    let mut world = world_with(&["player"]);
    let mut history = CommandHistory::default();
    assert_eq!(world.metadata().name, None);

    history
        .apply(
            edit(
                "Rename scene",
                vec![WorldCommand::SetSceneName {
                    name: Some("Gather".to_owned()),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_eq!(world.metadata().name.as_deref(), Some("Gather"));
    assert_eq!(
        world.to_scene().unwrap().metadata.name.as_deref(),
        Some("Gather"),
        "and it is what a save writes"
    );

    history.undo(&mut world).unwrap();
    assert_eq!(world.metadata().name, None);
}

/// A scene-level command names no entity, and says so.
#[test]
fn a_scene_command_is_about_no_entity() {
    assert_eq!(
        WorldCommand::SetSceneName { name: None }.entity(),
        None,
        "every other command is about one thing in the scene; this is about the scene"
    );
}
