use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec2, Vec3};
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, SceneComponent, SceneDocument, Transform3D,
    UnknownComponentPolicy, World,
};
use sindri_render::{
    ClearOperations, ExtractedFrame, FrameCamera, FrameCommand, FramePass, FramePlanError,
    OrthographicCamera, PerspectiveCamera, PreparedFrame, RenderLayer, RenderStage, SpriteDepth,
    SpriteInstance, TextureId, TransparentOrder, TransparentOrderError, Viewport,
    orthographic_projection, perspective_projection,
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
                    view_projection: cameras
                        .world
                        .ok_or(SceneExtractError::MissingWorldCamera)?
                        .view_projection,
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
            let (model, camera) = match sprite.screen_anchor() {
                Some(anchor) => {
                    let extent = cameras
                        .overlay_extent
                        .ok_or(SceneExtractError::MissingOverlayCamera)?;
                    (
                        screen_sprite_matrix(transform, anchor, extent),
                        cameras
                            .overlay
                            .ok_or(SceneExtractError::MissingOverlayCamera)?,
                    )
                }
                None => (
                    transform_matrix(transform),
                    cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                ),
            };
            // A screen sprite is drawn flat against the overlay, but its Z
            // still says how far back in the stack it sits, so the distance is
            // measured against the authored Z rather than the flattened one. A
            // world sprite's two Zs are the same number.
            let position = model.w_axis.truncate().with_z(transform.position[2]);
            let order = TransparentOrder::new(
                sprite.layer,
                camera_distance(camera.view, position),
                entity.index(),
            )?;
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
            let (stage, camera, depth) = match space {
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
                FrameCamera {
                    view_projection: camera.view_projection,
                },
                FrameCommand::SpriteBatch {
                    texture,
                    depth,
                    instances: sprites.into_iter().map(|(_, sprite)| sprite).collect(),
                },
            ));
        }

        Ok(frame.prepare()?)
    }

    /// Where the world camera ends up looking, under the same adjustment a
    /// frame would be extracted with.
    ///
    /// An editor paints chrome of its own — an axis indicator, a grid, a
    /// gizmo — and has to know which way the world is facing to draw any of it
    /// truthfully. Without this it either extracts a frame it throws away or
    /// keeps a second copy of the orbit maths, and a second copy is how an
    /// indicator ends up disagreeing with the picture it sits on top of.
    ///
    /// The view alone, not the projection: chrome sits in the corner of a
    /// viewport rather than in the world, so how the world is flattened is not
    /// its business. `None` means the world holds no perspective camera, which
    /// is what extraction reports as [`SceneExtractError::MissingWorldCamera`].
    pub fn world_camera_view(
        &self,
        world: &World,
        view: CameraView,
    ) -> Result<Option<Mat4>, SceneExtractError> {
        // Any aspect ratio will do: it shapes a projection, and none is
        // returned.
        Ok(self
            .resolve_cameras(world, 1.0, view)?
            .world
            .map(|camera| camera.view))
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

                    // One view for both projections: where the camera is and
                    // what it looks at does not depend on how it flattens the
                    // world, and a sprite must not change places when the
                    // editor toggles between them.
                    let camera = PerspectiveCamera {
                        eye,
                        target,
                        up,
                        vertical_fov_radians,
                        near,
                        far,
                    };
                    let projection = match view.projection {
                        WorldProjection::Perspective => {
                            perspective_projection(vertical_fov_radians, aspect, near, far)
                        }
                        WorldProjection::Orthographic => {
                            let half_width = half_height * aspect;
                            orthographic_projection(
                                -half_width,
                                half_width,
                                -half_height,
                                half_height,
                                near,
                                far,
                            )
                        }
                    };
                    let view = camera.view();
                    resolved.world = Some(ResolvedCamera {
                        view,
                        view_projection: projection * view,
                    });
                }
                CameraComponent::Orthographic {
                    center,
                    vertical_size,
                    near,
                    far,
                } => {
                    let center = Vec2::from_array(center);
                    let camera = OrthographicCamera {
                        center,
                        vertical_size,
                        near,
                        far,
                    };
                    resolved.overlay = Some(ResolvedCamera {
                        view: camera.view(),
                        view_projection: camera.view_projection(aspect),
                    });
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
    world: Option<ResolvedCamera>,
    overlay: Option<ResolvedCamera>,
    overlay_extent: Option<OverlayExtent>,
}

/// A camera as extraction needs it: the matrix that draws through it, and the
/// view on its own, which is what a distance is measured in.
#[derive(Clone, Copy, Debug)]
struct ResolvedCamera {
    view: Mat4,
    view_projection: Mat4,
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

/// How far in front of a camera a point is, which is what transparent draws
/// sort by.
///
/// Measured along the camera's forward axis rather than as a straight line to
/// the eye: two sprites side by side at the same depth have to sort as equally
/// far away, and a radial distance would call the one nearer the edge of the
/// screen further back. Nothing divides, so a sprite sitting exactly on the
/// camera plane produces a number rather than an infinity.
fn camera_distance(view: Mat4, position: Vec3) -> f32 {
    -(view * position.extend(1.0)).z
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
/// positioned against the camera's extent, and its Z orders it rather than
/// placing it, so the matrix is flat. A world-space sprite has no such split —
/// it goes through `transform_matrix`, the same one a mesh does, because it is
/// in the same world a mesh is.
///
/// The whole rotation still reaches the overlay: a quad turned about X or Y
/// foreshortens under the orthographic camera, which is a card flip rather than
/// a mistake. Only the position and the scale are read two-dimensionally, and
/// they are read through the transform's own 2D accessors so that this agrees
/// with every other piece of code that means "in the plane".
fn screen_sprite_matrix(
    transform: Transform3D,
    anchor: SpriteAnchor,
    extent: OverlayExtent,
) -> Mat4 {
    let unit = Vec2::from_array(anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + Vec2::from_array(transform.position_2d());
    let rotation = Quat::from_array(transform.rotation);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_quat(rotation)
        * Mat4::from_scale(Vec2::from_array(transform.scale_2d()).extend(1.0))
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
