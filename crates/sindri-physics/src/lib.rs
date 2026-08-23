//! Sindri-owned physics types and runtime adapters.
//!
//! Rapier is deliberately private implementation detail. Scenes, the editor,
//! Decay, and games speak only in the types defined here. The first runtime
//! slice is 2D; the parallel 3D data model exists so the 2D API cannot quietly
//! become the dimension-neutral contract.

use std::{collections::HashMap, sync::mpsc, time::Duration};

use rapier2d::prelude as r2;
use serde::{Deserialize, Serialize};
use sindri_core::EntityId;
use thiserror::Error;

/// What drives a rigid body's motion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RigidBodyKind {
    Static,
    #[default]
    Dynamic,
    KinematicPosition,
    KinematicVelocity,
}

/// A 32-bit collision membership/filter mask owned by Sindri.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CollisionLayers {
    pub memberships: u32,
    pub filter: u32,
}

impl CollisionLayers {
    pub const ALL: Self = Self {
        memberships: u32::MAX,
        filter: u32::MAX,
    };

    pub const fn new(memberships: u32, filter: u32) -> Self {
        Self {
            memberships,
            filter,
        }
    }
}

impl Default for CollisionLayers {
    fn default() -> Self {
        Self::ALL
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose2d {
    pub position: [f32; 2],
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigidBody2d {
    pub kind: RigidBodyKind,
    pub pose: PhysicsPose2d,
    pub linear_velocity: [f32; 2],
    pub angular_velocity: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub lock_rotation: bool,
}

impl Default for RigidBody2d {
    fn default() -> Self {
        Self {
            kind: RigidBodyKind::Dynamic,
            pose: PhysicsPose2d::default(),
            linear_velocity: [0.0; 2],
            angular_velocity: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            lock_rotation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum ColliderShape2d {
    Box { half_extents: [f32; 2] },
    Circle { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider2d {
    pub shape: ColliderShape2d,
    pub offset: [f32; 2],
    pub rotation: f32,
    pub sensor: bool,
    pub layers: CollisionLayers,
    pub friction: f32,
    pub restitution: f32,
}

impl Collider2d {
    pub const fn circle(radius: f32) -> Self {
        Self {
            shape: ColliderShape2d::Circle { radius },
            offset: [0.0; 2],
            rotation: 0.0,
            sensor: false,
            layers: CollisionLayers::ALL,
            friction: 0.5,
            restitution: 0.0,
        }
    }

    pub const fn rectangle(half_extents: [f32; 2]) -> Self {
        Self {
            shape: ColliderShape2d::Box { half_extents },
            offset: [0.0; 2],
            rotation: 0.0,
            sensor: false,
            layers: CollisionLayers::ALL,
            friction: 0.5,
            restitution: 0.0,
        }
    }
}

/// The 3D body model is deliberately parallel to 2D, but no 3D runtime is
/// claimed by this foundation slice.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigidBody3d {
    pub kind: RigidBodyKind,
    pub position: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub lock_rotation: bool,
}

impl Default for RigidBody3d {
    fn default() -> Self {
        Self {
            kind: RigidBodyKind::Dynamic,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            lock_rotation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum ColliderShape3d {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider3d {
    pub shape: ColliderShape3d,
    pub offset: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub sensor: bool,
    pub layers: CollisionLayers,
    pub friction: f32,
    pub restitution: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsEventKind {
    CollisionStarted,
    CollisionStopped,
    SensorEntered,
    SensorExited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicsEvent2d {
    pub first: EntityId,
    pub second: EntityId,
    pub kind: PhysicsEventKind,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PhysicsError {
    #[error("entity {0:?} is already registered with physics")]
    EntityAlreadyRegistered(EntityId),
    #[error("entity {0:?} has no physics body")]
    MissingEntity(EntityId),
    #[error("entity {0:?} cannot perform {1} with a {2:?} body")]
    WrongBodyKind(EntityId, &'static str, RigidBodyKind),
    #[error("physics value '{0}' must be finite")]
    NonFinite(&'static str),
    #[error("physics value '{0}' must be positive")]
    NonPositive(&'static str),
    #[error("physics value '{0}' must be non-negative")]
    Negative(&'static str),
    #[error("physics value '{0}' must be between 0 and 1")]
    NotNormalized(&'static str),
    #[error("physics timestep must be finite and greater than zero")]
    InvalidTimestep,
}

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
        Ok(())
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
        self.backend.bodies[record.body].set_linvel(
            r2::Vector::new(velocity[0], velocity[1]),
            true,
        );
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
    .translation(r2::Vector::new(body.pose.position[0], body.pose.position[1]))
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

fn validate_body2d(body: &RigidBody2d) -> Result<(), PhysicsError> {
    validate_pose2d(body.pose)?;
    finite2("linear_velocity", body.linear_velocity)?;
    finite("angular_velocity", body.angular_velocity)?;
    finite("gravity_scale", body.gravity_scale)?;
    non_negative("linear_damping", body.linear_damping)?;
    non_negative("angular_damping", body.angular_damping)
}

fn validate_collider2d(collider: &Collider2d) -> Result<(), PhysicsError> {
    finite2("collider_offset", collider.offset)?;
    finite("collider_rotation", collider.rotation)?;
    non_negative("friction", collider.friction)?;
    normalized("restitution", collider.restitution)?;
    match collider.shape {
        ColliderShape2d::Box { half_extents } => {
            positive("box_half_extent_x", half_extents[0])?;
            positive("box_half_extent_y", half_extents[1])
        }
        ColliderShape2d::Circle { radius } => positive("circle_radius", radius),
        ColliderShape2d::Capsule {
            half_height,
            radius,
        } => {
            positive("capsule_half_height", half_height)?;
            positive("capsule_radius", radius)
        }
    }
}

fn validate_pose2d(pose: PhysicsPose2d) -> Result<(), PhysicsError> {
    finite2("position", pose.position)?;
    finite("rotation", pose.rotation)
}

fn finite(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PhysicsError::NonFinite(name))
    }
}

fn finite2(name: &'static str, value: [f32; 2]) -> Result<(), PhysicsError> {
    if value.into_iter().all(f32::is_finite) {
        Ok(())
    } else {
        Err(PhysicsError::NonFinite(name))
    }
}

fn positive(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(PhysicsError::NonPositive(name))
    }
}

fn non_negative(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    finite(name, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(PhysicsError::Negative(name))
    }
}

fn normalized(name: &'static str, value: f32) -> Result<(), PhysicsError> {
    finite(name, value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(PhysicsError::NotNormalized(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: Duration = Duration::from_millis(16);

    fn entity(index: u32) -> EntityId {
        EntityId::from_bits(u64::from(index) << 32)
    }

    #[test]
    fn dynamic_body_is_advanced_by_the_fixed_step() {
        let mut world = PhysicsWorld2d::new([0.0, -9.81]).unwrap();
        let falling = entity(1);
        world
            .insert_body(falling, RigidBody2d::default(), Collider2d::circle(0.5))
            .unwrap();

        let before = world.pose(falling).unwrap();
        world.step(STEP).unwrap();
        let after = world.pose(falling).unwrap();

        assert!(after.position[1] < before.position[1]);
        assert!(world.linear_velocity(falling).unwrap()[1] < 0.0);
    }

    #[test]
    fn sensor_events_contain_only_sindri_entities() {
        let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
        let player = entity(1);
        let pickup = entity(2);
        world
            .insert_body(
                player,
                RigidBody2d::default(),
                Collider2d::circle(0.5),
            )
            .unwrap();
        let mut sensor = Collider2d::circle(1.0);
        sensor.sensor = true;
        world
            .insert_static_collider(pickup, PhysicsPose2d::default(), sensor)
            .unwrap();

        let events = world.step(STEP).unwrap();
        assert!(events.contains(&PhysicsEvent2d {
            first: player,
            second: pickup,
            kind: PhysicsEventKind::SensorEntered,
        }));
    }

    #[test]
    fn collision_layers_filter_pairs_before_the_public_event_surface() {
        let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
        let first = entity(1);
        let second = entity(2);
        let mut left = Collider2d::circle(1.0);
        left.layers = CollisionLayers::new(1, 1);
        let mut right = Collider2d::circle(1.0);
        right.layers = CollisionLayers::new(2, 2);
        world
            .insert_body(first, RigidBody2d::default(), left)
            .unwrap();
        world
            .insert_static_collider(second, PhysicsPose2d::default(), right)
            .unwrap();

        assert!(world.step(STEP).unwrap().is_empty());
    }

    #[test]
    fn a_removed_entity_cannot_leave_a_reused_physics_record() {
        let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
        let old = EntityId::from_bits(7_u64 << 32);
        let reused = EntityId::from_bits((7_u64 << 32) | 1);
        world
            .insert_body(old, RigidBody2d::default(), Collider2d::circle(0.5))
            .unwrap();
        assert!(world.remove(old));
        assert!(!world.contains(old));
        world
            .insert_body(reused, RigidBody2d::default(), Collider2d::circle(0.5))
            .unwrap();
        assert!(world.contains(reused));
        assert!(!world.contains(old));
    }

    #[test]
    fn invalid_dimensions_are_rejected_before_reaching_the_backend() {
        let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
        let bad = Collider2d::rectangle([0.0, 1.0]);
        assert_eq!(
            world.insert_body(entity(1), RigidBody2d::default(), bad),
            Err(PhysicsError::NonPositive("box_half_extent_x"))
        );
    }

    #[test]
    fn dynamic_only_operations_are_checked_at_the_sindri_boundary() {
        let mut world = PhysicsWorld2d::new([0.0, 0.0]).unwrap();
        let wall = entity(1);
        world
            .insert_static_collider(
                wall,
                PhysicsPose2d::default(),
                Collider2d::rectangle([1.0, 1.0]),
            )
            .unwrap();
        assert_eq!(
            world.apply_impulse(wall, [1.0, 0.0]),
            Err(PhysicsError::WrongBodyKind(
                wall,
                "apply impulse",
                RigidBodyKind::Static,
            ))
        );
    }

    #[test]
    fn the_3d_contract_is_sindri_owned_even_before_the_3d_runtime_slice() {
        let body = RigidBody3d::default();
        let collider = Collider3d {
            shape: ColliderShape3d::Sphere { radius: 0.5 },
            offset: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sensor: false,
            layers: CollisionLayers::ALL,
            friction: 0.5,
            restitution: 0.0,
        };
        let json = serde_json::to_string(&(body, collider)).unwrap();
        assert!(json.contains("sphere"));
        assert!(!json.contains("rapier"));
    }

    #[test]
    fn rapier3d_compiles_behind_the_private_boundary() {
        let _backend = rapier3d::prelude::PhysicsWorld::new();
    }
}
