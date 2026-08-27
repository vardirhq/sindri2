//! Deriving what to draw from what the world holds.
//!
//! Gameplay writes the world; nothing here does. Each component family
//! has a file that turns it into frame commands, and `extract` is the
//! order those run in. A new drawable component is a file beside them
//! and one `push_` call here.

mod camera;
mod mesh;
mod registry;
mod sprite;
mod text;
mod tilemap;
mod ui;

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

use crate::{AnimationError, SpriteAnimations, TextureBindings, TilemapError};

pub use camera::view::{CameraView, WorldProjection};
pub use camera::{OverlayPlacement, OverlayView, ViewCamera, overlay_for_viewport};

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

use registry::builtin_components;

impl SceneExtractor {
    /// Registers the built-in `sindri.*` components.
    pub fn new() -> Result<Self, SceneExtractError> {
        Ok(Self {
            components: builtin_components()?,
        })
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

    /// Registers an additional component type along with every field it has.
    ///
    /// Preferred over [`Self::register`] for anything an editor will draw: the
    /// field template is what lets a panel show the same rows for one of these
    /// however it was authored. The registry checks the template against what
    /// serde will ask the type for, so one that drifts is a registration error
    /// rather than a panel quietly missing a row.
    pub fn register_with_fields<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
        fields: serde_json::Value,
    ) -> Result<(), SceneExtractError> {
        self.components
            .register_with_fields::<T>(display_name, fields)?;
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
        self.push_images(world, &cameras, textures, animations, &mut frame)?;
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
        self.push_images(world, &cameras, textures, animations, &mut frame)?;
        self.push_text(world, viewport, &cameras, &mut frame)?;
        Ok(frame.prepare()?)
    }
}
