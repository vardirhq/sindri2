//! Fixtures every editor test group builds on.
//!
//! The demo scene is embedded in the example, so a test reaches it without
//! depending on the working directory. Only the tests want it: the editor
//! itself no longer loads through the example's scene type.

use sindri_core::{SceneDocument, SceneEntity, SceneEntityId, World};
use sindri_cube::DemoScene;
use sindri_scene::SceneExtractor;

use super::super::scene_io::load_world;

pub(super) fn extractor() -> SceneExtractor {
    SceneExtractor::new().unwrap()
}

/// The scene the editor opens with no argument, loaded the way the editor
/// loads it.
pub(super) fn demo_world() -> World {
    load_world(&extractor(), &DemoScene::authored_document().unwrap())
        .expect("the demo scene loads")
}

pub(super) fn nested_scene() -> SceneDocument {
    let mut torso = SceneEntity::new(SceneEntityId::new("torso").unwrap());
    torso.parent = Some(SceneEntityId::new("root").unwrap());
    let mut arm = SceneEntity::new(SceneEntityId::new("arm").unwrap());
    arm.parent = Some(SceneEntityId::new("torso").unwrap());
    let mut leg = SceneEntity::new(SceneEntityId::new("leg").unwrap());
    leg.parent = Some(SceneEntityId::new("root").unwrap());

    SceneDocument {
        entities: vec![
            SceneEntity::new(SceneEntityId::new("root").unwrap()),
            torso,
            arm,
            leg,
        ],
        ..SceneDocument::default()
    }
}
