use glam::{Quat, Vec2};
use sindri_core::{SceneDocument, SceneJsonError, UnknownComponentPolicy, World, WorldError};
use sindri_render::{PreparedFrame, Viewport};
use sindri_scene::{CameraView, SceneExtractError, SceneExtractor, TextureBindings};
use thiserror::Error;

const SCENE_JSON: &str = include_str!("../assets/demo.scene.json");

/// The demo's component schemas, and the extraction they drive.
///
/// It deliberately owns no world. A world belongs to whoever is running the
/// engine — `EngineCore` at runtime, the editor while authoring — and a scene
/// that kept its own copy would mean two of them, with only one of them the one
/// gameplay writes.
#[derive(Debug)]
pub struct DemoScene {
    extractor: SceneExtractor,
}

impl DemoScene {
    /// The schemas alone, for a host that already has a world.
    pub fn new() -> Result<Self, DemoSceneError> {
        Ok(Self {
            extractor: SceneExtractor::new()?,
        })
    }

    /// The schemas and the authored world together, which is what a host that
    /// is starting from the demo scene wants.
    pub fn load() -> Result<(Self, World), DemoSceneError> {
        let scene = Self::new()?;
        let world = scene.load_world(&Self::authored_document()?)?;
        Ok((scene, world))
    }

    /// The scene exactly as authored on disk, used to reset edited state.
    pub fn authored_document() -> Result<SceneDocument, DemoSceneError> {
        Ok(SceneDocument::from_json(SCENE_JSON)?)
    }

    /// Builds a runtime world from any document the built-in components accept.
    pub fn load_world(&self, document: &SceneDocument) -> Result<World, DemoSceneError> {
        self.extractor
            .validate(document, UnknownComponentPolicy::Reject)?;
        Ok(World::from_scene(document)?.world)
    }

    /// Extracts a world through the authored camera.
    pub fn extract_frame(
        &self,
        world: &World,
        viewport: Viewport,
        textures: &TextureBindings,
    ) -> Result<PreparedFrame, DemoSceneError> {
        self.extract(world, viewport, CameraView::default(), textures)
    }

    /// Extracts a world through a viewer-adjusted camera.
    pub fn extract(
        &self,
        world: &World,
        viewport: Viewport,
        view: CameraView,
        textures: &TextureBindings,
    ) -> Result<PreparedFrame, DemoSceneError> {
        Ok(self.extractor.extract(world, viewport, view, textures)?)
    }
}

/// Turns the cube by writing its transform, the way gameplay does.
///
/// Nothing downstream knows this happened: extraction simply reads whatever the
/// world now holds.
pub fn spin_cube(world: &mut World, rotation: Vec2) -> Result<(), DemoSceneError> {
    let cube = world
        .entities()
        .find(|(_, data)| data.components.contains_key("sindri.mesh"))
        .map(|(entity, _)| entity)
        .ok_or(DemoSceneError::Missing("cube mesh"))?;
    let data = world
        .get_mut(cube)
        .ok_or(DemoSceneError::Missing("cube mesh"))?;
    let mut transform = data.transform_3d.unwrap_or_default();
    transform.rotation =
        (Quat::from_rotation_y(rotation.x) * Quat::from_rotation_x(rotation.y)).to_array();
    data.transform_3d = Some(transform);
    Ok(())
}

