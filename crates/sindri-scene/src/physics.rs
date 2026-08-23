use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_physics::{Collider2d, RigidBody2d};

/// An authored 2D rigid body.
///
/// The scene owns only Sindri's public physics model. Rapier remains private to
/// `sindri-physics`, so serialized projects cannot acquire backend handles or
/// backend-specific configuration by accident.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct RigidBody2dComponent(pub RigidBody2d);

impl SceneComponent for RigidBody2dComponent {
    const TYPE_NAME: &'static str = "sindri.rigid_body_2d";
}

/// An authored 2D collider.
///
/// Kept separate from the rigid body because static collision geometry does not
/// require a body component, matching the physics boundary in `docs/physics.md`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Collider2dComponent(pub Collider2d);

impl SceneComponent for Collider2dComponent {
    const TYPE_NAME: &'static str = "sindri.collider_2d";
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sindri_core::SceneComponent;
    use sindri_physics::{ColliderShape2d, RigidBodyKind};

    use super::{Collider2dComponent, RigidBody2dComponent};

    #[test]
    fn rigid_body_scene_data_is_the_sindri_physics_model() {
        let body: RigidBody2dComponent = serde_json::from_value(json!({
            "kind": "dynamic",
            "pose": { "position": [2.0, 3.0], "rotation": 0.25 },
            "linear_velocity": [4.0, 5.0],
            "angular_velocity": 0.5,
            "gravity_scale": 1.0,
            "linear_damping": 0.1,
            "angular_damping": 0.2,
            "lock_rotation": false
        }))
        .unwrap();

        assert_eq!(RigidBody2dComponent::TYPE_NAME, "sindri.rigid_body_2d");
        assert_eq!(body.0.kind, RigidBodyKind::Dynamic);
        assert!((body.0.pose.position[0] - 2.0).abs() < f32::EPSILON);
        assert!((body.0.pose.position[1] - 3.0).abs() < f32::EPSILON);
        assert!((body.0.linear_velocity[0] - 4.0).abs() < f32::EPSILON);
        assert!((body.0.linear_velocity[1] - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn collider_scene_data_carries_sindri_shapes_and_layers() {
        let collider: Collider2dComponent = serde_json::from_value(json!({
            "shape": { "shape": "box", "half_extents": [0.5, 1.0] },
            "offset": [0.0, 0.25],
            "rotation": 0.0,
            "sensor": true,
            "layers": { "memberships": 2, "filter": 5 },
            "friction": 0.6,
            "restitution": 0.1
        }))
        .unwrap();

        assert_eq!(Collider2dComponent::TYPE_NAME, "sindri.collider_2d");
        assert!(collider.0.sensor);
        assert_eq!(collider.0.layers.memberships, 2);
        assert_eq!(collider.0.layers.filter, 5);
        let ColliderShape2d::Box { half_extents } = collider.0.shape else {
            panic!("expected a box collider");
        };
        assert!((half_extents[0] - 0.5).abs() < f32::EPSILON);
        assert!((half_extents[1] - 1.0).abs() < f32::EPSILON);
    }
}
