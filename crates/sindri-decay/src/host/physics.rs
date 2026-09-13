//! Performing a `Physics.*` call.
//!
//! Sindri physics, never Rapier. A host with no physics answers every one of
//! these by saying so, rather than reporting a velocity of zero for a body that
//! does not exist — a game whose bullets never move because nothing is stepping
//! should hear about it on the first frame.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::EntityId;
use sindri_physics::PhysicsEventKind;

use crate::surface::PhysicsCall;

/// The component that says an entity takes part in physics.
///
/// Spelled here because this is the engine, not a script: `docs/scripting.md`
/// keeps component names out of Decay, and a host asking what an entity is made
/// of is the reason it can.
const COLLIDER: &str = "sindri.physics2d.collider";

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    pub(super) fn physics_call(
        &mut self,
        call: PhysicsCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        if call.is_event() {
            return self.physics_events(call, path);
        }
        if matches!(call, PhysicsCall::ConnectDistance) {
            return self.connect_distance(path, args);
        }
        let entity = self.entity_argument(path, args, 0, "the body")?;
        // Whether the entity authored physics at all, asked before the physics
        // world is borrowed. A body that is authored but not yet built is the
        // ordinary case on the frame a prefab was spawned, and it is the only
        // case a set is allowed to arrive early for.
        let authored = self
            .world
            .get(entity)
            .is_some_and(|data| data.components.contains_key(COLLIDER));
        let Some(physics) = self.physics.as_mut() else {
            return Err(no_physics(path));
        };
        match call {
            PhysicsCall::VelocityX | PhysicsCall::VelocityY => {
                let velocity = match physics.world.linear_velocity(entity) {
                    Ok(velocity) => velocity,
                    // A prefab becomes visible to other scripts as soon as it
                    // is spawned, but its physics body is materialized at the
                    // next synchronize. Until then its authored starting
                    // velocity is effectively zero unless a script has already
                    // queued another value, which `linear_velocity` returns via
                    // `pending_velocity`. Treating this short, valid lifecycle
                    // window as "no physics body" made systems such as gravity
                    // fields race freshly spawned projectiles.
                    Err(sindri_physics::PhysicsError::MissingEntity(_)) if authored => [0.0, 0.0],
                    Err(error) => return Err(body_error(path, &error)),
                };
                Ok(Value::Number(f64::from(
                    if matches!(call, PhysicsCall::VelocityX) {
                        velocity[0]
                    } else {
                        velocity[1]
                    },
                )))
            }
            PhysicsCall::SetVelocity | PhysicsCall::ApplyImpulse => {
                let x = number(path, args.get(1).unwrap_or(&Value::Null))?;
                let y = number(path, args.get(2).unwrap_or(&Value::Null))?;
                // Narrowed here for the reason every number crossing into the
                // engine is: Decay holds an `f64` and a transform is `f32`.
                #[allow(clippy::cast_possible_truncation)]
                let value = [x as f32, y as f32];
                let outcome = if matches!(call, PhysicsCall::SetVelocity) {
                    match physics.world.set_linear_velocity(entity, value) {
                        // Spawned this pass: the body is built when the scene
                        // next synchronizes, and this is what it starts with.
                        // Without this a bullet could not be aimed on the frame
                        // it was fired, which is the shape `docs/scripting.md`
                        // documents and the reason a spawned script starts in
                        // the pass that made it.
                        Err(sindri_physics::PhysicsError::MissingEntity(_)) if authored => {
                            physics.world.remember_linear_velocity(entity, value)
                        }
                        other => other,
                    }
                } else {
                    physics.world.apply_impulse(entity, value)
                };
                outcome.map_err(|error| body_error(path, &error))?;
                Ok(Value::Unit)
            }
            PhysicsCall::ConnectDistance => unreachable!("answered before the one-body path"),
            _ => unreachable!("event calls answered above"),
        }
    }

    fn connect_distance(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let first = self.entity_argument(path, args, 0, "the first body")?;
        let second = self.entity_argument(path, args, 1, "the second body")?;
        let distance = number(path, args.get(2).unwrap_or(&Value::Null))?;
        #[allow(clippy::cast_possible_truncation)]
        let distance = distance as f32;

        let first_authored = self
            .world
            .get(first)
            .is_some_and(|data| data.components.contains_key(COLLIDER));
        let second_authored = self
            .world
            .get(second)
            .is_some_and(|data| data.components.contains_key(COLLIDER));
        let Some(physics) = self.physics.as_mut() else {
            return Err(no_physics(path));
        };
        let outcome = match physics.world.connect_distance(first, second, distance) {
            // A chain is normally created in one script pass: all its entities
            // exist immediately, while their backend bodies appear at the next
            // scene synchronization. Queue exactly that valid lifecycle window.
            Err(sindri_physics::PhysicsError::MissingEntity(_))
                if first_authored && second_authored =>
            {
                physics
                    .world
                    .remember_distance_joint(first, second, distance)
            }
            other => other,
        };
        outcome.map_err(|error| body_error(path, &error))?;
        Ok(Value::Unit)
    }

    /// What this entity started or stopped touching during the last step.
    ///
    /// About the entity the script is on, because an event is about a pair and
    /// the pair a script cares about is the one it is half of. The answer names
    /// the *other* half.
    fn physics_events(&self, call: PhysicsCall, path: &Path) -> Result<Value, RuntimeError> {
        let Some(physics) = self.physics.as_ref() else {
            return Err(no_physics(path));
        };
        let wanted = match call {
            PhysicsCall::CollisionStarted => PhysicsEventKind::CollisionStarted,
            PhysicsCall::CollisionStopped => PhysicsEventKind::CollisionStopped,
            PhysicsCall::SensorEntered => PhysicsEventKind::SensorEntered,
            PhysicsCall::SensorExited => PhysicsEventKind::SensorExited,
            _ => unreachable!("only event calls reach here"),
        };
        let mine = self.entity;
        // In the order the step reported them, which is the order the backend
        // produced and is the same order twice for the same simulation.
        let others: Vec<Value> = physics
            .events
            .iter()
            .filter(|event| event.kind == wanted)
            .filter_map(|event| other_half(event.first, event.second, mine))
            .map(|entity| Value::Reference(entity.to_bits()))
            .collect();
        Ok(Value::array(others))
    }
}

/// The other entity in a pair, when this one is in it.
///
/// An event naming the same entity twice is a body touching itself, which the
/// backend does not produce and which would answer with a script's own entity
/// if it did.
fn other_half(first: EntityId, second: EntityId, mine: EntityId) -> Option<EntityId> {
    if first == mine {
        Some(second)
    } else if second == mine {
        Some(first)
    } else {
        None
    }
}

fn no_physics(path: &Path) -> RuntimeError {
    RuntimeError::Host(format!(
        "{} needs physics, and this host is not running any",
        path.dotted()
    ))
}

/// A physics failure, named as what a script did rather than as a backend
/// error.
///
/// The ordinary way to reach one is asking about an entity with no body: a
/// script holding a reference to something that has a sprite and no collider.
fn body_error(path: &Path, error: &sindri_physics::PhysicsError) -> RuntimeError {
    RuntimeError::Host(format!("{}: {error}", path.dotted()))
}
