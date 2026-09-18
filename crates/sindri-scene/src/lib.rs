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
mod generation;
mod navigation;
mod occlusion;
mod physics;
mod physics_sync;
mod placement;
pub(crate) mod screen_ui;
mod textures;
mod tile_surface;
mod tilesets;

pub use animation::{AnimationClip, AnimationError, SpriteAnimationComponent, SpriteAnimations};
pub use audio::AudioSourceComponent;
pub use camera_math::camera_rotation_from_look_at;
pub use components::{
    CameraComponent, CameraFit, GridNavigationComponent, GridOccupantComponent,
    GridPlacementComponent, GridWallDocument, MeshComponent, MeshPrimitive, ShapeComponent,
    ShapeGeometry, SpriteColorTransform, SpriteComponent, TileCellDocument, TileDraw,
    TileGridComponent, TileGridError, TileProjection, TileSpace, TileVolumeComponent,
    TileVolumeError, TileVolumeIndex, TilemapComponent, TilemapError, UiAnchor, UiFill, UiFillEdge,
    UiImageComponent, UiShapeBlend, UiShapeComponent, UiShapeKind, UiTextAutoSize, UiTextCase,
    UiTextComponent, UiTextLineAlign, UiTextOutline, UiTextShadow, UiTextWrap, cell_to_local_in,
    ui_text_template,
};
pub use effects::{EffectBurstComponent, Effects2d, Fleck};
pub use extract::{
    CameraView, OverlayPlacement, OverlayView, SceneExtractError, SceneExtractor, SceneRuntime,
    UiCanvas, ViewCamera, WorldProjection, overlay_for_viewport, overlay_in_scene, world_camera_of,
};
pub use navigation::{GridNavigationError, GridPlacement, WorldGridNavigation};
pub use occlusion::{
    OcclusionError, OcclusionFinding, OcclusionProbe, OcclusionReport, sweep_occlusion,
};
pub use physics::{Collider2dComponent, RigidBody2dComponent, RigidBodyKind};
pub use physics_sync::{PhysicsSyncError, ScenePhysics2d};
pub use placement::{
    GridPlacementError, GridSurfaces, blocking_step_ahead, nearest_cell, resolve_grid_placements,
    standing_depth,
};
pub use screen_ui::{
    SafeArea, ScreenExtent, ScreenRect, ScreenUi, UiButtonComponent, UiDirection, UiHierarchy,
    UiLayoutComponent, UiPlaced, UiSliderComponent, UiSliderOrientation,
};
pub use textures::{
    FONT_NAMING_COMPONENTS, PROCEDURAL_TEXTURES, ProceduralTexture, SheetBindError,
    TEXTURE_NAMING_COMPONENTS, TextureBindings, referenced_fonts, referenced_sheets,
    referenced_textures, unresolved_sprites, unresolved_textures,
};
pub use tile_surface::{TileSurfaceError, TileSurfaces};
pub mod voxel;
pub use voxel::{VoxelError, VoxelFace, VoxelHit, cube_faces, face_quad, pick};

pub use tilesets::{TileSetBindings, referenced_tile_sets, tile_set_sheets, tile_set_textures};
