//! The 2D world: Rapier behind Sindri's own types.
//!
//! Rapier appears nowhere else. Scenes, the editor, Decay, and games
//! speak only in the types this crate defines, which is what makes the
//! backend replaceable.

use std::{collections::HashMap, sync::mpsc, time::Duration};

use rapier2d::prelude as r2;
use sindri_core::EntityId;

use crate::shared::RigidBodyKind;
use crate::types2d::{
    Collider2d, ColliderShape2d, PhysicsEvent2d, PhysicsEventKind, PhysicsPose2d, RigidBody2d,
};
use crate::validate::{
    PhysicsError, finite2, validate_body2d, validate_collider2d, validate_pose2d,
};

#[derive(Clone, Copy)]
struct BodyRecord2d {
    body: r2::RigidBodyHandle,
    collider: r2::ColliderHandle,
    kind: RigidBodyKind,
}

/// The first runtime physics world. It owns Rapier completely and exposes only
/// Sindri entities, values, and events.
pub struct PhysicsWorld2d {
    backend: r2::PhysicsWorld,
    bodies: HashMap<EntityId, BodyRecord2d>,
    collider_entities: HashMap<r2::ColliderHandle, EntityId>,
    /// Velocities set on a body that does not exist yet.
    ///
    /// A script spawns a bullet and sets its velocity in the same pass — which
    /// is the shape `docs/scripting.md` documents and the reason spawned
    /// scripts start in the pass that made them. But a body is built when the
    /// scene next synchronizes, which is the *following* frame, so the set
    /// arrived before there was anything to set. Refusing it would make the
    /// documented shape impossible; dropping it would make a bullet sit still.
    /// So it is kept, and applied when the body arrives.
    ///
    /// Only ever holds entities that were asked about between a spawn and the
    /// next synchronize: anything still here after that never had a body
    /// authored, and is discarded by `forget_pending`.
    pending_velocity: HashMap<EntityId, [f32; 2]>,
}

impl PhysicsWorld2d {
    pub fn new(gravity: [f32; 2]) -> Result<Self, PhysicsError> {
        finite2("gravity", gravity)?;
        let mut backend = r2::PhysicsWorld::new();
        backend.gravity = r2::Vector::new(gravity[0], gravity[1]);
        Ok(Self {
            backend,
            bodies: HashMap::new(),
            collider_entities: HashMap::new(),
            pending_velocity: HashMap::new(),
        })
    }

    pub fn insert_body(
        &mut self,
        entity: EntityId,
        body: RigidBody2d,
        collider: Collider2d,
    ) -> Result<(), PhysicsError> {
        if self.bodies.contains_key(&entity) {
            return Err(PhysicsError::EntityAlreadyRegistered(entity));
        }
        validate_body2d(&body)?;
        validate_collider2d(&collider)?;

        let builder = body_builder(body);
        let collider_builder = collider_builder(entity, collider);
        let (body_handle, collider_handle) = self.backend.insert(builder, collider_builder);
        self.bodies.insert(
            entity,
            BodyRecord2d {
                body: body_handle,
                collider: collider_handle,
                kind: body.kind,
            },
        );
        self.collider_entities.insert(collider_handle, entity);
        // What a script asked for before this existed.
        if let Some(velocity) = self.pending_velocity.remove(&entity)
            && matches!(
                body.kind,
                RigidBodyKind::Dynamic | RigidBodyKind::KinematicVelocity
            )
        {
            self.backend.bodies[body_handle]
                .set_linvel(r2::Vector::new(velocity[0], velocity[1]), true);
        }
        Ok(())
    }

    /// Remembers a velocity for a body that has not been built yet.
    ///
    /// For the one caller that knows the entity is real and authored physics —
    /// the scripting host, which can see the components on it — and would
    /// otherwise have to refuse the frame a bullet is spawned on. Everything
    /// else should use `set_linear_velocity` and hear about a body that is not
    /// there.
    pub fn remember_linear_velocity(
        &mut self,
        entity: EntityId,
        velocity: [f32; 2],
    ) -> Result<(), PhysicsError> {
        finite2("linear_velocity", velocity)?;
        self.pending_velocity.insert(entity, velocity);
        Ok(())
    }

