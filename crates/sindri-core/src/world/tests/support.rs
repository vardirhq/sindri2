//! The authored scene the round-trip tests start from.

use std::collections::BTreeMap;

use serde_json::json;

use crate::{SceneDocument, SceneEntity, SceneEntityId, SceneMetadata, Transform3D};

pub(super) fn authored_scene() -> SceneDocument {
    let mut root = SceneEntity::new(SceneEntityId::new("root").unwrap());
    root.name = Some("Root".into());
    root.transform_3d = Some(Transform3D::default());
    root.components = BTreeMap::from([("game.marker".to_owned(), json!({ "kind": "spawn" }))]);
    root.editor = BTreeMap::from([("collapsed".to_owned(), json!(false))]);

    let mut child = SceneEntity::new(SceneEntityId::new("child").unwrap());
    child.parent = Some(SceneEntityId::new("root").unwrap());
    child.transform_3d = Some(Transform3D {
        position: [1.5, -2.25, 0.0],
        ..Transform3D::default()
    });

    SceneDocument {
        metadata: SceneMetadata {
            name: Some("Round trip".into()),
            editor: BTreeMap::from([("grid_snap".to_owned(), json!(0.25))]),
        },
        // Deliberately unsorted so the save has to canonicalize.
        entities: vec![root, child],
        ..SceneDocument::default()
    }
}
