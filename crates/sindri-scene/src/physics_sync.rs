//! Keeping a scene and the physics world in step.
//!
//! `sindri-physics` owns bodies, colliders, and the step; `sindri-core` owns
//! entities and transforms. Nothing joined them, so a scene could author a
//! collider and no collision would ever happen — the audit's "no exercised game
//! integration". This is that join, and it is the same shape as
//! [`crate::SpriteAnimations`]: runtime state beside the world, derived from
//! authored components, never serialized.
//!
//! What it does *not* do is decide when. `docs/physics.md` fixes the order
//! within one fixed update — author state in, step, results out, events
//! published — and the host decides how often that happens. A render frame
//! never steps physics.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use sindri_core::{ComponentRegistryError, ComponentSchemaRegistry, EntityId, Transform3D, World};
use sindri_physics::{
    PhysicsError, PhysicsEvent2d, PhysicsPose2d, PhysicsWorld2d, RigidBody2d, RigidBodyKind,
};
use thiserror::Error;

use crate::physics::{Collider2dComponent, RigidBody2dComponent};

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum PhysicsSyncError {
    #[error("a fixed step cannot be {0:?} long")]
    BadStep(Duration),
    #[error(transparent)]
    Registry(#[from] ComponentRegistryError),
    #[error(transparent)]
    Physics(#[from] PhysicsError),
}

/// The physics world a scene's authored bodies and colliders drive.
///
/// Held beside [`World`] rather than in it, for the reason every derived thing
/// is: a scene saved mid-run has to be the scene that was opened, and a body's
/// solver state is not something an author wrote.
pub struct ScenePhysics2d {
    world: PhysicsWorld2d,
    /// Which entities are in the physics world, and what they were authored as.
    ///
    /// Kept so a re-registration can be told from an unchanged one: rebuilding
    /// every body every step would throw away the velocity the simulation just
    /// computed, which is the whole state physics owns.
    registered: BTreeMap<EntityId, Authored>,
    events: Vec<PhysicsEvent2d>,
}

/// What a scene said about one entity, as far as physics is concerned.
///
/// Compared rather than the whole component, because a change to *anything*
/// physics reads means the body has to be rebuilt, and a change to anything
/// else must not.
#[derive(Clone, Copy, PartialEq)]
struct Authored {
    body: Option<RigidBody2d>,
    collider: sindri_physics::Collider2d,
    kind: RigidBodyKind,
}

impl ScenePhysics2d {
    /// A physics world under `gravity`, in scene units per second squared.
    ///
    /// Gravity belongs to the host rather than to a component, because it is a
    /// fact about the world and not about an entity. A top-down game passes
    /// zero, which is why [`Self::top_down`] exists and says so by name.
    pub fn new(gravity: [f32; 2]) -> Result<Self, PhysicsSyncError> {
        Ok(Self {
            world: PhysicsWorld2d::new(gravity)?,
            registered: BTreeMap::new(),
            events: Vec::new(),
        })
    }

    /// A world with no gravity, for a game seen from above.
    pub fn top_down() -> Result<Self, PhysicsSyncError> {
        Self::new([0.0, 0.0])
    }

    /// The physics world itself, for the operations gameplay drives directly:
    /// velocity, impulses, kinematic targets.
    pub const fn world(&self) -> &PhysicsWorld2d {
        &self.world
    }

    pub const fn world_mut(&mut self) -> &mut PhysicsWorld2d {
        &mut self.world
    }

    /// What collided during the last step, in the order the backend reported.
    pub fn events(&self) -> &[PhysicsEvent2d] {
        &self.events
    }

    /// The world to drive and the events to read, together.
    ///
    /// Both at once because a script pass needs both and they are disjoint
    /// halves of this: asking for them separately would be one mutable and one
    /// shared borrow of the same value, which is a thing a caller should not
    /// have to work around.
    pub const fn for_scripts(&mut self) -> (&mut PhysicsWorld2d, &[PhysicsEvent2d]) {
        (&mut self.world, self.events.as_slice())
    }

    /// One fixed update: author state in, step, results out.
    ///
    /// The order is `docs/physics.md`'s and is not a detail — a body registered
    /// after the step would miss it, and a transform written back before the
    /// step would be the previous frame's.
    pub fn step(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        delta: Duration,
    ) -> Result<(), PhysicsSyncError> {
        if delta.is_zero() || !delta.as_secs_f32().is_finite() {
            return Err(PhysicsSyncError::BadStep(delta));
        }
        self.synchronize(world, components)?;
        // Anything a script set a velocity for that never got a body: it had
        // its chance above, and keeping it would mean a mistaken write sitting
        // in the map for the rest of the run.
        self.world.forget_pending();
        self.events = self.world.step(delta)?;
        self.write_back(world);
        Ok(())
    }

    /// Registers what is newly authored, updates what changed, and forgets what
    /// is gone.
    ///
    /// Called every step rather than once, because entities are spawned and
    /// despawned by scripts now: a physics world built at load would know
    /// nothing about a bullet.
    fn synchronize(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> Result<(), PhysicsSyncError> {
        let mut live = BTreeSet::new();
        for (entity, collider) in components.query::<Collider2dComponent>(world)? {
            live.insert(entity);
            let body = components
                .get::<RigidBody2dComponent>(world, entity)?
                .map(|authored| authored.0);
            let authored = Authored {
                body,
                collider: collider.0,
                kind: body.map_or(RigidBodyKind::Static, |body| body.kind),
            };
            match self.registered.get(&entity) {
                // Unchanged: leave the body alone. Rebuilding it would discard
                // the velocity and contacts the simulation owns, which is every
                // frame's worth of physics.
                Some(previous) if *previous == authored => continue,
                Some(_) => {
                    self.world.remove(entity);
                }
                None => {}
            }
            // The authored pose comes from the entity's transform, which is the
            // one place a position is written down. A body's own pose field is
            // where physics puts the answer back, not a second authored truth.
            let pose = pose_of(world, entity, authored.body);
            let outcome = match authored.body {
                Some(body) => {
                    self.world
                        .insert_body(entity, RigidBody2d { pose, ..body }, authored.collider)
                }
                None => self
                    .world
                    .insert_static_collider(entity, pose, authored.collider),
            };
            match outcome {
                Ok(()) => {
                    self.registered.insert(entity, authored);
                }
                // A body the backend refuses is reported once and skipped, not
                // retried every step: an invalid collider would otherwise fill
                // a console sixty times a second with the same line.
                Err(error) => {
                    self.registered.remove(&entity);
                    return Err(error.into());
                }
            }
        }

        // Anything that stopped being authored — despawned, switched off, or
        // had its collider removed — leaves the physics world with it. A body
        // outliving its entity would collide with things on behalf of nothing.
        let gone: Vec<EntityId> = self
            .registered
            .keys()
            .filter(|entity| !live.contains(entity))
            .copied()
            .collect();
        for entity in gone {
            self.world.remove(entity);
            self.registered.remove(&entity);
        }
        Ok(())
    }

    /// Puts what physics decided back into the transforms the renderer reads.
    ///
    /// Only for bodies physics owns the position of. A static body's position
    /// is the author's, and writing it back would fight whoever moved it.
    fn write_back(&mut self, world: &mut World) {
        for (entity, authored) in &self.registered {
            if !matches!(
                authored.kind,
                RigidBodyKind::Dynamic | RigidBodyKind::KinematicVelocity
            ) {
                continue;
            }
            let Ok(pose) = self.world.pose(*entity) else {
                continue;
            };
            let Some(data) = world.get_mut(*entity) else {
                continue;
            };
            let Some(mut transform) = data.transform_3d else {
                continue;
            };
            // X, Y and the rotation about Z. The Z position and the 3D scale are
            // the author's and are preserved, which is what `docs/2d-model.md`
            // means by one transform: a 2D entity keeps to a plane rather than
            // having a transform of its own kind.
            transform.set_position_2d(pose.position);
            transform.set_rotation_z_radians(pose.rotation);
            // A Z-locked transform is one an author said stays on its layer,
            // and physics is a write path like any other.
            if !transform.z_lock_rejects(data.transform_3d) {
                data.transform_3d = Some(transform);
            }
        }
    }
}

/// Where an entity is, for physics to start from.
///
/// The transform, because that is where a position is written down. A body
/// component's pose is where physics writes its answer, and treating it as a
/// second authored truth is how the two drift.
fn pose_of(world: &World, entity: EntityId, body: Option<RigidBody2d>) -> PhysicsPose2d {
    world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .map_or_else(
            || body.map(|body| body.pose).unwrap_or_default(),
            |transform: Transform3D| PhysicsPose2d {
                position: transform.position_2d(),
                rotation: transform.rotation_z_radians(),
            },
        )
}
