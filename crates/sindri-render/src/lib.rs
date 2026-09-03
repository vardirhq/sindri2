//! Target-independent Sindri rendering building blocks.

mod camera;
mod color;
mod cube;
mod depth;
mod encode;
mod frame;
mod glyph;
mod mesh;
mod offscreen;
mod sprite;
mod sprite_batch;
mod target;
mod text;
mod texture;
mod textured_cube;
mod transparency;
mod triangle;
mod uv_rect;

pub use camera::{
    OrthographicCamera, PerspectiveCamera, look_at, orthographic_projection, perspective_projection,
};
pub use color::{COLOR_TARGET_FORMAT, ColorSpaceError, require_srgb_target};
pub use cube::CubeRenderer;
pub use depth::DepthTarget;
pub use encode::{FrameEncodeError, FrameRenderers, FrameTarget, encode_prepared_frame};
pub use frame::{
    ClearOperations, ExtractedFrame, FrameCamera, FrameCommand, FramePass, FramePlanError,
    PreparedFrame, RenderLayer, RenderStage, Viewport,
};
pub use glyph::{GlyphAtlas, GlyphSlot, RASTER_EM};
pub use mesh::{ColoredVertex, MeshBuffers, TexturedVertex};
pub use offscreen::{OffscreenError, OffscreenReadback, OffscreenTarget};
pub use sprite::{SpriteBlendMode, SpriteRenderer};
pub use sprite_batch::{
    SpriteBatchError, SpriteBatchRenderer, SpriteBatchStats, SpriteDepth, SpriteInstance,
};
pub use target::{ViewportTarget, encode_clear, sampled_format};
pub use text::{GlyphQuads, TextAlign, TextError, TextInstance, TextRenderer, aligned_origin};
pub use texture::{Texture2D, TextureError, TextureFilter, TextureId, TextureRegistry};
pub use textured_cube::{DrawContext, TexturedCubeRenderer};
pub use transparency::{TransparentOrder, TransparentOrderError};
pub use triangle::TriangleRenderer;
pub use uv_rect::{UvRect, UvRectError};
