//! What "the selection" is once it can be more than one entity.
//!
//! It used to be an `Option<EntityId>`, which made every bulk verb impossible
//! to express: deleting five pips meant five deletes and five undo steps, and
//! moving a row of them meant dragging each one to the same place by eye.
//!
//! Two facts rather than one. *Which entities are selected* is what the
//! hierarchy bands, what Delete removes and what a gizmo drag moves. *Which of
//! them is the primary* is what the inspector edits and where the handle is
//! drawn — because a panel of fields and a single gizmo can only be about one
//! subject, and the honest answer to "which one" is the last one pointed at.

use sindri_core::{EntityId, World};

/// The entities the editor is pointing at, in the order they were added.
///
/// Order matters twice: the last is the primary, and a range extends from the
/// primary rather than from whichever end happens to be first in the tree.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    entities: Vec<EntityId>,
}

impl Selection {
    /// The one entity the inspector edits and the gizmo is drawn on.
    pub fn primary(&self) -> Option<EntityId> {
        self.entities.last().copied()
    }

    /// Every selected entity, oldest first.
    pub fn all(&self) -> &[EntityId] {
        &self.entities
    }

    /// Whether this entity wears the selection band.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.entities.contains(&entity)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Makes this the whole selection, or empties it with `None`.
    ///
    /// The ordinary click. Reports whether the primary changed, because that is
    /// the thing a half-typed inspector field belongs to.
    pub fn replace(&mut self, entity: Option<EntityId>) -> bool {
        let before = self.primary();
        self.entities.clear();
        self.entities.extend(entity);
        before != self.primary()
    }

    /// Adds this entity, or removes it if it is already in.
    ///
    /// Ctrl-click. Adding puts it last, so the thing just pointed at becomes
    /// the primary; removing leaves whatever was pointed at before it as the
    /// primary, rather than clearing the inspector because one row was
    /// deselected.
    pub fn toggle(&mut self, entity: EntityId) -> bool {
        let before = self.primary();
        if let Some(at) = self.entities.iter().position(|held| *held == entity) {
            self.entities.remove(at);
        } else {
            self.entities.push(entity);
        }
        before != self.primary()
    }

    /// Selects everything between the primary and this entity, as `order`
    /// lists them.
    ///
    /// Shift-click, and `order` is the rows as they are drawn rather than as
    /// the world holds them: a range in a tree means the rows between two rows,
    /// and a collapsed subtree is not in it because it is not on screen.
    ///
    /// With no primary, or with either end missing from the listing, this is an
    /// ordinary click — there is no range without two ends.
    pub fn extend_through(&mut self, entity: EntityId, order: &[EntityId]) -> bool {
        let anchor = self.primary().and_then(|primary| index_of(order, primary));
        let (Some(anchor), Some(target)) = (anchor, index_of(order, entity)) else {
            return self.replace(Some(entity));
        };
        let before = self.primary();
        let span = if anchor <= target {
            &order[anchor..=target]
        } else {
            &order[target..=anchor]
        };
        // The clicked end ends up last however the range runs, so shift-click
        // leaves the inspector on the row that was clicked.
        for held in span.iter().filter(|held| **held != entity) {
            if !self.entities.contains(held) {
                self.entities.push(*held);
            }
        }
        self.toggle_to_primary(entity);
        before != self.primary()
    }

    /// Drops everything the world no longer holds.
    ///
    /// Called after anything that despawns, so a stale handle cannot be handed
    /// to a command as a subject.
    pub fn retain_live(&mut self, world: &World) {
        self.entities.retain(|entity| world.get(*entity).is_some());
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }

    /// Moves an entity to the end of the order, adding it if it is not in.
    fn toggle_to_primary(&mut self, entity: EntityId) {
        if let Some(at) = self.entities.iter().position(|held| *held == entity) {
            self.entities.remove(at);
        }
        self.entities.push(entity);
    }
}

impl FromIterator<EntityId> for Selection {
    fn from_iter<I: IntoIterator<Item = EntityId>>(entities: I) -> Self {
        Self {
            entities: entities.into_iter().collect(),
        }
    }
}

fn index_of(order: &[EntityId], entity: EntityId) -> Option<usize> {
    order.iter().position(|held| *held == entity)
}

/// The entities in a set that no other entity in the set contains.
///
/// Every bulk verb needs this, because every one of them already takes the
/// subtree: deleting a parent and its child means despawning a handle twice,
/// duplicating both means the child arrives twice, and dragging both moves the
/// child by the parent's delta and then again by its own. Selecting a parent
/// and its child is an ordinary thing to do with a rubber band or a Shift, so
/// the answer is to fold the set rather than to refuse it.
pub fn topmost(world: &World, entities: &[EntityId]) -> Vec<EntityId> {
    entities
        .iter()
        .copied()
        .filter(|entity| !has_ancestor_in(world, *entity, entities))
        .collect()
}

fn has_ancestor_in(world: &World, entity: EntityId, set: &[EntityId]) -> bool {
    let mut walked = world.get(entity).and_then(|data| data.parent);
    // Bounded by the number of entities: a cycle is impossible because
    // `World::check_set_parent` refuses to make one, and a broken handle ends
    // the walk at the first `get` that misses.
    while let Some(parent) = walked {
        if set.contains(&parent) {
            return true;
        }
        walked = world.get(parent).and_then(|data| data.parent);
    }
    false
}

/// Which way a click on a row means to change the selection.
///
/// Read from the modifiers rather than from a mode, because a selection that
/// needs a mode set first is a selection nobody makes by accident and nobody
/// makes on purpose either.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pick {
    /// This one, and only this one.
    Only,
    /// This one as well, or no longer — Ctrl, or Command on a Mac.
    Also,
    /// Everything from the primary to here — Shift.
    Through,
}

#[cfg(test)]
mod tests;
