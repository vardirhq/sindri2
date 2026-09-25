//! Which space a handle works in.
//!
//! A child's transform is stored relative to its parent, but a handle is drawn
//! and dragged in the world, where the person sees the thing. These translate
//! between the two; a UI element keeps its own transform, which the overlay
//! already places inside its parent's box.

use sindri_core::{EntityId, Transform3D};

use super::EditorApp;

impl EditorApp {
    /// The transform a handle is drawn at and dragged from: where the entity is
    /// in the world, or, for a UI element, its own transform, which the overlay
    /// already places inside its parent.
    pub(super) fn gizmo_subject(&self, entity: EntityId) -> Option<Transform3D> {
        if self.is_overlaid(entity) {
            self.world.get(entity)?.transform_3d
        } else {
            self.world.world_transform(entity)
        }
    }

    /// What a dragged handle's answer is stored as: a child's place relative
    /// to its parent, since a handle works in the world.
    pub(super) fn stored_from_gizmo(&self, entity: EntityId, placed: Transform3D) -> Transform3D {
        if self.is_overlaid(entity) {
            placed
        } else {
            self.world.local_for_world(entity, placed)
        }
    }
}
