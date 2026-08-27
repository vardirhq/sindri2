//! Where an entity sits among its siblings.
//!
//! It used to be nowhere: the hierarchy sorted siblings by stable ID, so
//! authoring order was alphabetical by a string most authors never look at.
//! Five pips made from one arrived as `pip-1`, `pip-1-copy`, `pip-1-copy-2`,
//! and putting them in the order the HUD reads them meant renaming their IDs.
//!
//! The order is recorded in `EntityData.editor`, the map a runtime carries but
//! never interprets. That is deliberate rather than convenient. A scene's
//! document order is canonical — sorted by ID, and explicitly meaningless, so
//! that a save stays stable while entities are added and reparented — and draw
//! order is expressed by render layers and depths. So sibling order is a fact
//! about a panel, not about a scene being played, and the editor's own section
//! of the file is exactly where a fact about a panel belongs.

use sindri_core::{CommandBuffer, EntityData, EntityId, World, WorldCommand};

/// What the editor's section of an entity calls its place among its siblings.
pub const ORDER_KEY: &str = "order";

/// Which of two siblings comes first.
///
/// An entity with no recorded place sorts after every entity that has one, and
/// ties break on the stable ID — which is what the whole order was before, and
/// so is what a scene nobody has reordered still looks like. An entity created
/// after a reorder therefore arrives at the bottom of its parent's list, which
/// is where a new thing belongs and where the eye looks for it.
#[must_use]
pub fn sibling_key(world: &World, entity: EntityId) -> (i64, String) {
    let Some(data) = world.get(entity) else {
        return (i64::MAX, String::new());
    };
    (
        rank(data).unwrap_or(i64::MAX),
        data.source_id.as_ref().map_or_else(
            || format!("~{:010}", entity.index()),
            |id| id.as_str().to_owned(),
        ),
    )
}

/// The place this entity records, or `None` for one that records none.
#[must_use]
pub fn rank(data: &EntityData) -> Option<i64> {
    data.editor.get(ORDER_KEY)?.as_i64()
}

/// One parent's children, in the order the hierarchy draws them.
///
/// `None` means the top level, whose children are the entities with no parent.
#[must_use]
pub fn siblings(world: &World, parent: Option<EntityId>) -> Vec<EntityId> {
    let mut siblings: Vec<EntityId> = match parent {
        Some(parent) => world.get(parent).map(|data| data.children.clone()),
        None => Some(
            world
                .entities()
                .filter(|(_, data)| data.parent.is_none())
                .map(|(entity, _)| entity)
                .collect(),
        ),
    }
    .unwrap_or_default();
    siblings.sort_by_key(|sibling| sibling_key(world, *sibling));
    siblings
}

/// The commands that move an entity `offset` places among its siblings.
///
/// Empty when there is nowhere to move to, which is what the ends of a list
/// are: the first row's Move up is refused rather than wrapping around, and a
/// refusal that changes nothing is better than a move that surprises.
///
/// Every sibling is stamped, not only the two that swapped. Recording one
/// entity's place and leaving the rest to sort by ID would put it in a list
/// half of which is alphabetical, so the second move would read an order that
/// is not the one on screen. Stamping the lot means the recorded order and the
/// drawn order are the same list, always, and it is what makes this idempotent:
/// a parent already in order produces no commands at all.
#[must_use]
pub fn move_by(world: &World, entity: EntityId, offset: isize) -> CommandBuffer {
    let mut buffer = CommandBuffer::new();
    let Some(parent) = world.get(entity).map(|data| data.parent) else {
        return buffer;
    };
    let mut order = siblings(world, parent);
    let Some(from) = order.iter().position(|sibling| *sibling == entity) else {
        return buffer;
    };
    let Some(to) = checked_move(from, offset, order.len()) else {
        return buffer;
    };
    let moved = order.remove(from);
    order.insert(to, moved);
    stamp(world, &order, &mut buffer);
    buffer
}

/// Whether an entity has anywhere to go in this direction.
///
/// Asked so a menu can grey out Move up on the first row rather than offering
/// it and doing nothing, which is how an interface teaches people to stop
/// trusting it.
#[must_use]
pub fn can_move(world: &World, entity: EntityId, offset: isize) -> bool {
    let Some(parent) = world.get(entity).map(|data| data.parent) else {
        return false;
    };
    let order = siblings(world, parent);
    order
        .iter()
        .position(|sibling| *sibling == entity)
        .and_then(|from| checked_move(from, offset, order.len()))
        .is_some()
}

/// Where an index lands after a move, or `None` for off the end of the list.
fn checked_move(from: usize, offset: isize, len: usize) -> Option<usize> {
    let to = isize::try_from(from).ok()?.checked_add(offset)?;
    let to = usize::try_from(to).ok()?;
    (to < len && to != from).then_some(to)
}

/// The commands that make the recorded order this list.
///
/// Only what differs, so a no-op move produces an empty transaction and the
/// document is not marked unsaved by a menu entry that changed nothing.
fn stamp(world: &World, order: &[EntityId], buffer: &mut CommandBuffer) {
    for (place, sibling) in order.iter().enumerate() {
        let Ok(place) = i64::try_from(place) else {
            return;
        };
        if world.get(*sibling).and_then(rank) == Some(place) {
            continue;
        }
        buffer.push(WorldCommand::SetEditorEntry {
            entity: *sibling,
            key: ORDER_KEY.to_owned(),
            value: Some(place.into()),
        });
    }
}

#[cfg(test)]
mod tests;
