use crate::{CachedMeshId, CachedTexturedMeshUpload, MeshSurface, TextureId, TextureRegistry};

/// The GPU handles and texture a draw resolves against.
///
/// Bundled because a draw needs all of them together, and threading four more
/// parameters through every encode call obscures what is actually being drawn.
#[derive(Clone, Copy)]
pub struct DrawContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub textures: &'a TextureRegistry,
    pub texture: TextureId,
}

#[derive(Clone, Copy)]
pub(crate) struct CachedMeshRequest<'a> {
    pub id: CachedMeshId,
    pub revision: u64,
    pub replacement: Option<&'a CachedTexturedMeshUpload>,
    pub surface: MeshSurface,
}
