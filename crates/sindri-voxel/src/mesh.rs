use crate::{SectionCoord, VoxelCoord, VoxelId, VoxelSource, SECTION_EDGE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelFace {
    Left,
    Right,
    Bottom,
    Top,
    Back,
    Front,
}

impl VoxelFace {
    const ALL: [Self; 6] = [
        Self::Left,
        Self::Right,
        Self::Bottom,
        Self::Top,
        Self::Back,
        Self::Front,
    ];

    const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Left => (-1, 0, 0),
            Self::Right => (1, 0, 0),
            Self::Bottom => (0, -1, 0),
            Self::Top => (0, 1, 0),
            Self::Back => (0, 0, -1),
            Self::Front => (0, 0, 1),
        }
    }
}

/// One exposed block face in CPU-side compiled section geometry.
///
/// This intentionally carries semantic material/face identity rather than atlas
/// coordinates or GPU vertices. The render bridge decides how a material maps
/// to textures and may later greedy-merge compatible faces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockFace {
    pub voxel: VoxelCoord,
    pub material: VoxelId,
    pub face: VoxelFace,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockMesh {
    pub section: SectionCoord,
    pub faces: Vec<BlockFace>,
}

impl BlockMesh {
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
}

/// Baseline block mesher. Only exposed faces are emitted.
///
/// Neighbours are queried through VoxelSource, including coordinates outside
/// the target section. This is the one-voxel halo contract: correctness at a
/// boundary does not depend on the neighbouring section being resident.
#[must_use]
pub fn mesh_block_section(source: &impl VoxelSource, section: SectionCoord) -> BlockMesh {
    let min = section.min_voxel();
    let mut faces = Vec::new();
    for y in 0..SECTION_EDGE {
        for z in 0..SECTION_EDGE {
            for x in 0..SECTION_EDGE {
                let voxel = VoxelCoord::new(min.x + x, min.y + y, min.z + z);
                let material = source.voxel(voxel);
                if material.is_air() {
                    continue;
                }
                for face in VoxelFace::ALL {
                    let (dx, dy, dz) = face.offset();
                    let neighbour = VoxelCoord::new(voxel.x + dx, voxel.y + dy, voxel.z + dz);
                    if source.voxel(neighbour).is_air() {
                        faces.push(BlockFace {
                            voxel,
                            material,
                            face,
                        });
                    }
                }
            }
        }
    }
    BlockMesh { section, faces }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Solid;

    impl VoxelSource for Solid {
        fn voxel(&self, _coord: VoxelCoord) -> VoxelId {
            VoxelId::new(1)
        }
    }

    struct OneSection;

    impl VoxelSource for OneSection {
        fn voxel(&self, coord: VoxelCoord) -> VoxelId {
            if coord.section() == SectionCoord::new(0, 0, 0) {
                VoxelId::new(2)
            } else {
                VoxelId::AIR
            }
        }
    }

    struct TwoSections;

    impl VoxelSource for TwoSections {
        fn voxel(&self, coord: VoxelCoord) -> VoxelId {
            if matches!(coord.section(), SectionCoord { x: 0 | 1, y: 0, z: 0 }) {
                VoxelId::new(3)
            } else {
                VoxelId::AIR
            }
        }
    }

    #[test]
    fn an_infinite_solid_world_emits_no_faces() {
        assert_eq!(
            mesh_block_section(&Solid, SectionCoord::new(0, 0, 0)).face_count(),
            0
        );
    }

    #[test]
    fn isolated_solid_section_emits_only_its_exterior() {
        let mesh = mesh_block_section(&OneSection, SectionCoord::new(0, 0, 0));
        assert_eq!(mesh.face_count(), 6 * 16 * 16);
    }

    #[test]
    fn adjacent_sections_do_not_emit_internal_boundary_faces() {
        let left = mesh_block_section(&TwoSections, SectionCoord::new(0, 0, 0));
        let right = mesh_block_section(&TwoSections, SectionCoord::new(1, 0, 0));
        assert_eq!(left.face_count(), 5 * 16 * 16);
        assert_eq!(right.face_count(), 5 * 16 * 16);
        assert!(!left.faces.iter().any(|face| {
            face.voxel.x == 15 && face.face == VoxelFace::Right
        }));
        assert!(!right.faces.iter().any(|face| {
            face.voxel.x == 16 && face.face == VoxelFace::Left
        }));
    }
}