    /// Drops anything remembered for an entity that never got a body.
    ///
    /// Called once a synchronize has had its chance to build them, so a
    /// mistaken write cannot sit in the map for the rest of the run.
    pub fn forget_pending(&mut self) {
        self.pending_velocity.clear();
    }

    /// Inserts an entity with no authored rigid-body as static collision
    /// geometry. The hidden backend may synthesize a fixed body; that choice is
    /// intentionally not observable through the Sindri API.
    pub fn insert_static_collider(
        &mut self,
        entity: EntityId,
        pose: PhysicsPose2d,
        collider: Collider2d,
    ) -> Result<(), PhysicsError> {
        self.insert_body(
            entity,
            RigidBody2d {
                kind: RigidBodyKind::Static,
                pose,
                ..RigidBody2d::default()
            },
            collider,
        )
    }

    pub fn remove(&mut self, entity: EntityId) -> bool {
        let Some(record) = self.bodies.remove(&entity) else {
            return false;
        };
        self.collider_entities.remove(&record.collider);
        let _ = self.backend.remove_body(record.body);
        true
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.bodies.contains_key(&entity)
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    pub fn body_kind(&self, entity: EntityId) -> Result<RigidBodyKind, PhysicsError> {
        Ok(self.record(entity)?.kind)
    }

    pub fn pose(&self, entity: EntityId) -> Result<PhysicsPose2d, PhysicsError> {
        let record = self.record(entity)?;
        let body = &self.backend.bodies[record.body];
        let position = body.translation();
        Ok(PhysicsPose2d {
            position: [position.x, position.y],
            rotation: body.rotation().angle(),
        })
    }

    pub fn linear_velocity(&self, entity: EntityId) -> Result<[f32; 2], PhysicsError> {
        let record = self.record(entity)?;
        let velocity = self.backend.bodies[record.body].linvel();
        Ok([velocity.x, velocity.y])
    }

    pub fn set_linear_velocity(
        &mut self,
        entity: EntityId,
        velocity: [f32; 2],
    ) -> Result<(), PhysicsError> {
        finite2("linear_velocity", velocity)?;
        let record = *self.record(entity)?;
        if !matches!(
            record.kind,
            RigidBodyKind::Dynamic | RigidBodyKind::KinematicVelocity
        ) {
            return Err(PhysicsError::WrongBodyKind(
                entity,
                "set linear velocity",
                record.kind,
            ));
        }
        self.backend.bodies[record.body]
            .set_linvel(r2::Vector::new(velocity[0], velocity[1]), true);
        Ok(())
    }

    pub fn apply_impulse(
        &mut self,
        entity: EntityId,
        impulse: [f32; 2],
    ) -> Result<(), PhysicsError> {
        finite2("impulse", impulse)?;
        let record = *self.record(entity)?;
        if record.kind != RigidBodyKind::Dynamic {
            return Err(PhysicsError::WrongBodyKind(
                entity,
                "apply impulse",
                record.kind,
            ));
        }
        self.backend.bodies[record.body]
            .apply_impulse(r2::Vector::new(impulse[0], impulse[1]), true);
        Ok(())
    }

    pub fn set_kinematic_target(
        &mut self,
        entity: EntityId,
        pose: PhysicsPose2d,
    ) -> Result<(), PhysicsError> {
        validate_pose2d(pose)?;
        let record = *self.record(entity)?;
        if record.kind != RigidBodyKind::KinematicPosition {
            return Err(PhysicsError::WrongBodyKind(
                entity,
                "set kinematic target",
                record.kind,
            ));
        }
        self.backend.bodies[record.body].set_next_kinematic_position(r2::Pose::new(
            r2::Vector::new(pose.position[0], pose.position[1]),
            pose.rotation,
        ));
        Ok(())
    }

    /// Advances exactly one engine fixed step and returns normalized Sindri
    /// collision/sensor events generated during that step.
    pub fn step(&mut self, delta: Duration) -> Result<Vec<PhysicsEvent2d>, PhysicsError> {
        let dt = delta.as_secs_f32();
        if !dt.is_finite() || dt <= 0.0 {
            return Err(PhysicsError::InvalidTimestep);
        }
        self.backend.integration_parameters.dt = dt;

        let (collision_send, collision_recv) = mpsc::channel();
        let (force_send, _force_recv) = mpsc::channel();
        let events = r2::ChannelEventCollector::new(collision_send, force_send);
        self.backend.step_with_events(&(), &events);

        Ok(collision_recv
            .try_iter()
            .filter_map(|event| self.normalize_event(event))
            .collect())
    }

    fn record(&self, entity: EntityId) -> Result<&BodyRecord2d, PhysicsError> {
        self.bodies
            .get(&entity)
            .ok_or(PhysicsError::MissingEntity(entity))
    }

    fn normalize_event(&self, event: r2::CollisionEvent) -> Option<PhysicsEvent2d> {
        let mut first = *self.collider_entities.get(&event.collider1())?;
        let mut second = *self.collider_entities.get(&event.collider2())?;
        if second < first {
            std::mem::swap(&mut first, &mut second);
        }
        let kind = match (event.started(), event.sensor()) {
            (true, false) => PhysicsEventKind::CollisionStarted,
            (false, false) => PhysicsEventKind::CollisionStopped,
            (true, true) => PhysicsEventKind::SensorEntered,
            (false, true) => PhysicsEventKind::SensorExited,
        };
        Some(PhysicsEvent2d {
            first,
            second,
            kind,
        })
    }
}

fn body_builder(body: RigidBody2d) -> r2::RigidBodyBuilder {
    let builder = match body.kind {
        RigidBodyKind::Static => r2::RigidBodyBuilder::fixed(),
        RigidBodyKind::Dynamic => r2::RigidBodyBuilder::dynamic(),
        RigidBodyKind::KinematicPosition => r2::RigidBodyBuilder::kinematic_position_based(),
        RigidBodyKind::KinematicVelocity => r2::RigidBodyBuilder::kinematic_velocity_based(),
    }
    .translation(r2::Vector::new(
        body.pose.position[0],
        body.pose.position[1],
    ))
    .rotation(body.pose.rotation)
    .linvel(r2::Vector::new(
        body.linear_velocity[0],
        body.linear_velocity[1],
    ))
    .angvel(body.angular_velocity)
    .gravity_scale(body.gravity_scale)
    .linear_damping(body.linear_damping)
    .angular_damping(body.angular_damping);

    if body.lock_rotation {
        builder.lock_rotations()
    } else {
        builder
    }
}

fn collider_builder(entity: EntityId, collider: Collider2d) -> r2::ColliderBuilder {
    let groups = r2::InteractionGroups::new(
        r2::Group::from_bits_retain(collider.layers.memberships),
        r2::Group::from_bits_retain(collider.layers.filter),
        r2::InteractionTestMode::And,
    );
    let builder = match collider.shape {
        ColliderShape2d::Box { half_extents } => {
            r2::ColliderBuilder::cuboid(half_extents[0], half_extents[1])
        }
        ColliderShape2d::Circle { radius } => r2::ColliderBuilder::ball(radius),
        ColliderShape2d::Capsule {
            half_height,
            radius,
        } => r2::ColliderBuilder::capsule_y(half_height, radius),
    };

    builder
        .translation(r2::Vector::new(collider.offset[0], collider.offset[1]))
        .rotation(collider.rotation)
        .sensor(collider.sensor)
        .collision_groups(groups)
        .active_collision_types(r2::ActiveCollisionTypes::all())
        .active_events(r2::ActiveEvents::COLLISION_EVENTS)
        .friction(collider.friction)
        .restitution(collider.restitution)
        .user_data(u128::from(entity.to_bits()))
}
