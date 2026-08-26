//! Parentage: checking a move, making one, and taking a subtree out.
//!
//! Despawning is here because it is a hierarchy operation: a despawned
//! entity takes its descendants with it, and undo has to put the whole
//! subtree back where it was, siblings in order.

use crate::EntityId;

use super::{EntityData, World, WorldError};

impl World {
    /// Whether [`World::set_parent`] would accept this move, without making it.
    ///
    /// An interface that offers reparenting has to answer this before the move
    /// happens — a hierarchy refusing a drop while it is being dragged is a
    /// different thing from one that accepts it and then reports an error.
    /// Sharing the rule with `set_parent` is what keeps the two answers the
    /// same; asking it twice in two places is how they stop agreeing.
    pub fn check_set_parent(
        &self,
        child: EntityId,
        parent: Option<EntityId>,
    ) -> Result<(), WorldError> {
        self.require(child)?;
        if let Some(parent) = parent {
            self.require(parent)?;
            if child == parent || self.is_descendant(parent, child) {
                return Err(WorldError::HierarchyCycle);
            }
        }
        Ok(())
    }

    pub fn set_parent(
        &mut self,
        child: EntityId,
        parent: Option<EntityId>,
    ) -> Result<(), WorldError> {
        self.check_set_parent(child, parent)?;

        let previous = self.get(child).and_then(|data| data.parent);
        if previous == parent {
            return Ok(());
        }
        if let Some(previous) = previous
            && let Some(data) = self.get_mut(previous)
        {
            data.children.retain(|candidate| *candidate != child);
        }
        self.get_mut(child).expect("validated child").parent = parent;
        if let Some(parent) = parent {
            self.get_mut(parent)
                .expect("validated parent")
                .children
                .push(child);
        }
        Ok(())
    }

    /// Every entity in a subtree, parents before their children, with the data
    /// each holds right now.
    ///
    /// Captured before anything is removed, because removing an entity edits
    /// its parent's child list — so a capture taken during a removal would
    /// record lists already missing their siblings.
    pub fn capture_subtree(
        &self,
        entity: EntityId,
    ) -> Result<Vec<(EntityId, EntityData)>, WorldError> {
        self.require(entity)?;
        let mut captured = Vec::new();
        let mut queue = vec![entity];
        while let Some(current) = queue.pop() {
            let Some(data) = self.get(current) else {
                continue;
            };
            queue.extend(data.children.iter().copied());
            captured.push((current, data.clone()));
        }
        Ok(captured)
    }

    /// Where an entity sits among its parent's children, so putting it back
    /// puts it back in the same place rather than at the end.
    #[must_use]
    pub fn sibling_index(&self, entity: EntityId) -> Option<usize> {
        let parent = self.get(entity)?.parent?;
        self.get(parent)?
            .children
            .iter()
            .position(|child| *child == entity)
    }

    /// Puts an entity back into its parent's child list at a known position.
    pub fn relink_child(
        &mut self,
        entity: EntityId,
        sibling_index: Option<usize>,
    ) -> Result<(), WorldError> {
        let Some(parent) = self.get(entity).and_then(|data| data.parent) else {
            return Ok(());
        };
        let Some(data) = self.get_mut(parent) else {
            return Err(WorldError::InvalidEntity(parent));
        };
        if data.children.contains(&entity) {
            return Ok(());
        }
        let at = sibling_index
            .unwrap_or(data.children.len())
            .min(data.children.len());
        data.children.insert(at, entity);
        Ok(())
    }

    pub fn despawn_recursive(&mut self, entity: EntityId) -> Result<Vec<EntityId>, WorldError> {
        self.require(entity)?;
        let mut stack = vec![entity];
        let mut removed = Vec::new();
        while let Some(current) = stack.pop() {
            if let Some(data) = self.get(current) {
                stack.extend(data.children.iter().copied());
            }
            if self.remove_one(current) {
                removed.push(current);
            }
        }
        Ok(removed)
    }

    fn is_descendant(&self, candidate: EntityId, ancestor: EntityId) -> bool {
        let mut cursor = Some(candidate);
        while let Some(entity) = cursor {
            if entity == ancestor {
                return true;
            }
            cursor = self.get(entity).and_then(|data| data.parent);
        }
        false
    }

    fn remove_one(&mut self, entity: EntityId) -> bool {
        let parent = self.get(entity).and_then(|data| data.parent);
        if let Some(parent) = parent
            && let Some(data) = self.get_mut(parent)
        {
            data.children.retain(|candidate| *candidate != entity);
        }
        let Some(slot) = self.slot_mut(entity) else {
            return false;
        };
        if slot.data.take().is_none() {
            return false;
        }
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(entity.index());
        self.len -= 1;
        true
    }
}
