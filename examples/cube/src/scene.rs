use glam::{Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;
use sindri_core::{EntityData, SceneDocument, Transform2D, Transform3D, World, WorldError};
use sindri_render::{
    ClearOperations, ExtractedFrame, FrameCamera, FrameCommand, FramePass, FramePlanError,
    OrthographicCamera, PerspectiveCamera, PreparedFrame, RenderLayer, RenderStage, SpriteInstance,
    TransparentOrder, TransparentOrderError, Viewport,
};
use thiserror::Error;

const CAMERA_COMPONENT: &str = "sindri.camera";
const MESH_COMPONENT: &str = "sindri.mesh";
const SPRITE_COMPONENT: &str = "sindri.sprite";
const SCENE_JSON: &str = include_str!("../assets/demo.scene.json");

#[derive(Debug)]
pub struct DemoScene {
    world: World,
}

impl DemoScene {
    pub fn load() -> Result<Self, DemoSceneError> {
        let document: SceneDocument = serde_json::from_str(SCENE_JSON)?;
        let loaded = World::from_scene(&document)?;
        Ok(Self {
            world: loaded.world,
        })
    }

    pub fn extract_frame(
        &self,
        viewport: Viewport,
        rotation: Vec2,
    ) -> Result<PreparedFrame, DemoSceneError> {
        let aspect = viewport.aspect_ratio()?;
        let cameras = self.extract_cameras(aspect)?;
        let (cube_model, cube_layer) = self.extract_cube(rotation)?;
        let (sprite_layer, sprites) = self.extract_sprites(aspect)?;

        let mut frame = ExtractedFrame::new(viewport, ClearOperations::default());
        frame.push(FramePass::new(
            RenderStage::Opaque3d,
            RenderLayer(cube_layer),
            FrameCamera {
                view_projection: cameras.perspective,
            },
            FrameCommand::TexturedCube { model: cube_model },
        ));
        frame.push(FramePass::new(
            RenderStage::Overlay,
            RenderLayer(sprite_layer),
            FrameCamera {
                view_projection: cameras.orthographic,
            },
            FrameCommand::SpriteBatch { instances: sprites },
        ));
        Ok(frame.prepare()?)
    }

    fn extract_cameras(&self, aspect: f32) -> Result<CameraMatrices, DemoSceneError> {
        let mut cameras = CameraMatrices::default();
        for (_, entity) in self.world.entities() {
            let Some(value) = entity.components.get(CAMERA_COMPONENT) else {
                continue;
            };
            match decode_component::<CameraComponent>(entity, CAMERA_COMPONENT, value)? {
                CameraComponent::Perspective {
                    target,
                    up,
                    vertical_fov_degrees,
                    near,
                    far,
                } => {
                    let eye = entity.transform_3d.unwrap_or_default().position;
                    cameras.perspective = Some(
                        PerspectiveCamera {
                            eye: Vec3::from_array(eye),
                            target: Vec3::from_array(target),
                            up: Vec3::from_array(up),
                            vertical_fov_radians: vertical_fov_degrees.to_radians(),
                            near,
                            far,
                        }
                        .view_projection(aspect),
                    );
                }
                CameraComponent::Orthographic {
                    center,
                    vertical_size,
                    near,
                    far,
                } => {
                    cameras.orthographic = Some(
                        OrthographicCamera {
                            center: Vec2::from_array(center),
                            vertical_size,
                            near,
                            far,
                        }
                        .view_projection(aspect),
                    );
                }
            }
        }
        cameras.require_complete()
    }

    fn extract_cube(&self, rotation: Vec2) -> Result<(Mat4, i32), DemoSceneError> {
        for (_, entity) in self.world.entities() {
            let Some(value) = entity.components.get(MESH_COMPONENT) else {
                continue;
            };
            let mesh = decode_component::<MeshComponent>(entity, MESH_COMPONENT, value)?;
            if mesh.primitive == MeshPrimitive::Cube {
                let authored = transform_3d_matrix(entity.transform_3d.unwrap_or_default());
                let animated =
                    Mat4::from_rotation_y(rotation.x) * Mat4::from_rotation_x(rotation.y);
                return Ok((authored * animated, mesh.layer));
            }
        }
        Err(DemoSceneError::Missing("cube mesh"))
    }

    fn extract_sprites(&self, aspect: f32) -> Result<(i32, Vec<SpriteInstance>), DemoSceneError> {
        let mut extracted = Vec::new();
        let mut shared_layer = None;
        for (entity_id, entity) in self.world.entities() {
            let Some(value) = entity.components.get(SPRITE_COMPONENT) else {
                continue;
            };
            let sprite = decode_component::<SpriteComponent>(entity, SPRITE_COMPONENT, value)?;
            let transform = entity.transform_2d.unwrap_or_default();
            let model = transform_2d_matrix(transform, sprite.anchor, aspect);
            let order =
                TransparentOrder::new(sprite.layer, sprite.depth, u64::from(entity_id.index()))?;
            if shared_layer
                .replace(sprite.layer)
                .is_some_and(|layer| layer != sprite.layer)
            {
                return Err(DemoSceneError::MixedSpriteLayers);
            }
            extracted.push((order, SpriteInstance::new(model, sprite.tint)));
        }
        extracted.sort_by_key(|(order, _)| *order);
        let layer = shared_layer.ok_or(DemoSceneError::Missing("sprite overlay"))?;
        Ok((
            layer,
            extracted.into_iter().map(|(_, sprite)| sprite).collect(),
        ))
    }
}

#[derive(Default)]
struct CameraMatrices {
    perspective: Option<Mat4>,
    orthographic: Option<Mat4>,
}

impl CameraMatrices {
    fn require_complete(self) -> Result<CompleteCameraMatrices, DemoSceneError> {
        Ok(CompleteCameraMatrices {
            perspective: self
                .perspective
                .ok_or(DemoSceneError::Missing("perspective camera"))?,
            orthographic: self
                .orthographic
                .ok_or(DemoSceneError::Missing("orthographic camera"))?,
        })
    }
}

struct CompleteCameraMatrices {
    perspective: Mat4,
    orthographic: Mat4,
}

#[derive(Deserialize)]
#[serde(tag = "projection", rename_all = "snake_case")]
enum CameraComponent {
    Perspective {
        target: [f32; 3],
        up: [f32; 3],
        vertical_fov_degrees: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        center: [f32; 2],
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MeshPrimitive {
    Cube,
}

#[derive(Deserialize)]
struct MeshComponent {
    primitive: MeshPrimitive,
    #[serde(rename = "texture")]
    _texture: String,
    layer: i32,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SpriteAnchor {
    BottomRight,
}

#[derive(Deserialize)]
struct SpriteComponent {
    #[serde(rename = "texture")]
    _texture: String,
    anchor: SpriteAnchor,
    tint: [f32; 4],
    depth: f32,
    layer: i32,
}

fn decode_component<T: for<'de> Deserialize<'de>>(
    entity: &EntityData,
    component: &'static str,
    value: &serde_json::Value,
) -> Result<T, DemoSceneError> {
    serde_json::from_value(value.clone()).map_err(|source| DemoSceneError::InvalidComponent {
        entity: entity
            .source_id
            .as_ref()
            .map_or_else(|| "<runtime>".to_owned(), |id| id.as_str().to_owned()),
        component,
        source,
    })
}

fn transform_3d_matrix(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation),
        Vec3::from_array(transform.position),
    )
}

fn transform_2d_matrix(transform: Transform2D, anchor: SpriteAnchor, aspect: f32) -> Mat4 {
    let anchor_position = match anchor {
        SpriteAnchor::BottomRight => Vec2::new(aspect - 0.16, 0.0),
    };
    let position = anchor_position + Vec2::from_array(transform.position);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_rotation_z(transform.rotation_radians)
        * Mat4::from_scale(Vec3::new(transform.scale[0], transform.scale[1], 1.0))
}

#[derive(Debug, Error)]
pub enum DemoSceneError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    World(#[from] WorldError),
    #[error(transparent)]
    Frame(#[from] FramePlanError),
    #[error(transparent)]
    TransparentOrder(#[from] TransparentOrderError),
    #[error("entity '{entity}' has invalid {component} data")]
    InvalidComponent {
        entity: String,
        component: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("demo scene is missing its {0}")]
    Missing(&'static str),
    #[error("the current sprite batch requires all sprites to share a render layer")]
    MixedSpriteLayers,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_scene_extracts_ordered_opaque_and_overlay_passes() {
        let scene = DemoScene::load().unwrap();
        let frame = scene
            .extract_frame(Viewport::new(512, 512), Vec2::ZERO)
            .unwrap();
        assert_eq!(frame.passes().len(), 2);
        assert_eq!(frame.passes()[0].stage, RenderStage::Opaque3d);
        assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
        let FrameCommand::SpriteBatch { instances } = &frame.passes()[1].command else {
            panic!("second pass should contain the sprite batch");
        };
        assert_eq!(instances.len(), 5);
    }
}
