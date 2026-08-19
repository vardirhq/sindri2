use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec2, Vec3};
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, SceneComponent, SceneDocument, Transform3D,
    UnknownComponentPolicy, World,
};
use sindri_render::{
    ClearOperations, ExtractedFrame, FrameCamera, FrameCommand, FramePass, FramePlanError,
    OrthographicCamera, PerspectiveCamera, PreparedFrame, RenderLayer, RenderStage, SpriteDepth,
    SpriteInstance, TextureId, TransparentOrder, TransparentOrderError, Viewport, look_at,
    orthographic_projection,
};
use thiserror::Error;

use crate::{
    CameraComponent, MeshComponent, MeshPrimitive, SpriteAnchor, SpriteComponent, SpriteSpace,
    TextureBindings,
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
    /// Sideways and upward shift across the view plane, in fractions of the
    /// framed half-height.
    ///
    /// Measured against what the camera currently frames rather than in world
    /// units, so dragging moves the picture by the same amount whether the
    /// subject is a metre away or a kilometre, and the two projections agree.
    pub pan: Vec2,
    pub projection: WorldProjection,
}

impl Default for CameraView {
    fn default() -> Self {
        Self {
            orbit: Vec2::ZERO,
            distance_scale: 1.0,
            pan: Vec2::ZERO,
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

        // Sprites batch per space, layer, and texture, back to front within a
        // batch, with a stable tie-break. A batch is one draw, so instances
        // share one only when they share the texture it binds — and the space,
        // which decides both the camera and the pipeline.
        let mut batches: BTreeMap<
            (SpriteSpace, i32, TextureId),
            Vec<(TransparentOrder, SpriteInstance)>,
        > = BTreeMap::new();
        for (entity, sprite) in self.components.query::<SpriteComponent>(world)? {
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let model = match sprite.screen_anchor() {
                Some(anchor) => screen_sprite_matrix(
                    transform,
                    anchor,
                    cameras
                        .overlay_extent
                        .ok_or(SceneExtractError::MissingOverlayCamera)?,
                ),
                None => transform_matrix(transform),
            };
            let order = TransparentOrder::new(sprite.layer, sprite.depth, entity.index())?;
            batches
                .entry((
                    sprite.space,
                    sprite.layer,
                    textures.resolve(&sprite.texture),
                ))
                .or_default()
                .push((order, SpriteInstance::new(model, sprite.tint)));
        }

        for ((space, layer, texture), mut sprites) in batches {
            sprites.sort_by_key(|(order, _)| *order);
            let (stage, view_projection, depth) = match space {
                SpriteSpace::Screen => (
                    RenderStage::Overlay,
                    cameras
                        .overlay
                        .ok_or(SceneExtractError::MissingOverlayCamera)?,
                    SpriteDepth::Ignore,
                ),
                SpriteSpace::World => (
                    RenderStage::Transparent2d,
                    cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                    SpriteDepth::Test,
                ),
            };
            frame.push(FramePass::new(
                stage,
                RenderLayer(layer),
                FrameCamera { view_projection },
                FrameCommand::SpriteBatch {
                    texture,
                    depth,
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
        if !view.pan.is_finite() {
            return Err(SceneExtractError::InvalidCameraPan);
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
                    let vertical_fov_radians = vertical_fov_degrees.to_radians();
                    // Half the height the camera frames at the target, which is
                    // what both projections size themselves by, so a pan of one
                    // moves the picture by half a screen either way.
                    let half_height = offset.length() * (vertical_fov_radians * 0.5).tan();
                    let shift = panned_shift(offset, up, view.pan * half_height);
                    let target = target + shift;
                    let eye = target + offset;

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
                            let half_width = half_height * aspect;
                            orthographic_projection(
                                -half_width,
                                half_width,
                                -half_height,
                                half_height,
                                near,
                                far,
                            ) * look_at(eye, target, up)
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

/// Turns a pan measured in framed units into a world-space shift.
///
/// The shift stays in the plane the camera faces, so panning slides the picture
/// rather than pushing the camera towards or away from what it is looking at.
fn panned_shift(offset: Vec3, up: Vec3, pan: Vec2) -> Vec3 {
    let forward = -offset.normalize_or_zero();
    let right = forward.cross(up).normalize_or_zero();
    let plane_up = right.cross(forward);
    right * -pan.x + plane_up * -pan.y
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

/// Where a screen-anchored sprite lands, given the one transform.
///
/// Only X and Y of the transform reach the overlay: a screen-space sprite is
/// positioned against the camera's extent, so its Z has nowhere to go. A
/// world-space sprite has no such loss — it goes through `transform_matrix`,
/// the same one a mesh does, because it is in the same world a mesh is.
///
/// The rotation is taken about Z alone, which is what a flat thing facing the
/// camera can turn about.
fn screen_sprite_matrix(
    transform: Transform3D,
    anchor: SpriteAnchor,
    extent: OverlayExtent,
) -> Mat4 {
    let unit = Vec2::from_array(anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + Vec2::new(transform.position[0], transform.position[1]);
    let rotation = Quat::from_array(transform.rotation);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_quat(rotation)
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
    #[error("the scene draws in the world but has no perspective camera")]
    MissingWorldCamera,
    #[error("the scene draws screen-space sprites but has no orthographic camera")]
    MissingOverlayCamera,
    #[error("camera distance scale must be finite and greater than zero")]
    InvalidCameraDistanceScale,
    #[error("camera pan must be finite")]
    InvalidCameraPan,
}
