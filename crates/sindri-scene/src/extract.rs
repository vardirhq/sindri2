use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec2, Vec3};
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, EntityId, SceneComponent, SceneDocument,
    SpriteRefError, Transform3D, UnknownComponentPolicy, World,
};
use sindri_render::{
    ClearOperations, ExtractedFrame, FrameCamera, FrameCommand, FramePass, FramePlanError,
    OrthographicCamera, PerspectiveCamera, PreparedFrame, RenderLayer, RenderStage, SpriteDepth,
    SpriteInstance, TextError, TextInstance, TextureId, TransparentOrder, TransparentOrderError,
    UvRect, UvRectError, Viewport, orthographic_projection, perspective_projection,
};
use thiserror::Error;

use crate::{
    AnimationError, AudioSourceComponent, CameraComponent, Collider2dComponent,
    GridNavigationComponent, GridOccupantComponent, MeshComponent, MeshPrimitive,
    PROCEDURAL_TEXTURES, RigidBody2dComponent, SpriteAnchor, SpriteAnimationComponent,
    SpriteAnimations, SpriteComponent, SpriteSpace, TextComponent, TextureBindings,
    TilemapComponent, TilemapError,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldProjection {
    #[default]
    Authored,
    Perspective,
    Orthographic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    pub orbit: Vec2,
    pub distance_scale: f32,
    pub pan: Vec2,
    pub projection: WorldProjection,
}

impl Default for CameraView {
    fn default() -> Self {
        Self {
            orbit: Vec2::ZERO,
            distance_scale: 1.0,
            pan: Vec2::ZERO,
            projection: WorldProjection::Authored,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SceneExtractor {
    components: ComponentSchemaRegistry,
}

impl SceneExtractor {
    pub fn new() -> Result<Self, SceneExtractError> {
        let mut components = ComponentSchemaRegistry::default();
        components.register_with_default::<CameraComponent>(
            "Camera",
            serde_json::json!({
                "projection": "perspective",
                "vertical_fov_degrees": 60.0,
                "near": 0.1,
                "far": 100.0
            }),
        )?;
        components.register_with_default::<MeshComponent>(
            "Mesh",
            serde_json::json!({
                "primitive": "cube",
                "texture": PROCEDURAL_TEXTURES[0].reference,
                "layer": 0
            }),
        )?;
        components.register_with_default::<SpriteComponent>(
            "Sprite",
            serde_json::json!({
                "texture": PROCEDURAL_TEXTURES[0].reference,
                "tint": [1.0, 1.0, 1.0, 1.0],
                "layer": 0
            }),
        )?;
        components.register::<SpriteAnimationComponent>("Sprite Animation")?;
        components.register::<TextComponent>("Text")?;
        components.register_with_default::<TilemapComponent>(
            "Tilemap",
            serde_json::json!({
                "texture": PROCEDURAL_TEXTURES[0].reference,
                "palette": [],
                "columns": 1,
                "rows": 1,
                "tiles": [null],
                "space": "world"
            }),
        )?;
        components.register_with_default::<GridNavigationComponent>(
            "Grid Navigation",
            serde_json::json!({ "walls": [] }),
        )?;
        components.register::<GridOccupantComponent>("Grid Occupant")?;
        components.register_with_default::<RigidBody2dComponent>(
            "Rigid Body 2D",
            serde_json::json!({
                "kind": "dynamic",
                "pose": { "position": [0.0, 0.0], "rotation": 0.0 },
                "linear_velocity": [0.0, 0.0],
                "angular_velocity": 0.0,
                "gravity_scale": 1.0,
                "linear_damping": 0.0,
                "angular_damping": 0.0,
                "lock_rotation": false
            }),
        )?;
        components.register_with_default::<Collider2dComponent>(
            "Collider 2D",
            serde_json::json!({
                "shape": { "shape": "box", "half_extents": [0.5, 0.5] },
                "offset": [0.0, 0.0],
                "rotation": 0.0,
                "sensor": false,
                "layers": { "memberships": 4_294_967_295_u32, "filter": 4_294_967_295_u32 },
                "friction": 0.5,
                "restitution": 0.0
            }),
        )?;
        components.register::<AudioSourceComponent>("Audio Source")?;
        Ok(Self { components })
    }

    pub const fn components(&self) -> &ComponentSchemaRegistry {
        &self.components
    }

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

    pub fn extract(
        &self,
        world: &World,
        viewport: Viewport,
        view: CameraView,
        textures: &TextureBindings,
    ) -> Result<PreparedFrame, SceneExtractError> {
        self.extract_animated(world, viewport, view, textures, &SpriteAnimations::new())
    }

    pub fn extract_animated(
        &self,
        world: &World,
        viewport: Viewport,
        view: CameraView,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
    ) -> Result<PreparedFrame, SceneExtractError> {
        let aspect = viewport.aspect_ratio()?;
        let views = self.resolve_views(world, aspect, view)?;
        let mut frame = ExtractedFrame::new(viewport, ClearOperations::default());
        self.push_meshes(world, &views, textures, &mut frame)?;
        self.push_sprites(world, &views, textures, animations, &mut frame)?;
        self.push_text(world, viewport, &views, &mut frame)?;
        Ok(frame.prepare()?)
    }

    fn push_text(
        &self,
        world: &World,
        viewport: Viewport,
        views: &ResolvedViews,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let texts = self.components.query::<TextComponent>(world)?;
        if texts.is_empty() {
            return Ok(());
        }
        let width = f32::from(u16::try_from(viewport.width)?);
        let height = f32::from(u16::try_from(viewport.height)?);
        let mut layers: BTreeMap<i32, Vec<TextInstance>> = BTreeMap::new();

        for (entity, text) in texts {
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let model = screen_sprite_matrix(transform, text.anchor, views.screen_extent);
            let clip = views.screen.view_projection * model.w_axis;
            let ndc = clip.truncate() / clip.w;
            let position = [(ndc.x + 1.0) * 0.5 * width, (1.0 - ndc.y) * 0.5 * height];
            layers
                .entry(text.layer)
                .or_default()
                .push(TextInstance::new(
                    text.text,
                    text.font,
                    position,
                    text.font_size,
                    text.line_height,
                    text.color,
                )?);
        }

        for (layer, instances) in layers {
            frame.push(FramePass::new(
                RenderStage::Overlay,
                RenderLayer(layer),
                FrameCamera {
                    view_projection: views.screen.view_projection,
                },
                FrameCommand::Text { instances },
            ));
        }
        Ok(())
    }

    fn push_meshes(
        &self,
        world: &World,
        views: &ResolvedViews,
        textures: &TextureBindings,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
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
                    view_projection: views
                        .world
                        .ok_or(SceneExtractError::MissingWorldCamera)?
                        .view_projection,
                },
                command,
            ));
        }
        Ok(())
    }

    fn push_tilemaps(
        &self,
        world: &World,
        views: &ResolvedViews,
        textures: &TextureBindings,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
        for (entity, tilemap) in self.components.query::<TilemapComponent>(world)? {
            tilemap.validate()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let texture = textures.resolve(&tilemap.texture);
            let palette: Vec<UvRect> = tilemap
                .palette
                .iter()
                .map(|sprite| {
                    textures
                        .sheet_sprite(&tilemap.texture, sprite)
                        .unwrap_or(UvRect::FULL)
                })
                .collect();
            for (column, row, index) in tilemap.filled() {
                let rect = palette.get(index as usize).copied().unwrap_or(UvRect::FULL);
                let [offset_x, offset_y] = tilemap.tile_to_local(column, row);
                let local = Mat4::from_translation(Vec3::new(offset_x, offset_y, 0.0))
                    * Mat4::from_scale(Vec3::new(tilemap.tile_size[0], tilemap.tile_size[1], 1.0));

                let (model, camera) = if tilemap.is_screen_space() {
                    (
                        screen_sprite_matrix(transform, SpriteAnchor::default(), views.screen_extent)
                            * local,
                        views.screen,
                    )
                } else {
                    (
                        transform_matrix(transform) * local,
                        views.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                    )
                };
                let position = model.w_axis.truncate().with_z(transform.position[2]);
                let order = TransparentOrder::new(
                    tilemap.layer,
                    camera_distance(camera.view, position),
                    row.saturating_mul(tilemap.columns).saturating_add(column),
                )?;
                batches
                    .entry((tilemap.space, tilemap.layer, texture))
                    .or_default()
                    .push((
                        order,
                        SpriteInstance::new(model, tilemap.tint).with_uv_rect(rect),
                    ));
            }
        }
        Ok(())
    }

    fn push_sprites(
        &self,
        world: &World,
        views: &ResolvedViews,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let mut batches: SpriteBatches = BTreeMap::new();
        let mut resting: BTreeMap<EntityId, String> = BTreeMap::new();
        for (entity, animation) in self.components.query::<SpriteAnimationComponent>(world)? {
            if let Some(sprite) = animation.resting_sprite().ok().flatten() {
                resting.insert(entity, sprite.to_owned());
            }
        }
        for (entity, sprite) in self.components.query::<SpriteComponent>(world)? {
            let reference = sprite.reference()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let (model, camera) = match sprite.screen_anchor() {
                Some(anchor) => (
                    screen_sprite_matrix(transform, anchor, views.screen_extent),
                    views.screen,
                ),
                None => (
                    transform_matrix(transform),
                    views.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                ),
            };
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
                    textures.resolve(reference.texture()),
                ))
                .or_default()
                .push((
                    order,
                    SpriteInstance::new(model, sprite.tint).with_uv_rect(
                        match animations.sprite(entity) {
                            Some(name) => textures
                                .sheet_sprite(reference.texture(), name)
                                .unwrap_or(UvRect::FULL),
                            None => reference
                                .sprite()
                                .is_none()
                                .then(|| {
                                    resting.get(&entity).and_then(|name| {
                                        textures.sheet_sprite(reference.texture(), name)
                                    })
                                })
                                .flatten()
                                .or_else(|| textures.sprite_rect(&reference))
                                .unwrap_or(UvRect::FULL),
                        },
                    ),
                ));
        }

        self.push_tilemaps(world, views, textures, &mut batches)?;

        for ((space, layer, texture), mut sprites) in batches {
            sprites.sort_by_key(|(order, _)| *order);
            let (stage, camera, depth) = match space {
                SpriteSpace::Screen => (RenderStage::Overlay, views.screen, SpriteDepth::Ignore),
                SpriteSpace::World => (
                    RenderStage::Transparent2d,
                    views.world.ok_or(SceneExtractError::MissingWorldCamera)?,
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
        Ok(())
    }

    pub fn world_camera(
        &self,
        world: &World,
        view: CameraView,
    ) -> Result<Option<ViewCamera>, SceneExtractError> {
        self.world_camera_for_viewport(world, 1.0, view)
    }

    pub fn world_camera_for_viewport(
        &self,
        world: &World,
        aspect: f32,
        view: CameraView,
    ) -> Result<Option<ViewCamera>, SceneExtractError> {
        Ok(self.resolve_views(world, aspect, view)?.world.map(|camera| ViewCamera {
            view: camera.view,
            view_projection: camera.view_projection,
            framed_half_height: camera.framed_half_height,
        }))
    }

    fn resolve_views(
        &self,
        world: &World,
        aspect: f32,
        view: CameraView,
    ) -> Result<ResolvedViews, SceneExtractError> {
        if !view.distance_scale.is_finite() || view.distance_scale <= 0.0 {
            return Err(SceneExtractError::InvalidCameraDistanceScale);
        }
        if !view.pan.is_finite() {
            return Err(SceneExtractError::InvalidCameraPan);
        }

        match view.projection {
            WorldProjection::Authored => self.resolve_authored_views(world, aspect),
            WorldProjection::Perspective | WorldProjection::Orthographic => {
                Ok(self.resolve_viewer_views(aspect, view))
            }
        }
    }

    fn resolve_viewer_views(&self, aspect: f32, view: CameraView) -> ResolvedViews {
        let mut resolved = screen_views(aspect);
        let up = Vec3::Y;
        let offset = orbited_offset(Vec3::new(3.0, 2.0, 4.0), up, view);
        let vertical_fov_radians = 45.0_f32.to_radians();
        let near = 0.1;
        let far = 1_000.0;
        let half_height = offset.length() * (vertical_fov_radians * 0.5).tan();
        let shift = panned_shift(offset, up, view.pan * half_height);
        let target = shift;
        let eye = target + offset;
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
            WorldProjection::Authored => unreachable!("viewer views are never authored"),
        };
        let view = camera.view();
        resolved.world = Some(ResolvedCamera {
            view,
            view_projection: projection * view,
            framed_half_height: half_height,
        });
        resolved
    }

    fn resolve_authored_views(
        &self,
        world: &World,
        aspect: f32,
    ) -> Result<ResolvedViews, SceneExtractError> {
        let mut resolved = screen_views(aspect);
        for (entity, camera) in self.components.query::<CameraComponent>(world)? {
            if resolved.world.is_some() {
                return Err(SceneExtractError::MultipleWorldCameras);
            }
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let eye = Vec3::from_array(transform.position);
            let rotation = safe_rotation(transform);
            let target = eye + rotation * -Vec3::Z;
            let up = rotation * Vec3::Y;
            let view = Mat4::look_at_rh(eye, target, up);

            resolved.world = Some(match camera {
                CameraComponent::Perspective {
                    vertical_fov_degrees,
                    near,
                    far,
                } => {
                    let vertical_fov_radians = vertical_fov_degrees.to_radians();
                    ResolvedCamera {
                        view,
                        view_projection: perspective_projection(
                            vertical_fov_radians,
                            aspect,
                            near,
                            far,
                        ) * view,
                        framed_half_height: (vertical_fov_radians * 0.5).tan(),
                    }
                }
                CameraComponent::Orthographic {
                    vertical_size,
                    near,
                    far,
                } => {
                    let half_height = vertical_size * 0.5;
                    let half_width = half_height * aspect;
                    ResolvedCamera {
                        view,
                        view_projection: orthographic_projection(
                            -half_width,
                            half_width,
                            -half_height,
                            half_height,
                            near,
                            far,
                        ) * view,
                        framed_half_height: half_height,
                    }
                }
            });
        }
        Ok(resolved)
    }

    pub fn extract_animated_with_world_camera(
        &self,
        world: &World,
        viewport: Viewport,
        world_camera: ViewCamera,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
    ) -> Result<PreparedFrame, SceneExtractError> {
        let aspect = viewport.aspect_ratio()?;
        let mut views = screen_views(aspect);
        views.world = Some(ResolvedCamera {
            view: world_camera.view,
            view_projection: world_camera.view_projection,
            framed_half_height: world_camera.framed_half_height,
        });

        let mut frame = ExtractedFrame::new(viewport, ClearOperations::default());
        self.push_meshes(world, &views, textures, &mut frame)?;
        self.push_sprites(world, &views, textures, animations, &mut frame)?;
        self.push_text(world, viewport, &views, &mut frame)?;
        Ok(frame.prepare()?)
    }
}

