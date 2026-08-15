use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec2, Vec3};
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, SceneComponent, SceneDocument, Transform2D,
    Transform3D, UnknownComponentPolicy, World,
};
use sindri_render::{
    ClearOperations, ExtractedFrame, FrameCamera, FrameCommand, FramePass, FramePlanError,
    OrthographicCamera, PerspectiveCamera, PreparedFrame, RenderLayer, RenderStage, SpriteInstance,
    TextureId, TransparentOrder, TransparentOrderError, Viewport,
};
use thiserror::Error;

use crate::{
    CameraComponent, MeshComponent, MeshPrimitive, SpriteAnchor, SpriteComponent, TextureBindings,
};

/// Which projection the world camera uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldProjection {
    #[default]
    Perspective,
    /// An orthographic projection framed to match the perspective camera, so
    /// toggling between them keeps the subject the same size.
    Orthographic,
}

/// A viewer's adjustment to the authored world camera.
///
/// Gameplay renders through the authored camera. An editor moves around it
/// without touching the scene, which is what this describes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    /// Yaw and pitch in radians, orbited around the camera's target.
    pub orbit: Vec2,
    /// Multiplier on the authored eye-to-target distance.
    pub distance_scale: f32,
    pub projection: WorldProjection,
}

impl Default for CameraView {
    fn default() -> Self {
        Self {
            orbit: Vec2::ZERO,
            distance_scale: 1.0,
            projection: WorldProjection::Perspective,
        }
    }
}

/// Turns a world into a frame the renderer can draw.
///
/// This is the seam between simulation and rendering: gameplay only ever writes
/// to the world, and everything drawn is derived from registered components. No
/// scene needs hand-written extraction code.
#[derive(Clone, Debug)]
pub struct SceneExtractor {
    components: ComponentSchemaRegistry,
}

impl SceneExtractor {
    /// Registers the built-in `sindri.*` components.
    pub fn new() -> Result<Self, SceneExtractError> {
        let mut components = ComponentSchemaRegistry::default();
        components.register::<CameraComponent>("Camera")?;
        components.register::<MeshComponent>("Mesh")?;
        components.register::<SpriteComponent>("Sprite")?;
        Ok(Self { components })
    }

    pub const fn components(&self) -> &ComponentSchemaRegistry {
        &self.components
    }

