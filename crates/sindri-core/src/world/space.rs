//! Where an entity is in the world, when what it stores is local to its parent.
//!
//! A child's `transform_3d` is relative to its parent, as it is in every other
//! engine: move a ship and its turret goes with it, turn it and the turret
//! turns, grow it and the turret grows. What is stored is the local transform,
//! because that is what an author edits and what survives the parent moving.
//! What is drawn, simulated and heard is the world transform, composed here
//! from the parent chain.
//!
//! Scale composes per axis, which is exact while no rotated parent scales
//! unevenly. That is the case for everything a game here builds; a sheared
//! world transform would need a matrix, and nothing yet reads one.
//!
//! Screen UI does not use this. An element is laid out in its parent's box by
//! `sindri_scene::screen_ui`, which already reads its transform as local to
//! that box.

use crate::{EntityId, Transform3D};

use super::World;

/// How deep a parent chain is followed. The world refuses cycles; this is the
/// guard should one ever get in, so a lookup cannot hang a frame.
const DEPTH_LIMIT: usize = 256;

impl Transform3D {
    /// This transform, held by a parent at `parent`: where it is in the
    /// parent's space.
    #[must_use]
    pub fn within(self, parent: Self) -> Self {
        let scaled = [
            self.position[0] * parent.scale[0],
            self.position[1] * parent.scale[1],
            self.position[2] * parent.scale[2],
        ];
        let turned = rotate(parent.rotation, scaled);
        Self {
            position: [
                parent.position[0] + turned[0],
                parent.position[1] + turned[1],
                parent.position[2] + turned[2],
            ],
            rotation: normalized(multiply(parent.rotation, self.rotation)),
            scale: [
                self.scale[0] * parent.scale[0],
                self.scale[1] * parent.scale[1],
                self.scale[2] * parent.scale[2],
            ],
            z_locked: self.z_locked,
        }
    }

    /// The local transform that puts something at this world transform when
    /// its parent is at `parent`: the inverse of [`Self::within`].
    ///
    /// A parent scaled to zero on an axis cannot be undone on that axis; the
    /// local value there is left at what a unit parent would give.
    #[must_use]
    pub fn relative_to(self, parent: Self) -> Self {
        let inverse = conjugate(normalized(parent.rotation));
        let offset = rotate(
            inverse,
            [
                self.position[0] - parent.position[0],
                self.position[1] - parent.position[1],
                self.position[2] - parent.position[2],
            ],
        );
        let divide = |value: f32, by: f32| {
            if by.abs() > f32::EPSILON {
                value / by
            } else {
                value
            }
        };
        Self {
            position: [
                divide(offset[0], parent.scale[0]),
                divide(offset[1], parent.scale[1]),
                divide(offset[2], parent.scale[2]),
            ],
            rotation: normalized(multiply(inverse, self.rotation)),
            scale: [
                divide(self.scale[0], parent.scale[0]),
                divide(self.scale[1], parent.scale[1]),
                divide(self.scale[2], parent.scale[2]),
            ],
            z_locked: self.z_locked,
        }
    }
}

impl World {
    /// Where the parent chain above `entity` puts things: the world transform
    /// of its parent, or the identity for an entity at the top.
    ///
    /// A parent with no transform of its own passes its parent's through, so
    /// an entity used only to group others does not move them.
    #[must_use]
    pub fn parent_space(&self, entity: EntityId) -> Transform3D {
        let mut chain = Vec::new();
        let mut next = self.get(entity).and_then(|data| data.parent);
        while let Some(parent) = next {
            let Some(data) = self.get(parent) else {
                break;
            };
            if chain.len() >= DEPTH_LIMIT {
                break;
            }
            if let Some(transform) = data.transform_3d {
                chain.push(transform);
            }
            next = data.parent;
        }
        chain
            .into_iter()
            .rev()
            .fold(Transform3D::default(), |space, local| local.within(space))
    }

    /// Where `entity` is in the world, or `None` if it has no transform.
    #[must_use]
    pub fn world_transform(&self, entity: EntityId) -> Option<Transform3D> {
        let local = self.get(entity)?.transform_3d?;
        Some(local.within(self.parent_space(entity)))
    }

    /// The local transform that puts `entity` at `world` under its current
    /// parent.
    #[must_use]
    pub fn local_for_world(&self, entity: EntityId, world: Transform3D) -> Transform3D {
        world.relative_to(self.parent_space(entity))
    }

    /// Moves `child` under `parent` and keeps it where it was in the world, by
    /// rewriting its local transform for the new parent. What an editor's drag
    /// in a hierarchy and a script's `World.set_parent` both mean.
    ///
    /// # Errors
    /// As [`World::set_parent`].
    pub fn set_parent_keeping_place(
        &mut self,
        child: EntityId,
        parent: Option<EntityId>,
    ) -> Result<(), super::WorldError> {
        self.check_set_parent(child, parent)?;
        let world = self.world_transform(child);
        self.set_parent(child, parent)?;
        if let Some(world) = world {
            let local = self.local_for_world(child, world);
            if let Some(data) = self.get_mut(child) {
                data.transform_3d = Some(local);
            }
        }
        Ok(())
    }
}

fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

const fn conjugate([x, y, z, w]: [f32; 4]) -> [f32; 4] {
    [-x, -y, -z, w]
}

fn normalized(quaternion: [f32; 4]) -> [f32; 4] {
    let length = quaternion
        .iter()
        .map(|part| part * part)
        .sum::<f32>()
        .sqrt();
    if length > f32::EPSILON && length.is_finite() {
        quaternion.map(|part| part / length)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

fn rotate(rotation: [f32; 4], vector: [f32; 3]) -> [f32; 3] {
    let rotation = normalized(rotation);
    let [x, y, z, _] = multiply(
        multiply(rotation, [vector[0], vector[1], vector[2], 0.0]),
        conjugate(rotation),
    );
    [x, y, z]
}

#[cfg(test)]
#[path = "space_tests.rs"]
mod tests;
