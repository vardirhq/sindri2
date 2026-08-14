//! Target-independent Sindri rendering building blocks.

mod camera;
mod cube;
mod depth;
mod mesh;
mod offscreen;
mod sprite;
mod sprite_batch;
mod texture;
mod textured_cube;
mod transparency;
mod triangle;

pub use camera::{OrthographicCamera, PerspectiveCamera};
pub use cube::CubeRenderer;
pub use depth::DepthTarget;
pub use mesh::{ColoredVertex, MeshBuffers, TexturedVertex};
pub use offscreen::{OffscreenError, OffscreenReadback, OffscreenTarget};
pub use sprite::{SpriteBlendMode, SpriteRenderer};
pub use sprite_batch::{
    SpriteBatchError, SpriteBatchRenderer, SpriteBatchStats, SpriteInstance,
};
pub use texture::{Texture2D, TextureError};
pub use textured_cube::TexturedCubeRenderer;
pub use transparency::{TransparentOrder, TransparentOrderError};
pub use triangle::TriangleRenderer;
