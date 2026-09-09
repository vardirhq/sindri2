use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_physics::{Collider2d, RigidBody2d};

/// What drives a body, re-exported because a scene stores the name and the
/// editor offers the choice: both need the engine's own list rather than a
/// second copy of it.
pub use sindri_physics::RigidBodyKind;

/// An authored 2D rigid body.
///
/// The scene owns only Sindri's public physics model. Rapier remains private to
/// `sindri-physics`, so serialized projects cannot acquire backend handles or
/// backend-specific configuration by accident.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct RigidBody2dComponent(pub RigidBody2d);

impl SceneComponent for RigidBody2dComponent {
    const TYPE_NAME: &'static str = "sindri.physics2d.rigid_body";
}

/// An authored 2D collider, in one or more pieces.
///
/// Kept separate from the rigid body because static collision geometry does not
/// require a body component, matching the physics boundary in `docs/physics.md`.
///
/// Several pieces because one shape is often a poor description of a thing: a
/// character is a capsule with a circle at each side, a ship a box and two
/// pods. They belong to the one entity and move as one object — a compound is
/// one collider made of parts, not several colliders, and needs no child
/// entities to say so, because a piece already carries its own offset and
/// rotation.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(from = "AuthoredCollider2d")]
pub struct Collider2dComponent(pub Vec<Collider2d>);

/// One collider or a list of them.
///
/// The single form is what every project authored before compounds existed, and
/// it still means what it always did. Kept rather than migrated: a scene is a
/// file someone wrote, and a format that can only be read after a rewrite is a
/// format that breaks their project on upgrade.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum AuthoredCollider2d {
    /// `{ "pieces": [ … ] }` — the form a compound is written in, and what the
    /// editor now creates.
    Compound { pieces: Vec<Collider2d> },
    /// `{ "shape": … }` — one collider, as before.
    Single(Collider2d),
}

impl From<AuthoredCollider2d> for Collider2dComponent {
    fn from(authored: AuthoredCollider2d) -> Self {
        Self(match authored {
            AuthoredCollider2d::Compound { pieces } => pieces,
            AuthoredCollider2d::Single(collider) => vec![collider],
        })
    }
}

impl SceneComponent for Collider2dComponent {
    const TYPE_NAME: &'static str = "sindri.physics2d.collider";
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

        assert_eq!(
            RigidBody2dComponent::TYPE_NAME,
            "sindri.physics2d.rigid_body"
        );
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

        assert_eq!(Collider2dComponent::TYPE_NAME, "sindri.physics2d.collider");
        let [piece] = &collider.0[..] else {
            panic!("one authored collider is one piece")
        };
        assert!(piece.sensor);
        assert_eq!(piece.layers.memberships, 2);
        assert_eq!(piece.layers.filter, 5);
        let ColliderShape2d::Box { half_extents } = piece.shape else {
            panic!("expected a box collider");
        };
        assert!((half_extents[0] - 0.5).abs() < f32::EPSILON);
        assert!((half_extents[1] - 1.0).abs() < f32::EPSILON);
    }

    /// A compound is a list of pieces, each with its own offset — which is what
    /// lets one entity carry the whole shape without child entities.
    #[test]
    fn a_collider_can_be_authored_in_several_pieces() {
        let collider: Collider2dComponent = serde_json::from_value(json!({
            "pieces": [
                { "shape": { "shape": "capsule", "half_height": 0.4, "radius": 0.22 },
                  "offset": [0.0, 0.05], "rotation": 0.0, "sensor": false,
                  "layers": { "memberships": 1, "filter": 1 },
                  "friction": 0.5, "restitution": 0.0 },
                { "shape": { "shape": "circle", "radius": 0.18 },
                  "offset": [-0.38, -0.1], "rotation": 0.0, "sensor": false,
                  "layers": { "memberships": 1, "filter": 1 },
                  "friction": 0.5, "restitution": 0.0 }
            ]
        }))
        .unwrap();

        assert_eq!(collider.0.len(), 2);
        assert!((collider.0[1].offset[0] + 0.38).abs() < f32::EPSILON);
        assert!(matches!(
            collider.0[0].shape,
            ColliderShape2d::Capsule { .. }
        ));
    }

    /// The single form keeps meaning what it always did.
    ///
    /// A scene is a file someone wrote. A format that can only be read after a
    /// rewrite is one that breaks their project on upgrade, so the old spelling
    /// stays readable rather than being migrated.
    #[test]
    fn one_authored_collider_is_still_one_authored_collider() {
        let single = json!({
            "shape": { "shape": "circle", "radius": 0.5 },
            "offset": [0.0, 0.0], "rotation": 0.0, "sensor": false,
            "layers": { "memberships": 1, "filter": 1 },
            "friction": 0.5, "restitution": 0.0
        });
        let scalar: Collider2dComponent = serde_json::from_value(single.clone()).unwrap();
        let listed: Collider2dComponent =
            serde_json::from_value(json!({ "pieces": [single] })).unwrap();

        assert_eq!(scalar.0.len(), 1);
        assert_eq!(scalar, listed, "the two spellings must mean the same thing");
    }
}
