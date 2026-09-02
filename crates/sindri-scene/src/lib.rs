//! The seam between a simulated world and a drawn frame.
//!
//! `sindri-render` and `sindri-grid` deliberately know nothing about worlds,
//! components, or scenes, and `sindri-core` knows nothing about drawing or
//! grid gameplay. This crate owns the built-in `sindri.*` component schemas and
//! adapts a world into the derived forms those neutral crates consume: ordered
//! render frames and validated navigation snapshots.

mod animation;
mod audio;
mod camera_math;
mod components;
pub(crate) mod effects;
mod extract;
mod navigation;
mod physics;
mod physics_sync;
pub(crate) mod screen_ui;
mod textures;

pub use animation::{AnimationClip, AnimationError, SpriteAnimationComponent, SpriteAnimations};
pub use audio::AudioSourceComponent;
pub use camera_math::camera_rotation_from_look_at;
pub use components::{
    CameraComponent, GridNavigationComponent, GridOccupantComponent, GridWallDocument,
    MeshComponent, MeshPrimitive, SpriteComponent, TileProjection, TilemapComponent, TilemapError,
    UiAnchor, UiFill, UiFillEdge, UiImageComponent, UiTextComponent, ui_text_template,
};
pub use effects::{EffectBurstComponent, Effects2d, Fleck};
pub use extract::{
    CameraView, OverlayPlacement, OverlayView, SceneExtractError, SceneExtractor, SceneRuntime,
    ViewCamera, WorldProjection, overlay_for_viewport,
};
pub use navigation::{GridNavigationError, GridPlacement, WorldGridNavigation};
pub use physics::{Collider2dComponent, RigidBody2dComponent, RigidBodyKind};
pub use physics_sync::{PhysicsSyncError, ScenePhysics2d};
pub use screen_ui::{
    PointerFrame, SafeArea, ScreenExtent, ScreenRect, ScreenUi, UiButtonComponent, UiDirection,
    UiLayoutComponent,
};
pub use textures::{
    FONT_NAMING_COMPONENTS, PROCEDURAL_TEXTURES, ProceduralTexture, SheetBindError,
    TEXTURE_NAMING_COMPONENTS, TextureBindings, referenced_fonts, referenced_sheets,
    referenced_textures, unresolved_sprites, unresolved_textures,
};
