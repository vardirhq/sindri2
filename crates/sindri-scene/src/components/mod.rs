//! The component types a scene may hold.
//!
//! One file per component family. A new component is a file here, its
//! re-export below, a schema registration in `SceneExtractor::new`, and
//! whatever `extract/` needs to draw it — and no existing file grows.
//!
//! The families are also where a scene's two kinds of entity come from. A
//! `sindri.ui.*` component draws on the viewport and knows nothing about a
//! camera; everything else is in the world. Nothing declares which kind an
//! entity is, because carrying one of these components already says it.

mod camera;
mod grid;
mod mesh;
mod sprite;
mod tilemap;
mod ui;
pub mod ui_text_template;

pub use camera::{CameraComponent, CameraFit};
pub use grid::{GridNavigationComponent, GridOccupantComponent, GridWallDocument};
pub use mesh::{MeshComponent, MeshPrimitive};
pub use sprite::SpriteComponent;
pub use tilemap::{TileProjection, TilemapComponent, TilemapError};
pub use ui::{UiAnchor, UiFill, UiFillEdge, UiImageComponent, UiTextComponent};

/// The tint a component that does not name one draws with.
///
/// Shared rather than repeated per component so that "no tint" means the same
/// thing everywhere it can be left out.
pub(super) const fn opaque_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}
