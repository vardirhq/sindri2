//! Target-independent Sindri rendering building blocks.

mod camera;
mod cube;
mod depth;
mod mesh;
mod offscreen;
mod texture;
mod textured_cube;
mod triangle;

pub use camera::PerspectiveCamera;
pub use cube::CubeRenderer;
pub use depth::DepthTarget;
pub use mesh::{ColoredVertex, MeshBuffers, TexturedVertex};
pub use offscreen::{OffscreenError, OffscreenReadback, OffscreenTarget};
pub use texture::{Texture2D, TextureError};
pub use textured_cube::TexturedCubeRenderer;
pub use triangle::TriangleRenderer;
