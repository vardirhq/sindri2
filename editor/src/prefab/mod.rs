//! Putting an authored prefab into the open scene.
//!
//! The runtime already spawns prefabs, and deliberately does it differently:
//! `World::spawn_prefab` gives its entities no stable identity, because a
//! prefab's identities name entities *inside the prefab* and two instances
//! would collide on every one of them. That is right for a bullet that exists
//! for a second and is never saved.
//!
//! An editor is authoring a document. What it instantiates has to be saveable,
//! so every entity needs a stable identity that nothing else in the scene is
//! using, and the whole thing has to arrive as one undoable step. So this is
//! not a wrapper around the runtime's spawn; it is the same idea answered for
//! a file rather than a frame.
//!
//! The handle problem, and its answer, are `duplicate.rs`'s: `WorldCommand::
//! Spawn` names the handle it spawns at, and `World::next_handle` is a peek
//! rather than an allocation, so the instantiation is rehearsed into a clone of
//! the world that hands out the handles the real one is about to.

use std::path::Path;

use sindri_core::{
    CommandBuffer, EntityData, EntityId, PrefabDocument, SceneEntity, SceneEntityId, World,
    WorldCommand,
};

/// Reads a prefab from the project, or says why it cannot be used.
///
/// Validated here rather than at instantiation, so a prefab with two roots is
/// refused while it is still an asset somebody chose and not a half-built
/// subtree in their scene.
pub fn load(root: Option<&Path>, relative: &str) -> Result<PrefabDocument, String> {
    let root = root.ok_or_else(|| "This project has no assets folder".to_owned())?;
    let path = root.join(relative);
    let json = std::fs::read_to_string(&path)
        .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
    let document = PrefabDocument::from_json(&json)
        .map_err(|error| format!("{relative} is not valid: {error}"))?;
    document
        .validate()
        .map_err(|error| format!("{relative} cannot be instantiated: {error}"))?;
    Ok(document)
}

/// Adds the commands that put `prefab` into the scene under `parent`, and
/// answers with the handle its root will land on.
///
/// `rehearsal` is a clone of the world that receives exactly the spawns the
/// real one is about to, so it hands out the handles the real one will and
/// knows which stable IDs are taken as they are taken. Taken rather than made
/// here for the reason duplication takes one: instantiating twice in a single
/// transaction would otherwise pick the same handle and the same ID twice, and
/// spawn the second thing on top of the first.
pub fn instantiate_into(
    rehearsal: &mut World,
    prefab: &PrefabDocument,
    parent: Option<EntityId>,
    buffer: &mut CommandBuffer,
) -> Result<EntityId, String> {
    let root = prefab
        .root()
        .map_err(|error| format!("this prefab has no single root: {error}"))?;
    Ok(spawn_into(rehearsal, prefab, root, parent, buffer))
}

/// Spawns one authored entity and then everything under it, parents first.
///
/// Parents first so a child can name the handle its parent was given. The
/// prefab's own parent links are authored identities, which mean nothing in
/// this world; what a child is actually parented to is whatever its parent
/// just became.
fn spawn_into(
    rehearsal: &mut World,
    prefab: &PrefabDocument,
    entity: &SceneEntity,
    parent: Option<EntityId>,
    buffer: &mut CommandBuffer,
) -> EntityId {
    let data = EntityData {
        source_id: Some(unused_id(rehearsal, &entity.id)),
        name: entity.name.clone(),
        parent,
        // Rebuilt by the recursion below. Taking the authored list would name
        // identities that are the prefab's rather than entities in this world.
        children: Vec::new(),
        transform_3d: entity.transform_3d,
        components: entity.components.clone(),
        // A prefab that authored something switched off meant it.
        disabled: entity.disabled,
        // Editor state describes the prefab as a document -- what was folded
        // open while somebody edited it -- and says nothing about an instance.
        editor: std::collections::BTreeMap::new(),
    };
    let handle = rehearsal.spawn(data.clone());
    rehearsal
        .set_parent(handle, parent)
        .expect("a fresh entity accepts the parent it was given");
    buffer.push(WorldCommand::Spawn {
        entity: handle,
        data: Box::new(data),
    });
    for child in prefab.children_of(&entity.id) {
        spawn_into(rehearsal, prefab, child, Some(handle), buffer);
    }
    handle
}

/// A stable ID like the prefab's that nothing in the scene is using.
///
/// Derived from the authored one rather than generated, because `oak-tree`
/// says what it is and `game-object-7` does not. Instantiating the same prefab
/// twice is the ordinary case, so the collision path is the common one rather
/// than the exception.
fn unused_id(world: &World, authored: &SceneEntityId) -> SceneEntityId {
    let stem = authored.as_str();
    if !taken(world, stem) {
        return authored.clone();
    }
    let mut suffix = 2_u32;
    loop {
        let candidate = format!("{stem}-{suffix}");
        if !taken(world, &candidate) {
            return SceneEntityId::new(candidate).expect("a derived ID is never empty");
        }
        suffix += 1;
    }
}

fn taken(world: &World, candidate: &str) -> bool {
    world.entities().any(|(_, data)| {
        data.source_id
            .as_ref()
            .is_some_and(|id| id.as_str() == candidate)
    })
}

#[cfg(test)]
mod tests;