#[derive(Debug, Error)]
pub enum DemoSceneError {
    #[error(transparent)]
    Scene(#[from] SceneJsonError),
    #[error(transparent)]
    Extract(#[from] SceneExtractError),
    #[error(transparent)]
    World(#[from] WorldError),
    #[error("demo scene is missing its {0}")]
    Missing(&'static str),
}

#[cfg(test)]
mod tests {
    use sindri_render::{FrameCommand, RenderStage};

    use super::*;

    const VIEWPORT: Viewport = Viewport::new(512, 512);

    /// The demo's texture references, bound to distinct handles so extraction
    /// batches per texture the way the real host does.
    fn bindings() -> TextureBindings {
        let mut bindings = TextureBindings::new();
        bindings.bind("procedural:checkerboard", sindri_render::TextureId::new(1));
        bindings.bind("textures/badge.png", sindri_render::TextureId::new(2));
        bindings
    }

    /// The shipped scene asset is stored in canonical form. Regenerate it with
    /// `SINDRI_UPDATE_SCENE_FIXTURES=1 cargo test --package sindri-cube`.
    #[test]
    fn embedded_scene_is_canonical_and_round_trips_through_a_world() {
        let document = SceneDocument::from_json(SCENE_JSON).unwrap();
        let canonical = document.to_canonical_json().unwrap();

        if std::env::var_os("SINDRI_UPDATE_SCENE_FIXTURES").is_some() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join("demo.scene.json");
            std::fs::write(path, &canonical).unwrap();
            return;
        }

        assert_eq!(canonical, SCENE_JSON);
        let loaded = World::from_scene(&document).unwrap();
        assert_eq!(loaded.world.to_scene().unwrap(), document);
    }

    #[test]
    fn the_embedded_scene_extracts_an_opaque_pass_and_an_overlay_batch() {
        let (scene, world) = DemoScene::load().unwrap();
        let frame = scene.extract_frame(&world, VIEWPORT, &bindings()).unwrap();

        assert_eq!(frame.passes().len(), 2);
        assert_eq!(frame.passes()[0].stage, RenderStage::Opaque3d);
        assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[1].command else {
            panic!("the overlay pass should be a sprite batch");
        };
        assert_eq!(instances.len(), 5);
    }

    /// Neither canonical entity ordering, generalised anchoring, nor sorting by
    /// distance rather than an authored number may disturb the stack these
    /// badges were drawn in. The alphas are the order: back to front.
    #[test]
    fn the_overlay_keeps_its_authored_back_to_front_order() {
        let (scene, world) = DemoScene::load().unwrap();
        let frame = scene.extract_frame(&world, VIEWPORT, &bindings()).unwrap();
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[1].command else {
            panic!("the overlay pass should be a sprite batch");
        };
        let alphas: Vec<f32> = instances
            .iter()
            .map(|instance| instance.tint()[3])
            .collect();
        assert_eq!(alphas, [0.62, 0.68, 0.72, 0.78, 0.88]);
    }

    /// Retuning the badges for the shared anchor must leave them where they were.
    #[test]
    fn the_overlay_badges_sit_where_they_were_authored() {
        let (scene, world) = DemoScene::load().unwrap();
        let frame = scene.extract_frame(&world, VIEWPORT, &bindings()).unwrap();
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[1].command else {
            panic!("the overlay pass should be a sprite batch");
        };
        let positions: Vec<(f32, f32)> = instances
            .iter()
            .map(|instance| {
                let translation = instance.model().w_axis;
                (
                    (translation.x * 100.0).round(),
                    (translation.y * 100.0).round(),
                )
            })
            .collect();
        // The values the hand-written extraction produced at this aspect.
        assert_eq!(
            positions,
            [
                (22.0, -56.0),
                (40.0, -68.0),
                (56.0, -48.0),
                (72.0, -66.0),
                (82.0, -42.0),
            ]
        );
    }

    #[test]
    fn spinning_the_cube_writes_the_world_and_changes_the_frame() {
        let (scene, mut world) = DemoScene::load().unwrap();
        let resting = scene.extract_frame(&world, VIEWPORT, &bindings()).unwrap();
        spin_cube(&mut world, Vec2::new(0.8, 0.3)).unwrap();
        let spun = scene.extract_frame(&world, VIEWPORT, &bindings()).unwrap();

        let (
            FrameCommand::TexturedCube { model: before, .. },
            FrameCommand::TexturedCube { model: after, .. },
        ) = (&resting.passes()[0].command, &spun.passes()[0].command)
        else {
            panic!("the opaque pass should draw the cube");
        };
        assert_ne!(before, after);

        // The rotation lives in the scene now, so it survives a save.
        let saved = world.to_scene().unwrap();
        let cube = saved
            .entity(&sindri_core::SceneEntityId::new("checker-cube").unwrap())
            .expect("the cube is still in the scene");
        let rotation = cube.transform_3d.unwrap().rotation;
        assert!(
            rotation[3] < 0.999,
            "spinning should have written the cube transform, got {rotation:?}"
        );
    }

    #[test]
    fn an_editor_view_moves_the_camera_without_moving_the_model() {
        let (scene, world) = DemoScene::load().unwrap();
        let authored = scene.extract_frame(&world, VIEWPORT, &bindings()).unwrap();
        let orbited = scene
            .extract(
                &world,
                VIEWPORT,
                CameraView {
                    orbit: Vec2::new(0.5, 0.25),
                    ..CameraView::default()
                },
                &bindings(),
            )
            .unwrap();

        assert_ne!(
            authored.passes()[0].camera.view_projection,
            orbited.passes()[0].camera.view_projection
        );
        let (
            FrameCommand::TexturedCube { model: before, .. },
            FrameCommand::TexturedCube { model: after, .. },
        ) = (&authored.passes()[0].command, &orbited.passes()[0].command)
        else {
            panic!("the opaque pass should draw the cube");
        };
        assert_eq!(before, after);
    }
}
