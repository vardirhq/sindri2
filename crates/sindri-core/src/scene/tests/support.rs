//! The entities the scene tests build documents from.

use crate::{SceneEntity, SceneEntityId};

pub(super) fn entity(id: &str, parent: Option<&str>) -> SceneEntity {
    SceneEntity {
        parent: parent.map(|value| SceneEntityId::new(value).unwrap()),
        ..SceneEntity::new(SceneEntityId::new(id).unwrap())
    }
}
