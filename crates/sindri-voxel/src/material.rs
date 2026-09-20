use crate::VoxelId;

/// Rendering pass used for geometry produced from a voxel material.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RenderClass {
    Opaque,
    Cutout,
    Transparent,
}

/// How a full-cube voxel hides a neighbouring voxel face.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FaceOcclusion {
    /// Hide every face touching this voxel.
    Solid,
    /// Hide only a face belonging to the same voxel identity.
    MatchingVoxel,
    /// Never hide a neighbouring face.
    None,
}

/// Renderer-independent description of one voxel material.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VoxelMaterial {
    pub render_class: RenderClass,
    pub face_occlusion: FaceOcclusion,
}

impl VoxelMaterial {
    #[must_use]
    pub const fn new(render_class: RenderClass, face_occlusion: FaceOcclusion) -> Self {
        Self {
            render_class,
            face_occlusion,
        }
    }

    #[must_use]
    pub const fn opaque() -> Self {
        Self::new(RenderClass::Opaque, FaceOcclusion::Solid)
    }

    #[must_use]
    pub const fn cutout() -> Self {
        Self::new(RenderClass::Cutout, FaceOcclusion::None)
    }

    #[must_use]
    pub const fn transparent() -> Self {
        Self::new(RenderClass::Transparent, FaceOcclusion::MatchingVoxel)
    }

    #[must_use]
    pub const fn blocks_face(self, voxel: VoxelId, neighbour: VoxelId) -> bool {
        match self.face_occlusion {
            FaceOcclusion::Solid => true,
            FaceOcclusion::MatchingVoxel => voxel.value() == neighbour.value(),
            FaceOcclusion::None => false,
        }
    }
}

/// Resolves a compact `VoxelId` into meshing policy.
///
/// Texture handles, atlas regions, shaders, and GPU bindings remain the
/// renderer's responsibility.
pub trait VoxelMaterialSource {
    fn material(&self, voxel: VoxelId) -> VoxelMaterial;
}

/// Baseline mapping used when a game has not supplied material policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultVoxelMaterials;

impl VoxelMaterialSource for DefaultVoxelMaterials {
    fn material(&self, _voxel: VoxelId) -> VoxelMaterial {
        VoxelMaterial::opaque()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_materials_express_expected_occlusion() {
        let glass = VoxelId::new(3);
        assert!(VoxelMaterial::opaque().blocks_face(glass, VoxelId::new(4)));
        assert!(VoxelMaterial::transparent().blocks_face(glass, glass));
        assert!(!VoxelMaterial::transparent().blocks_face(glass, VoxelId::new(4)));
        assert!(!VoxelMaterial::cutout().blocks_face(glass, glass));
    }
}
