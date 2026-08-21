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
    AnimationError, CameraComponent, MeshComponent, MeshPrimitive, PROCEDURAL_TEXTURES,
    SpriteAnchor, SpriteAnimationComponent, SpriteAnimations, SpriteComponent, SpriteSpace,
    TextComponent, TextureBindings, TilemapComponent, TilemapError,
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
        // Each default is what a freshly added component of that type looks
        // like, and is what makes the type addable at all. They are chosen to
        // be visible rather than neutral: a sprite added to an entity should
        // appear, or the author is left wondering whether the click worked.
        components.register_with_default::<CameraComponent>(
            "Camera",
            serde_json::json!({
                "projection": "perspective",
                "target": [0.0, 0.0, 0.0],
                "up": [0.0, 1.0, 0.0],
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
        // No default: an animation with no sheet and no clips is a component
        // that does nothing, and one with an invented sheet would claim a
        // texture is laid out a way it is not.
        components.register::<SpriteAnimationComponent>("Sprite Animation")?;
        // No default: unlike a procedural texture, there is no honest font the
        // engine can invent. The editor's font picker supplies a project asset
        // when text is added; existing text remains generically editable now.
        components.register::<TextComponent>("Text")?;
        // A one-by-one map of one empty cell: the smallest tilemap that is
        // still a valid one, so adding the component in the editor gives
        // something to paint into rather than something to repair.
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

    /// Extracts every drawable in `world` into an ordered frame, with no
    /// animation playing.
    ///
    /// A sprite carrying clips draws its own authored rect, which is the pose a
    /// scene shows before anything has run it.
    pub fn extract(
        &self,
        world: &World,
        viewport: Viewport,
        view: CameraView,
        textures: &TextureBindings,
    ) -> Result<PreparedFrame, SceneExtractError> {
        self.extract_animated(world, viewport, view, textures, &SpriteAnimations::new())
    }

    /// Extracts every drawable in `world`, with each animated sprite showing the
    /// frame `animations` has it on.
    pub fn extract_animated(
        &self,
        world: &World,
        viewport: Viewport,
        view: CameraView,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
    ) -> Result<PreparedFrame, SceneExtractError> {
        let aspect = viewport.aspect_ratio()?;
        let cameras = self.resolve_cameras(world, aspect, view)?;
        let mut frame = ExtractedFrame::new(viewport, ClearOperations::default());
        self.push_meshes(world, &cameras, textures, &mut frame)?;
        self.push_sprites(world, &cameras, textures, animations, &mut frame)?;
        self.push_text(world, viewport, &cameras, &mut frame)?;
        Ok(frame.prepare()?)
    }

    fn push_text(
        &self,
        world: &World,
        viewport: Viewport,
        cameras: &ResolvedCameras,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let texts = self.components.query::<TextComponent>(world)?;
        if texts.is_empty() {
            return Ok(());
        }
        let overlay = cameras
            .overlay
            .ok_or(SceneExtractError::MissingOverlayCamera)?;
        let extent = cameras
            .overlay_extent
            .ok_or(SceneExtractError::MissingOverlayCamera)?;
        let width = f32::from(u16::try_from(viewport.width)?);
        let height = f32::from(u16::try_from(viewport.height)?);
        let mut layers: BTreeMap<i32, Vec<TextInstance>> = BTreeMap::new();

        for (entity, text) in texts {
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let model = screen_sprite_matrix(transform, text.anchor, extent);
            let clip = overlay.view_projection * model.w_axis;
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
                    view_projection: overlay.view_projection,
                },
                FrameCommand::Text { instances },
            ));
        }
        Ok(())
    }

    fn push_meshes(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
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
                    view_projection: cameras
                        .world
                        .ok_or(SceneExtractError::MissingWorldCamera)?
                        .view_projection,
                },
                command,
            ));
        }
        Ok(())
    }

    /// Turns every tilemap's filled cells into sprite instances, into the same
    /// batches loose sprites use.
    ///
    /// The same batches deliberately: a tilemap is not a second kind of thing
    /// to draw, it is a compact way to author many of the first kind. Sharing
    /// the map means a tilemap and a loose sprite on one layer and one texture
    /// share a draw and sort against each other, so a prop can sit between two
    /// rows of floor without the tilemap being a plane that swallows it.
    fn push_tilemaps(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
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
            // The palette is resolved once and the cells index the answers: a
            // map of 49 tiles names a handful of sprites, so looking each one
            // up per cell would be the same lookup forty-nine times.
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
                // One tile is a sprite of the map's tile size, placed by the
                // map's own maths and then by the entity's transform, so moving
                // the entity moves the floor.
                let mut tile = transform;
                tile.position[0] += offset_x;
                tile.position[1] += offset_y;
                tile.scale[0] *= tilemap.tile_size[0];
                tile.scale[1] *= tilemap.tile_size[1];

                let (model, camera) = if tilemap.is_screen_space() {
                    let extent = cameras
                        .overlay_extent
                        .ok_or(SceneExtractError::MissingOverlayCamera)?;
                    (
                        screen_sprite_matrix(tile, SpriteAnchor::default(), extent),
                        cameras
                            .overlay
                            .ok_or(SceneExtractError::MissingOverlayCamera)?,
                    )
                } else {
                    (
                        transform_matrix(tile),
                        cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                    )
                };
                let position = model.w_axis.truncate().with_z(tile.position[2]);
                // Row and column break the tie rather than the entity index,
                // because every tile of one map shares an entity. Reading order
                // is the map's order, so the same map extracts the same way
                // every time.
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
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        // Sprites batch per space, layer, and texture, back to front within a
        // batch, with a stable tie-break. A batch is one draw, so instances
        // share one only when they share the texture it binds — and the space,
        // which decides both the camera and the pipeline.
        let mut batches: SpriteBatches = BTreeMap::new();
        // What an animated sprite shows when `animations` has not reached it —
        // a scene just loaded, an entity in the editor outside play mode, a
        // frame captured before the first tick. Only sprites that authored no
        // rect of their own take it: a sheet drawn whole is every frame at
        // once, which is a picture nobody meant, while an authored rect is how
        // a scene picks a rest pose other than the clip's first frame.
        //
        // A broken clip falls through to the sprite's rect rather than failing
        // the frame, for the reason a broken clip does not fail loading: the
        // editor is where it gets fixed, and it has to draw to be fixed there.
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
                    textures.resolve(reference.texture()),
                ))
                .or_default()
                .push((
                    order,
                    SpriteInstance::new(model, sprite.tint).with_uv_rect(
                        // What the animation says, then what a clip would show
                        // at rest, then what the sprite itself names. A sheet
                        // drawn whole is every sprite at once, which is why an
                        // animated sprite that names no part of its own falls
                        // back to its clip's first frame rather than to
                        // everything.
                        match animations.sprite(entity) {
                            // A playing clip decides, and if what it names does
                            // not resolve the answer is the whole image rather
                            // than some other frame: falling back to frame zero
                            // would draw a plausible picture of the wrong
                            // moment, which is the failure that hides.
                            Some(name) => textures
                                .sheet_sprite(reference.texture(), name)
                                .unwrap_or(UvRect::FULL),
                            // Nothing has ticked it yet. A sprite that names no
                            // part of its own shows its clip's first frame,
                            // because a sheet drawn whole is every frame at once
                            // and that is a picture nobody meant.
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

        self.push_tilemaps(world, cameras, textures, &mut batches)?;

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
        Ok(())
    }

    /// Where the world camera ends up looking, under the same adjustment a
    /// frame would be extracted with.
    ///
    /// An editor paints chrome of its own — an axis indicator, a grid, a
    /// gizmo — and moves the camera on the user's behalf, and both need to know
    /// which way the world is facing and how much of it is framed. Without this
    /// it either extracts a frame it throws away or keeps a second copy of the
    /// orbit maths, and a second copy is how an indicator ends up disagreeing
    /// with the picture it sits on top of.
    ///
    /// No projection: chrome sits in the corner of a viewport rather than in
    /// the world, and where a thing is on screen relative to the middle does
    /// not depend on how the world is flattened. `None` means the world holds
    /// no perspective camera, which is what extraction reports as
    /// [`SceneExtractError::MissingWorldCamera`].
    pub fn world_camera(
        &self,
        world: &World,
        view: CameraView,
    ) -> Result<Option<ViewCamera>, SceneExtractError> {
        // Any aspect ratio will do for camera controls and corner chrome. A
        // tool that maps a viewport point back into the world asks
        // `world_camera_for_viewport` with the actual one.
        self.world_camera_for_viewport(world, 1.0, view)
    }

    /// Where the world camera looks, including the projection for one viewport.
    ///
    /// Tile painting is the first editor action that travels from a screen
    /// point back into the world. It must invert the exact view-projection the
    /// frame used; rebuilding one in the editor would be a second camera that
    /// only has to disagree once for every click to land on the wrong tile.
    pub fn world_camera_for_viewport(
        &self,
        world: &World,
        aspect: f32,
        view: CameraView,
    ) -> Result<Option<ViewCamera>, SceneExtractError> {
        Ok(self
            .resolve_cameras(world, aspect, view)?
            .world
            .map(|camera| ViewCamera {
                view: camera.view,
                view_projection: camera.view_projection,
                framed_half_height: camera.framed_half_height,
            }))
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
                        framed_half_height: half_height,
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
                    let half_height = vertical_size * 0.5;
                    resolved.overlay = Some(ResolvedCamera {
                        view: camera.view(),
                        view_projection: camera.view_projection(aspect),
                        framed_half_height: half_height,
                    });
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
    framed_half_height: f32,
}

/// The world camera as a viewport's own chrome and camera controls need it.
///
/// Handed out by [`SceneExtractor::world_camera`], which is the only supported
/// way to ask: everything here is derived from the authored camera and the
/// viewer's adjustment together, and deriving it a second time somewhere else
/// is how two answers about the same camera come to disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCamera {
    /// The matrix a frame drawn now would be seen through.
    pub view: Mat4,
    /// The exact projection and view used for a viewport of the requested
    /// aspect ratio. Inverting it turns a pointer into a world-space ray.
    pub view_projection: Mat4,
    /// Half the height the camera frames at its target, in world units.
    ///
    /// This is the unit a pan is measured in — a pan of one moves the picture
    /// by exactly this much — so it is also what turns a distance on screen
    /// back into a pan, which is how a viewport centres itself on something.
    pub framed_half_height: f32,
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

/// How close to straight up or straight down an orbit may take the camera.
///
/// At the pole the offset is parallel to `up`, and the pair no longer says
/// which way round the picture goes. `look_at` still returns a matrix there
/// rather than failing, which is worse than failing: the roll it picks is
/// decided by whatever rounding error survived, so dragging through straight
/// down whips the whole scene round to face the other way. Stopping a hundredth
/// of a radian short costs nothing anyone can see and removes the case.
const POLAR_LIMIT: f32 = 0.01;

/// Where the camera sits after the viewer's orbit, relative to its target.
///
/// Pitch turns the offset in the plane that holds it and `up`, so it adds
/// directly to the angle between them. That is what makes the guard here exact:
/// the limit is applied to the angle it actually produces, rather than to a
/// pitch that a caller would have to combine with an authored elevation it
/// cannot see to know whether it was safe.
fn orbited_offset(authored_offset: Vec3, up: Vec3, view: CameraView) -> Vec3 {
    let scaled = authored_offset * view.distance_scale;
    let yawed = Quat::from_axis_angle(up, view.orbit.x) * scaled;
    let right = up.cross(yawed).normalize_or_zero();
    if right == Vec3::ZERO {
        // Authored looking straight down its own up axis. There is no axis to
        // pitch about, and the scene chose this, so it is left alone.
        return yawed;
    }
    let polar = up.angle_between(yawed);
    let pitch = view.orbit.y.clamp(
        POLAR_LIMIT - polar,
        std::f32::consts::PI - POLAR_LIMIT - polar,
    );
    Quat::from_axis_angle(right, pitch) * yawed
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

/// Everything that will become a sprite draw, gathered before any of it is
/// ordered.
///
/// Keyed by what decides a batch — the space, which picks the camera and the
/// pipeline; the layer, which overrides distance; and the texture, which is what
/// a draw binds. Tilemaps and loose sprites fill the same map, so they share a
/// batch when they share all three.
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
    #[error("the scene draws in the world but has no perspective camera")]
    MissingWorldCamera,
    #[error("the scene draws screen-space sprites but has no orthographic camera")]
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