#[derive(Clone, Copy, Debug)]
struct ResolvedViews {
    world: Option<ResolvedCamera>,
    screen: ResolvedCamera,
    screen_extent: ScreenExtent,
}

#[derive(Clone, Copy, Debug)]
struct ResolvedCamera {
    view: Mat4,
    view_projection: Mat4,
    framed_half_height: f32,
}

#[derive(Clone, Copy, Debug)]
struct ScreenExtent {
    center: Vec2,
    half_extent: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCamera {
    pub view: Mat4,
    pub view_projection: Mat4,
    pub framed_half_height: f32,
}

fn screen_views(aspect: f32) -> ResolvedViews {
    let vertical_size = 2.0;
    let near = 0.0;
    let far = 10.0;
    let camera = OrthographicCamera {
        center: Vec2::ZERO,
        vertical_size,
        near,
        far,
    };
    let half_height = vertical_size * 0.5;
    ResolvedViews {
        world: None,
        screen: ResolvedCamera {
            view: camera.view(),
            view_projection: camera.view_projection(aspect),
            framed_half_height: half_height,
        },
        screen_extent: ScreenExtent {
            center: Vec2::ZERO,
            half_extent: Vec2::new(half_height * aspect, half_height),
        },
    }
}

fn safe_rotation(transform: Transform3D) -> Quat {
    let rotation = Quat::from_array(transform.rotation);
    if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn panned_shift(offset: Vec3, up: Vec3, pan: Vec2) -> Vec3 {
    let forward = -offset.normalize_or_zero();
    let right = forward.cross(up).normalize_or_zero();
    let plane_up = right.cross(forward);
    right * -pan.x + plane_up * -pan.y
}

const POLAR_LIMIT: f32 = 0.01;

fn orbited_offset(authored_offset: Vec3, up: Vec3, view: CameraView) -> Vec3 {
    let scaled = authored_offset * view.distance_scale;
    let yawed = Quat::from_axis_angle(up, view.orbit.x) * scaled;
    let right = up.cross(yawed).normalize_or_zero();
    if right == Vec3::ZERO {
        return yawed;
    }
    let polar = up.angle_between(yawed);
    let pitch = view.orbit.y.clamp(
        POLAR_LIMIT - polar,
        std::f32::consts::PI - POLAR_LIMIT - polar,
    );
    Quat::from_axis_angle(right, pitch) * yawed
}

fn camera_distance(view: Mat4, position: Vec3) -> f32 {
    -(view * position.extend(1.0)).z
}

fn transform_matrix(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        safe_rotation(transform),
        Vec3::from_array(transform.position),
    )
}

fn screen_sprite_matrix(
    transform: Transform3D,
    anchor: SpriteAnchor,
    extent: ScreenExtent,
) -> Mat4 {
    let unit = Vec2::from_array(anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + Vec2::from_array(transform.position_2d());
    let rotation = safe_rotation(transform);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_quat(rotation)
        * Mat4::from_scale(Vec2::from_array(transform.scale_2d()).extend(1.0))
}

type SpriteBatches =
    BTreeMap<(SpriteSpace, i32, TextureId), Vec<(TransparentOrder, SpriteInstance)>>;

#[derive(Debug, Error)]
pub enum SceneExtractError {
    #[error(transparent)]
    Components(#[from] ComponentRegistryError),
    #[error(transparent)]
    Frame(#[from] FramePlanError),
    #[error(transparent)]
    TransparentOrder(#[from] TransparentOrderError),
    #[error("the scene draws in the world but has no authored camera")]
    MissingWorldCamera,
    #[error("the scene contains more than one authored world camera")]
    MultipleWorldCameras,
    #[error("screen-space overlay no longer uses an authored camera")]
    MissingOverlayCamera,
    #[error("camera distance scale must be finite and greater than zero")]
    InvalidCameraDistanceScale,
    #[error("camera pan must be finite")]
    InvalidCameraPan,
    #[error(transparent)]
    UvRect(#[from] UvRectError),
    #[error(transparent)]
    Animation(#[from] AnimationError),
    #[error(transparent)]
    Tilemap(#[from] TilemapError),
    #[error(transparent)]
    SpriteRef(#[from] SpriteRefError),
    #[error(transparent)]
    Text(#[from] TextError),
    #[error("the viewport is too large for text rendering")]
    TextViewport(#[from] std::num::TryFromIntError),
}
