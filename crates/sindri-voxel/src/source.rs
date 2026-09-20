use crate::{SECTION_EDGE, SectionCoord, VoxelCoord, VoxelId, VoxelSection};

/// Deterministic random access to an authoritative voxel world.
///
/// Meshing may sample just outside a resident section to decide whether a
/// boundary face is visible. That neighbour query must not require the
/// neighbour to be resident or rendered.
pub trait VoxelSource {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId;

    fn generate_section(&self, section: SectionCoord) -> VoxelSection {
        let mut result = VoxelSection::default();
        let min = section.min_voxel();
        for y in 0..SECTION_EDGE {
            for z in 0..SECTION_EDGE {
                for x in 0..SECTION_EDGE {
                    let world = VoxelCoord::new(min.x + x, min.y + y, min.z + z);
                    result.set(world.local(), self.voxel(world));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HalfSpace;

    impl VoxelSource for HalfSpace {
        fn voxel(&self, coord: VoxelCoord) -> VoxelId {
            if coord.y < 0 {
                VoxelId::new(1)
            } else {
                VoxelId::AIR
            }
        }
    }

    #[test]
    fn section_generation_is_deterministic_and_coordinate_driven() {
        let source = HalfSpace;
        let below = source.generate_section(SectionCoord::new(3, -1, -2));
        let above = source.generate_section(SectionCoord::new(3, 0, -2));
        assert_eq!(
            below.get(crate::LocalVoxelCoord::new(7, 15, 9)),
            VoxelId::new(1)
        );
        assert_eq!(
            above.get(crate::LocalVoxelCoord::new(7, 0, 9)),
            VoxelId::AIR
        );
        assert_eq!(below, source.generate_section(SectionCoord::new(3, -1, -2)));
    }
}
