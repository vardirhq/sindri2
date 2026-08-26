//! What a document must be before it is accepted.

use crate::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, SceneError, SceneJsonError,
    Transform3D,
};

use super::support::entity;

#[test]
fn rejects_hierarchy_cycles() {
    let scene = SceneDocument {
        entities: vec![entity("a", Some("b")), entity("b", Some("a"))],
        ..SceneDocument::default()
    };
    assert!(matches!(
        scene.validate(),
        Err(SceneError::HierarchyCycle(_))
    ));
}

/// Entities sharing one ancestor chain are the case memoisation exists for:
/// the chain must be walked once, and still be correct for every entity.
#[test]
fn a_long_shared_chain_validates() {
    let mut entities = vec![entity("root", None)];
    for index in 1..2_000 {
        entities.push(entity(
            &format!("node-{index}"),
            Some(&format!("node-{}", index - 1)),
        ));
    }
    // The second entity's parent is the root rather than "node-0".
    entities[1].parent = Some(SceneEntityId::new("root").unwrap());

    let scene = SceneDocument {
        entities,
        ..SceneDocument::default()
    };
    assert_eq!(scene.validate(), Ok(()));
}

/// Remembering that an entity reaches a root must never let a cycle pass:
/// nothing on a path that loops is ever recorded as grounded.
#[test]
fn a_cycle_behind_a_long_chain_is_still_caught() {
    let mut entities = vec![entity("a", Some("b")), entity("b", Some("a"))];
    for index in 0..500 {
        let parent = if index == 0 {
            "a".to_owned()
        } else {
            format!("tail-{}", index - 1)
        };
        entities.push(entity(&format!("tail-{index}"), Some(&parent)));
    }

    let scene = SceneDocument {
        entities,
        ..SceneDocument::default()
    };
    assert!(matches!(
        scene.validate(),
        Err(SceneError::HierarchyCycle(_))
    ));
}

#[test]
fn rejects_non_finite_transforms() {
    let scene = SceneDocument {
        entities: vec![SceneEntity {
            transform_3d: Some(Transform3D {
                position: [f32::NAN, 0.0, 0.0],
                ..Transform3D::default()
            }),
            ..entity("drifting", None)
        }],
        ..SceneDocument::default()
    };
    assert_eq!(
        scene.validate(),
        Err(SceneError::NonFiniteTransform(
            SceneEntityId::new("drifting").unwrap()
        ))
    );
}

#[test]
fn unknown_document_versions_are_rejected() {
    let json = r#"{"format_version": 99, "entities": []}"#;
    assert!(matches!(
        SceneDocument::from_json(json),
        Err(SceneJsonError::Invalid(SceneError::UnsupportedVersion {
            found: 99,
            supported: SCENE_FORMAT_VERSION,
        }))
    ));
}
