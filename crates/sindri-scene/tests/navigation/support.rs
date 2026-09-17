//! Building the small worlds these tests derive navigation from.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sindri_core::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, Transform3D, World,
};

pub(crate) fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("test IDs are non-empty")
}

pub(crate) fn entity(
    entity_id: &str,
    position: Option<[f32; 3]>,
    components: impl IntoIterator<Item = (&'static str, Value)>,
) -> SceneEntity {
    let mut entity = SceneEntity::new(id(entity_id));
    entity.transform_3d = position.map(|position| Transform3D {
        position,
        ..Transform3D::default()
    });
    entity.components = components
        .into_iter()
        .map(|(name, payload)| (name.to_owned(), payload))
        .collect::<BTreeMap<_, _>>();
    entity
}

pub(crate) fn tilemap(columns: u32, rows: u32) -> Value {
    json!({
        "texture": "tiles",
        "palette": [],
        "columns": columns,
        "rows": rows,
        "tiles": vec![Value::Null; (columns * rows) as usize],
        "space": "world"
    })
}

pub(crate) fn world(entities: Vec<SceneEntity>) -> sindri_core::LoadedScene {
    World::from_scene(&SceneDocument {
        format_version: SCENE_FORMAT_VERSION,
        entities,
        ..SceneDocument::default()
    })
    .expect("test scene loads")
}