    /// Registers an additional component type a game brings of its own.
    pub fn register<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
    ) -> Result<(), SceneExtractError> {
        self.components.register::<T>(display_name)?;
        Ok(())
    }

    pub fn validate(
        &self,
        document: &SceneDocument,
        unknown: UnknownComponentPolicy,
    ) -> Result<(), SceneExtractError> {
        self.components.validate_scene(document, unknown)?;
        Ok(())
    }

    /// Extracts every drawable in `world` into an ordered frame.
    pub fn extract(
        &self,
        world: &World,
        viewport: Viewport,
        view: CameraView,
        textures: &TextureBindings,
    ) -> Result<PreparedFrame, SceneExtractError> {
        let aspect = viewport.aspect_ratio()?;
        let cameras = self.resolve_cameras(world, aspect, view)?;
        let mut frame = ExtractedFrame::new(viewport, ClearOperations::default());

        for (entity, mesh) in self.components.query::<MeshComponent>(world)? {
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let command = match mesh.primitive {
                MeshPrimitive::Cube => FrameCommand::TexturedCube {
                    model: transform_matrix(transform),
                    texture: textures.resolve(&mesh.texture),
                },
            };
            frame.push(FramePass::new(
                RenderStage::Opaque3d,
                RenderLayer(mesh.layer),
                FrameCamera {
                    view_projection: cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                },
                command,
            ));
        }

        // Sprites batch per layer, back to front, with a stable tie-break.
        // Keyed by layer and texture: a batch is one draw, so instances only
        // share one when they share a texture.
        let mut layers: BTreeMap<(i32, TextureId), Vec<(TransparentOrder, SpriteInstance)>> =
            BTreeMap::new();
        for (entity, sprite) in self.components.query::<SpriteComponent>(world)? {
            let extent = cameras
                .overlay_extent
                .ok_or(SceneExtractError::MissingOverlayCamera)?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_2d)
                .unwrap_or_default();
            let order = TransparentOrder::new(sprite.layer, sprite.depth, entity.index())?;
            layers
                .entry((sprite.layer, textures.resolve(&sprite.texture)))
                .or_default()
                .push((
                    order,
                    SpriteInstance::new(
                        sprite_matrix(transform, sprite.anchor, extent),
                        sprite.tint,
                    ),
                ));
        }

        for ((layer, texture), mut sprites) in layers {
            sprites.sort_by_key(|(order, _)| *order);
            frame.push(FramePass::new(
                RenderStage::Overlay,
                RenderLayer(layer),
                FrameCamera {
                    view_projection: cameras
                        .overlay
                        .ok_or(SceneExtractError::MissingOverlayCamera)?,
                },
                FrameCommand::SpriteBatch {
                    texture,
                    instances: sprites.into_iter().map(|(_, sprite)| sprite).collect(),
                },
            ));
        }

        Ok(frame.prepare()?)
    }

    fn resolve_cameras(
        &self,
        world: &World,
        aspect: f32,
        view: CameraView,
    ) -> Result<ResolvedCameras, SceneExtractError> {
        if !view.distance_scale.is_finite() || view.distance_scale <= 0.0 {
            return Err(SceneExtractError::InvalidCameraDistanceScale);
        }
        let mut resolved = ResolvedCameras::default();

        for (entity, camera) in self.components.query::<CameraComponent>(world)? {
            match camera {
                CameraComponent::Perspective {
                    target,
                    up,
                    vertical_fov_degrees,
                    near,
                    far,
                } => {
                    let authored_eye = Vec3::from_array(
                        world
                            .get(entity)
                            .and_then(|data| data.transform_3d)
                            .unwrap_or_default()
                            .position,
                    );
                    let target = Vec3::from_array(target);
                    let up = Vec3::from_array(up);
                    let offset = orbited_offset(authored_eye - target, up, view);
                    let eye = target + offset;
                    let vertical_fov_radians = vertical_fov_degrees.to_radians();

                    resolved.world = Some(match view.projection {
                        WorldProjection::Perspective => PerspectiveCamera {
                            eye,
                            target,
                            up,
                            vertical_fov_radians,
                            near,
                            far,
                        }
                        .view_projection(aspect),
                        WorldProjection::Orthographic => {
                            let half_height =
                                eye.distance(target) * (vertical_fov_radians * 0.5).tan();
                            let half_width = half_height * aspect;
                            Mat4::orthographic_rh(
                                -half_width,
                                half_width,
                                -half_height,
                                half_height,
                                near,
                                far,
                            ) * Mat4::look_at_rh(eye, target, up)
                        }
                    });
                }
                CameraComponent::Orthographic {
                    center,
                    vertical_size,
                    near,
                    far,
                } => {
                    let center = Vec2::from_array(center);
                    resolved.overlay = Some(
                        OrthographicCamera {
                            center,
                            vertical_size,
                            near,
                            far,
                        }
                        .view_projection(aspect),
                    );
                    let half_height = vertical_size * 0.5;
                    resolved.overlay_extent = Some(OverlayExtent {
                        center,
                        half_extent: Vec2::new(half_height * aspect, half_height),
                    });
                }
            }
        }
        Ok(resolved)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ResolvedCameras {
    world: Option<Mat4>,
    overlay: Option<Mat4>,
    overlay_extent: Option<OverlayExtent>,
}

/// The overlay camera's visible half-size, which sprite anchors resolve against.
#[derive(Clone, Copy, Debug)]
struct OverlayExtent {
    center: Vec2,
    half_extent: Vec2,
}

fn orbited_offset(authored_offset: Vec3, up: Vec3, view: CameraView) -> Vec3 {
    let scaled = authored_offset * view.distance_scale;
    let yawed = Quat::from_axis_angle(up, view.orbit.x) * scaled;
    let right = up.cross(yawed).normalize_or_zero();
    Quat::from_axis_angle(right, view.orbit.y) * yawed
}

fn transform_matrix(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation),
        Vec3::from_array(transform.position),
    )
}

fn sprite_matrix(transform: Transform2D, anchor: SpriteAnchor, extent: OverlayExtent) -> Mat4 {
    let unit = Vec2::from_array(anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + Vec2::from_array(transform.position);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_rotation_z(transform.rotation_radians)
        * Mat4::from_scale(Vec3::new(transform.scale[0], transform.scale[1], 1.0))
}

#[derive(Debug, Error)]
pub enum SceneExtractError {
    #[error(transparent)]
    Components(#[from] ComponentRegistryError),
    #[error(transparent)]
    Frame(#[from] FramePlanError),
    #[error(transparent)]
    TransparentOrder(#[from] TransparentOrderError),
    #[error("the scene draws meshes but has no perspective camera")]
    MissingWorldCamera,
    #[error("the scene draws sprites but has no orthographic camera")]
    MissingOverlayCamera,
    #[error("camera distance scale must be finite and greater than zero")]
    InvalidCameraDistanceScale,
}
