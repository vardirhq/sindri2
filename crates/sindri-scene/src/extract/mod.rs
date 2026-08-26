//! Deriving what to draw from what the world holds.
//!
//! Gameplay writes the world; nothing here does. Each component family
//! has a file that turns it into frame commands, and `extract` is the
//! order those run in. A new drawable component is a file beside them
//! and one `push_` call here.

mod camera;
mod mesh;
mod sprite;
mod text;
mod tilemap;

use camera::ResolvedCamera;
use camera::view::{resolved_screen_overlay, safe_rotation};
use glam::{Mat4, Vec3};
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, SceneComponent, SceneDocument, SpriteRefError,
    Transform3D, UnknownComponentPolicy, World,
};
use sindri_render::{
    ClearOperations, ExtractedFrame, FramePlanError, PreparedFrame, TextError,
    TransparentOrderError, UvRectError, Viewport,
};
use thiserror::Error;

use crate::{
    AnimationError, AudioSourceComponent, CameraComponent, Collider2dComponent,
    GridNavigationComponent, GridOccupantComponent, MeshComponent, PROCEDURAL_TEXTURES,
    RigidBody2dComponent, SpriteAnimationComponent, SpriteAnimations, SpriteComponent,
    TextComponent, TextureBindings, TilemapComponent, TilemapError,
};

pub use camera::ViewCamera;
pub use camera::view::{CameraView, WorldProjection};

/// Turns a world into a frame the renderer can draw.
///
/// This is the seam between simulation and rendering: gameplay only ever writes
/// to the world, and everything drawn is derived from registered components. No
/// scene needs hand-written extraction code.
#[derive(Clone, Debug)]
pub struct SceneExtractor {
    components: ComponentSchemaRegistry,
}

fn transform_matrix(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        safe_rotation(transform),
        Vec3::from_array(transform.position),
    )
}

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
        components.register_with_default::<GridNavigationComponent>(
            "Grid Navigation",
            serde_json::json!({ "walls": [] }),
        )?;
        // No default: an occupant must name the stable ID of the grid it
        // belongs to. Inventing one would create a valid-looking component
        // that cannot ever resolve.
        components.register::<GridOccupantComponent>("Grid Occupant")?;
        // Physics defaults are ordinary Sindri values rather than backend
        // values. A newly added body starts dynamic and a collider starts as a
        // one-unit box, so both are immediately valid and visible in the
        // generic command-backed inspector.
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
        // No default payload, for the reason the registry states: a blank one
        // would name the empty clip, and a button that adds a component the
        // engine then rejects is worse than no button. `sindri.text` and
        // `sindri.animation.sprite` are registered the same way.
        components.register::<AudioSourceComponent>("Audio Source")?;
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

    /// Extracts a Scene-view frame through an editor-owned world camera.
    ///
    /// The authored world camera is deliberately not consulted. Screen-space
    /// content is viewport-owned as well, so moving, removing, or replacing a
    /// gameplay camera cannot move or disable either half of the Scene view.
    pub fn extract_animated_with_world_camera(
        &self,
        world: &World,
        viewport: Viewport,
        world_camera: ViewCamera,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
    ) -> Result<PreparedFrame, SceneExtractError> {
        let aspect = viewport.aspect_ratio()?;
        let mut cameras = resolved_screen_overlay(aspect);
        cameras.world = Some(ResolvedCamera {
            view: world_camera.view,
            view_projection: world_camera.view_projection,
            framed_half_height: world_camera.framed_half_height,
        });

        let mut frame = ExtractedFrame::new(viewport, ClearOperations::default());
        self.push_meshes(world, &cameras, textures, &mut frame)?;
        self.push_sprites(world, &cameras, textures, animations, &mut frame)?;
        self.push_text(world, viewport, &cameras, &mut frame)?;
        Ok(frame.prepare()?)
    }
}
