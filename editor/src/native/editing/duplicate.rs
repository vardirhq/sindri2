//! Copying an entity and everything under it, as one undoable step.
//!
//! Its own file because the interesting part is not the copying but the
//! handles. `WorldCommand::Spawn` names the handle it spawns at, so a
//! transaction that copies a subtree has to know, before it runs, every handle
//! the world is about to hand out — and `World::next_handle` is a peek rather
//! than an allocation, so asking it four times answers the same thing four
//! times.
//!
//! So the copy is rehearsed. A clone of the world receives exactly the spawns
//! the real one is about to, in the same order, and hands out the handles it
//! will hand out. Cloning a world is what pressing Play already costs, and a
//! duplicate is rarer than a frame.

use sindri_core::{CommandBuffer, EntityData, EntityId, SceneEntityId, World, WorldCommand};

/// The commands that copy `entity` and its descendants beside it.
///
/// Empty when there is nothing at that handle, which is the only failure it can
/// have: everything else is a spawn the world is about to accept.
pub(crate) fn duplicate_commands(
    world: &World,
    entity: EntityId,
) -> (CommandBuffer, Option<EntityId>) {
    let mut buffer = CommandBuffer::new();
    if world.get(entity).is_none() {
        return (buffer, None);
    }
    // The rehearsal. It is a real world, so it allocates handles and links
    // parents exactly as the command layer will.
    let mut rehearsal = world.clone();
    let parent = world.get(entity).and_then(|data| data.parent);
    let root = copy_into(&mut rehearsal, world, entity, parent, &mut buffer);
    (buffer, Some(root))
}

/// Copies one entity and then everything under it, depth first.
///
/// Parents first, so a child's copy can name the handle its parent's copy was
/// given rather than the one the original had.
fn copy_into(
    rehearsal: &mut World,
    world: &World,
    entity: EntityId,
    parent: Option<EntityId>,
    buffer: &mut CommandBuffer,
) -> EntityId {
    let source = world.get(entity).expect("the caller checked this handle");
    let data = EntityData {
        source_id: Some(unused_id(rehearsal, source.source_id.as_ref())),
        name: source.name.as_ref().map(|name| format!("{name} copy")),
        parent,
        // The copy's own children are spawned by the recursion below; taking
        // the original's list would name entities that are not under it.
        children: Vec::new(),
        transform_3d: source.transform_3d,
        components: source.components.clone(),
        editor: source.editor.clone(),
    };
    let handle = rehearsal.spawn(data.clone());
    // The rehearsal spawns, so the real command has a handle to name. Its own
    // child list is rebuilt by `relink_child` when the command runs.
    rehearsal
        .set_parent(handle, parent)
        .expect("a fresh entity accepts the parent its original had");
    buffer.push(WorldCommand::Spawn {
        entity: handle,
        data: Box::new(data),
    });
    for child in &source.children {
        copy_into(rehearsal, world, *child, Some(handle), buffer);
    }
    handle
}

/// A stable ID like the original's that nothing is using yet.
///
/// Derived from the original rather than generated fresh, because `player-copy`
/// says what it is and `game-object-7` does not — and a stable ID is what a
/// `sindri.grid.occupant` names and what sibling order is sorted by.
fn unused_id(world: &World, original: Option<&SceneEntityId>) -> SceneEntityId {
    let stem = original.map_or_else(|| "game-object".to_owned(), |id| id.as_str().to_owned());
    let mut candidate = format!("{stem}-copy");
    let mut suffix = 2_u32;
    while taken(world, &candidate) {
        candidate = format!("{stem}-copy-{suffix}");
        suffix += 1;
    }
    SceneEntityId::new(candidate).expect("a derived ID is never empty")
}

fn taken(world: &World, candidate: &str) -> bool {
    world.entities().any(|(_, data)| {
        data.source_id
            .as_ref()
            .is_some_and(|id| id.as_str() == candidate)
    })
}
