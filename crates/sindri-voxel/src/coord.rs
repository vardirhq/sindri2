/// One section is deliberately Minecraft-sized: small enough to rebuild after
/// a local edit and large enough that its mesh amortises draw overhead.
pub const SECTION_EDGE: i32 = 16;
pub const SECTION_VOLUME: usize = 16 * 16 * 16;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VoxelCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl VoxelCoord {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub fn section(self) -> SectionCoord {
        SectionCoord::new(
            self.x.div_euclid(SECTION_EDGE),
            self.y.div_euclid(SECTION_EDGE),
            self.z.div_euclid(SECTION_EDGE),
        )
    }

    #[must_use]
    pub fn local(self) -> LocalVoxelCoord {
        LocalVoxelCoord::new(
            u8::try_from(self.x.rem_euclid(SECTION_EDGE)).expect("local x is 0..16"),
            u8::try_from(self.y.rem_euclid(SECTION_EDGE)).expect("local y is 0..16"),
            u8::try_from(self.z.rem_euclid(SECTION_EDGE)).expect("local z is 0..16"),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SectionCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SectionCoord {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub const fn min_voxel(self) -> VoxelCoord {
        VoxelCoord::new(
            self.x * SECTION_EDGE,
            self.y * SECTION_EDGE,
            self.z * SECTION_EDGE,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct LocalVoxelCoord {
    x: u8,
    y: u8,
    z: u8,
}

impl LocalVoxelCoord {
    #[must_use]
    pub fn new(x: u8, y: u8, z: u8) -> Self {
        assert!(
            i32::from(x) < SECTION_EDGE
                && i32::from(y) < SECTION_EDGE
                && i32::from(z) < SECTION_EDGE,
            "local voxel coordinates must fit one section"
        );
        Self { x, y, z }
    }

    #[must_use]
    pub const fn x(self) -> u8 {
        self.x
    }
    #[must_use]
    pub const fn y(self) -> u8 {
        self.y
    }
    #[must_use]
    pub const fn z(self) -> u8 {
        self.z
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.x as usize
            + self.z as usize * SECTION_EDGE as usize
            + self.y as usize * SECTION_EDGE as usize * SECTION_EDGE as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_world_coordinates_land_in_the_expected_section() {
        let voxel = VoxelCoord::new(-1, -16, -17);
        assert_eq!(voxel.section(), SectionCoord::new(-1, -1, -2));
        assert_eq!(voxel.local(), LocalVoxelCoord::new(15, 0, 15));
    }

    #[test]
    fn section_minimum_round_trips() {
        let section = SectionCoord::new(-7, 3, 12);
        let minimum = section.min_voxel();
        assert_eq!(minimum.section(), section);
        assert_eq!(minimum.local(), LocalVoxelCoord::new(0, 0, 0));
    }
}
