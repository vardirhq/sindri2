use std::collections::BTreeMap;

use sindri_render::{CachedTexturedMeshUpload, TextureId, TexturedVertex, UvRect};
use sindri_voxel::{
    BlockMesh, BlockMeshPart, RenderClass, SectionBounds, SectionCoord, VoxelFace, VoxelId,
};
use thiserror::Error;

/// Renderer texture and atlas region for one semantic voxel face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelTexture {
    pub texture: TextureId,
    pub uv: UvRect,
}

impl VoxelTexture {
    #[must_use]
    pub const fn new(texture: TextureId, uv: UvRect) -> Self {
        Self { texture, uv }
    }
}

/// Resolves semantic voxel and face identities at the scene/render boundary.
pub trait VoxelTextureSource {
    fn texture(&self, voxel: VoxelId, face: VoxelFace) -> VoxelTexture;
}

impl<F> VoxelTextureSource for F
where
    F: Fn(VoxelId, VoxelFace) -> VoxelTexture,
{
    fn texture(&self, voxel: VoxelId, face: VoxelFace) -> VoxelTexture {
        self(voxel, face)
    }
}

/// One texture-homogeneous renderer batch from a compiled voxel section.
#[derive(Clone, Debug)]
pub struct CompiledVoxelBatch {
    pub render_class: RenderClass,
    pub texture: TextureId,
    pub upload: CachedTexturedMeshUpload,
}

impl CompiledVoxelBatch {
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.upload.indices.len() / 3
    }
}

/// Renderer-ready geometry and bounds for one voxel section.
#[derive(Clone, Debug)]
pub struct CompiledVoxelSection {
    pub section: SectionCoord,
    pub bounds: SectionBounds,
    pub batches: Vec<CompiledVoxelBatch>,
}

impl CompiledVoxelSection {
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.batches
            .iter()
            .map(CompiledVoxelBatch::triangle_count)
            .sum()
    }
}

/// Maps semantic block geometry onto renderer textures without GPU access.
pub fn compile_block_mesh(
    mesh: &BlockMesh,
    textures: &impl VoxelTextureSource,
) -> Result<CompiledVoxelSection, VoxelRenderError> {
    let mut batches = Vec::new();
    for render_class in [
        RenderClass::Opaque,
        RenderClass::Cutout,
        RenderClass::Transparent,
    ] {
        batches.extend(compile_part(
            mesh.part(render_class),
            render_class,
            textures,
        )?);
    }
    Ok(CompiledVoxelSection {
        section: mesh.section,
        bounds: mesh.bounds,
        batches,
    })
}

fn compile_part(
    part: &BlockMeshPart,
    render_class: RenderClass,
    textures: &impl VoxelTextureSource,
) -> Result<Vec<CompiledVoxelBatch>, VoxelRenderError> {
    if !part.indices.len().is_multiple_of(6) {
        return Err(VoxelRenderError::IncompleteFace {
            render_class,
            indices: part.indices.len(),
        });
    }
    let mut uploads: BTreeMap<TextureId, CachedTexturedMeshUpload> = BTreeMap::new();
    for face_indices in part.indices.chunks_exact(6) {
        let first = vertex(part, face_indices[0], render_class)?;
        let mapping = textures.texture(first.material, first.face);
        let upload = uploads.entry(mapping.texture).or_default();
        let mut remapped = BTreeMap::new();
        for source_index in face_indices {
            let target_index = if let Some(index) = remapped.get(source_index) {
                *index
            } else {
                let source = vertex(part, *source_index, render_class)?;
                if textures.texture(source.material, source.face) != mapping {
                    return Err(VoxelRenderError::FaceSpansTextures { render_class });
                }
                let index = u32::try_from(upload.vertices.len())
                    .expect("one section's renderer vertices fit in u32");
                upload.vertices.push(TexturedVertex::new(
                    source.position.map(f32::from),
                    [
                        mapping
                            .uv
                            .width()
                            .mul_add(f32::from(source.uv[0]), mapping.uv.x()),
                        mapping
                            .uv
                            .height()
                            .mul_add(f32::from(source.uv[1]), mapping.uv.y()),
                    ],
                ));
                remapped.insert(*source_index, index);
                index
            };
            upload.indices.push(target_index);
        }
    }
    Ok(uploads
        .into_iter()
        .map(|(texture, upload)| CompiledVoxelBatch {
            render_class,
            texture,
            upload,
        })
        .collect())
}

fn vertex(
    part: &BlockMeshPart,
    index: u32,
    render_class: RenderClass,
) -> Result<&sindri_voxel::BlockVertex, VoxelRenderError> {
    let index = usize::try_from(index).expect("u32 fits usize on supported targets");
    part.vertices
        .get(index)
        .ok_or(VoxelRenderError::IndexOutOfBounds {
            render_class,
            index,
            vertices: part.vertices.len(),
        })
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum VoxelRenderError {
    #[error("{render_class:?} voxel geometry has {indices} indices, not complete six-index faces")]
    IncompleteFace {
        render_class: RenderClass,
        indices: usize,
    },
    #[error("{render_class:?} voxel index {index} is outside {vertices} vertices")]
    IndexOutOfBounds {
        render_class: RenderClass,
        index: usize,
        vertices: usize,
    },
    #[error("one {render_class:?} voxel face resolves to more than one texture region")]
    FaceSpansTextures { render_class: RenderClass },
    #[error("mesh for section {actual:?} completed job for {expected:?}")]
    SectionMismatch {
        expected: SectionCoord,
        actual: SectionCoord,
    },
    #[error("persistent voxel rendering does not support non-opaque {0:?} batches yet")]
    UnsupportedRenderClass(RenderClass),
}
