use crate::{LocalVoxelCoord, SECTION_VOLUME};

/// Compact identity of a voxel material. Zero is permanently air.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct VoxelId(u16);

impl VoxelId {
    pub const AIR: Self = Self(0);

    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_air(self) -> bool {
        self.0 == 0
    }
}

/// Palette-backed storage for one 16³ voxel section.
///
/// Uniform sections keep no index array at all. The first differing write
/// expands to palette indices; repeated material ids remain one palette entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSection {
    palette: Vec<VoxelId>,
    indices: Option<Box<[u16; SECTION_VOLUME]>>,
    revision: u64,
}

impl Default for VoxelSection {
    fn default() -> Self {
        Self::uniform(VoxelId::AIR)
    }
}

impl VoxelSection {
    #[must_use]
    pub fn uniform(voxel: VoxelId) -> Self {
        Self {
            palette: vec![voxel],
            indices: None,
            revision: 0,
        }
    }

    #[must_use]
    pub fn get(&self, coord: LocalVoxelCoord) -> VoxelId {
        let palette_index = self
            .indices
            .as_ref()
            .map_or(0, |indices| usize::from(indices[coord.index()]));
        self.palette[palette_index]
    }

    pub fn set(&mut self, coord: LocalVoxelCoord, voxel: VoxelId) -> bool {
        if self.get(coord) == voxel {
            return false;
        }
        let palette_index =
            if let Some(index) = self.palette.iter().position(|entry| *entry == voxel) {
                index
            } else {
                self.palette.push(voxel);
                self.palette.len() - 1
            };
        let indices = self
            .indices
            .get_or_insert_with(|| Box::new([0; SECTION_VOLUME]));
        indices[coord.index()] =
            u16::try_from(palette_index).expect("u16 voxel ids bound the section palette");
        self.revision = self.revision.wrapping_add(1);
        true
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn palette_len(&self) -> usize {
        self.palette.len()
    }

    #[must_use]
    pub const fn is_uniform(&self) -> bool {
        self.indices.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uniform_section_stays_compact_until_it_changes() {
        let stone = VoxelId::new(1);
        let mut section = VoxelSection::uniform(stone);
        assert!(section.is_uniform());
        assert_eq!(section.palette_len(), 1);

        assert!(!section.set(LocalVoxelCoord::new(2, 3, 4), stone));
        assert_eq!(section.revision(), 0);
        assert!(section.is_uniform());

        let dirt = VoxelId::new(2);
        assert!(section.set(LocalVoxelCoord::new(2, 3, 4), dirt));
        assert_eq!(section.get(LocalVoxelCoord::new(2, 3, 4)), dirt);
        assert_eq!(section.get(LocalVoxelCoord::new(2, 3, 5)), stone);
        assert_eq!(section.palette_len(), 2);
        assert_eq!(section.revision(), 1);
        assert!(!section.is_uniform());
    }

    #[test]
    fn repeated_materials_reuse_the_palette() {
        let mut section = VoxelSection::default();
        let grass = VoxelId::new(7);
        section.set(LocalVoxelCoord::new(0, 0, 0), grass);
        section.set(LocalVoxelCoord::new(15, 15, 15), grass);
        assert_eq!(section.palette_len(), 2);
        assert_eq!(section.revision(), 2);
    }
}
