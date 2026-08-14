//! Target-independent Sindri rendering building blocks.

mod camera;
mod cube;
mod depth;
mod mesh;
mod triangle;

pub use camera::PerspectiveCamera;
pub use cube::CubeRenderer;
pub use depth::DepthTarget;
pub use mesh::{ColoredVertex, MeshBuffers};
pub use triangle::TriangleRenderer;
