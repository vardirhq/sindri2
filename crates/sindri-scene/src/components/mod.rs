//! The component types a scene may hold.
//!
//! One file per component family. A new component is a file here, its
//! re-export below, a schema registration in `SceneExtractor::new`, and
//! whatever `extract/` needs to draw it — and no existing file grows.

mod camera;
mod grid;
mod mesh;
mod sprite;
mod text;
mod tilemap;

pub use camera::CameraComponent;
pub use grid::{GridNavigationComponent, GridOccupantComponent, GridWallDocument};
pub use mesh::{MeshComponent, MeshPrimitive};
pub use sprite::{SpriteAnchor, SpriteComponent, SpriteSpace};
pub use text::TextComponent;
pub use tilemap::{TileProjection, TilemapComponent, TilemapError};
