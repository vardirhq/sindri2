//! The seam between a simulated world and a drawn frame.
//!
//! `sindri-render` and `sindri-grid` deliberately know nothing about worlds,
//! components, or scenes, and `sindri-core` knows nothing about drawing or
//! grid gameplay. This crate owns the built-in `sindri.*` component schemas and
//! adapts a world into the derived forms those neutral crates consume: ordered
//! render frames and validated navigation snapshots.

mod animation;
mod components;
mod extract;
mod navigation;
mod textures;

pub use animation::{AnimationClip, AnimationError, SpriteAnimationComponent, SpriteAnimations};
pub use components::{
    CameraComponent, GridNavigationComponent, GridOccupantComponent, GridWallDocument,
    MeshComponent, MeshPrimitive, SpriteAnchor, SpriteComponent, SpriteSpace, TextComponent,
    TileProjection, TilemapComponent, TilemapError,
};
pub use extract::{CameraView, SceneExtractError, SceneExtractor, ViewCamera, WorldProjection};
pub use navigation::{GridNavigationError, GridPlacement, WorldGridNavigation};
pub use textures::{
    FONT_NAMING_COMPONENTS, PROCEDURAL_TEXTURES, ProceduralTexture, SheetBindError,
    TEXTURE_NAMING_COMPONENTS, TextureBindings, referenced_fonts, referenced_sheets,
    referenced_textures, unresolved_sprites, unresolved_textures,
};
