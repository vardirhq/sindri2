use crate::{
    DefaultVoxelMaterials, RenderClass, SECTION_EDGE, SectionCoord, VoxelCoord, VoxelId,
    VoxelMaterialSource, VoxelSource,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

    const fn normal(self) -> [i8; 3] {
        match self {
            Self::Left => [-1, 0, 0],
            Self::Right => [1, 0, 0],
            Self::Bottom => [0, -1, 0],
            Self::Top => [0, 1, 0],
            Self::Back => [0, 0, -1],
            Self::Front => [0, 0, 1],
        }
    }

    const fn corners(self, [x, y, z]: [u8; 3]) -> [[u8; 3]; 4] {
        let right = x + 1;
        let top = y + 1;
        let front = z + 1;
        match self {
            Self::Left => [[x, y, z], [x, y, front], [x, top, front], [x, top, z]],
            Self::Right => [
                [right, y, front],
                [right, y, z],
                [right, top, z],
                [right, top, front],
            ],
            Self::Bottom => [
                [x, y, front],
                [x, y, z],
                [right, y, z],
                [right, y, front],
            ],
            Self::Top => [
                [x, top, z],
                [x, top, front],
                [right, top, front],
                [right, top, z],
            ],
            Self::Back => [
                [right, y, z],
                [x, y, z],
                [x, top, z],
                [right, top, z],
            ],
            Self::Front => [
                [x, y, front],
                [right, y, front],
                [right, top, front],
                [x, top, front],
            ],
        }
    }
}

/// One renderer-consumable vertex using section-local coordinates.
///
/// UV values are unit-square corners. A render bridge maps the semantic
/// material and face identities to an atlas, array texture, or custom shader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockVertex {
    pub position: [u8; 3],
    pub normal: [i8; 3],
    pub uv: [u8; 2],
    pub material: VoxelId,
    pub face: VoxelFace,
}

/// Indexed geometry for one render class.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockMeshPart {
    pub vertices: Vec<BlockVertex>,
    pub indices: Vec<u32>,
}

impl BlockMeshPart {
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.indices.len() / 6
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    fn push_face(&mut self, local: [u8; 3], material: VoxelId, face: VoxelFace) {
        const UVS: [[u8; 2]; 4] = [[0, 0], [1, 0], [1, 1], [0, 1]];
        let base =
            u32::try_from(self.vertices.len()).expect("one section mesh fits in u32 indices");
        for (position, uv) in face.corners(local).into_iter().zip(UVS) {
            self.vertices.push(BlockVertex {
                position,
                normal: face.normal(),
                uv,
                material,
                face,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// World-space bounds of a compiled section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionBounds {
    pub min: VoxelCoord,
    pub max_exclusive: VoxelCoord,
}

impl SectionBounds {
    #[must_use]
    pub fn from_section(section: SectionCoord) -> Self {
        let min = section.min_voxel();
        Self {
            min,
            max_exclusive: VoxelCoord::new(
                min.x + SECTION_EDGE,
                min.y + SECTION_EDGE,
                min.z + SECTION_EDGE,
            ),
        }
    }
}

/// CPU-side compiled block geometry, split into renderer passes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockMesh {
    pub section: SectionCoord,
    pub bounds: SectionBounds,
    pub opaque: BlockMeshPart,
    pub cutout: BlockMeshPart,
    pub transparent: BlockMeshPart,
}

impl BlockMesh {
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.opaque.face_count() + self.cutout.face_count() + self.transparent.face_count()
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.opaque.triangle_count()
            + self.cutout.triangle_count()
            + self.transparent.triangle_count()
    }

    #[must_use]
    pub const fn part(&self, render_class: RenderClass) -> &BlockMeshPart {
        match render_class {
            RenderClass::Opaque => &self.opaque,
            RenderClass::Cutout => &self.cutout,
            RenderClass::Transparent => &self.transparent,
        }
    }

    fn part_mut(&mut self, render_class: RenderClass) -> &mut BlockMeshPart {
        match render_class {
            RenderClass::Opaque => &mut self.opaque,
            RenderClass::Cutout => &mut self.cutout,
            RenderClass::Transparent => &mut self.transparent,
        }
    }
}

/// Compiles exposed block faces using the default all-opaque material mapping.
#[must_use]
pub fn mesh_block_section(source: &impl VoxelSource, section: SectionCoord) -> BlockMesh {
    mesh_block_section_with_materials(source, &DefaultVoxelMaterials, section)
}

/// Compiles one section using game-provided material and face-occlusion policy.
///
/// Neighbours are queried through `VoxelSource`, including coordinates outside
/// the target section. Geometry remains section-local while `SectionBounds`
/// provides the world-space extent used by render caches and culling.
#[must_use]
pub fn mesh_block_section_with_materials(
    source: &impl VoxelSource,
    materials: &impl VoxelMaterialSource,
    section: SectionCoord,
) -> BlockMesh {
    let min = section.min_voxel();
    let mut mesh = BlockMesh {
        section,
        bounds: SectionBounds::from_section(section),
        opaque: BlockMeshPart::default(),
        cutout: BlockMeshPart::default(),
        transparent: BlockMeshPart::default(),
    };
    for y in 0..SECTION_EDGE {
        for z in 0..SECTION_EDGE {
            for x in 0..SECTION_EDGE {
                let voxel = VoxelCoord::new(min.x + x, min.y + y, min.z + z);
                let voxel_id = source.voxel(voxel);
                if voxel_id.is_air() {
                    continue;
                }
                let material = materials.material(voxel_id);
                let local = [
                    u8::try_from(x).expect("section x fits in u8"),
                    u8::try_from(y).expect("section y fits in u8"),
                    u8::try_from(z).expect("section z fits in u8"),
                ];
                for face in VoxelFace::ALL {
                    let (dx, dy, dz) = face.offset();
                    let neighbour_coord =
                        VoxelCoord::new(voxel.x + dx, voxel.y + dy, voxel.z + dz);
                    let neighbour = source.voxel(neighbour_coord);
                    let visible = neighbour.is_air()
                        || !materials
                            .material(neighbour)
                            .blocks_face(neighbour, voxel_id);
                    if visible {
                        mesh.part_mut(material.render_class)
                            .push_face(local, voxel_id, face);
                    }
                }
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FaceOcclusion, VoxelMaterial};

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
            if matches!(
                coord.section(),
                SectionCoord {
                    x: 0 | 1,
                    y: 0,
                    z: 0
                }
            ) {
                VoxelId::new(3)
            } else {
                VoxelId::AIR
            }
        }
    }

    struct Pair {
        right: VoxelId,
    }

    impl VoxelSource for Pair {
        fn voxel(&self, coord: VoxelCoord) -> VoxelId {
            match (coord.x, coord.y, coord.z) {
                (0, 0, 0) => VoxelId::new(4),
                (1, 0, 0) => self.right,
                _ => VoxelId::AIR,
            }
        }
    }

    struct TestMaterials;

    impl VoxelMaterialSource for TestMaterials {
        fn material(&self, voxel: VoxelId) -> VoxelMaterial {
            match voxel.value() {
                1 => VoxelMaterial::opaque(),
                2 => VoxelMaterial::cutout(),
                3 | 4 | 5 => VoxelMaterial::transparent(),
                _ => VoxelMaterial::new(RenderClass::Opaque, FaceOcclusion::Solid),
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
        assert_eq!(mesh.triangle_count(), 12 * 16 * 16);
        assert_eq!(mesh.opaque.vertices.len(), mesh.face_count() * 4);
        assert_eq!(mesh.opaque.indices.len(), mesh.face_count() * 6);
    }

    #[test]
    fn adjacent_sections_do_not_emit_internal_boundary_faces() {
        let left = mesh_block_section(&TwoSections, SectionCoord::new(0, 0, 0));
        let right = mesh_block_section(&TwoSections, SectionCoord::new(1, 0, 0));
        assert_eq!(left.face_count(), 5 * 16 * 16);
        assert_eq!(right.face_count(), 5 * 16 * 16);
        assert!(!left.opaque.vertices.iter().any(|vertex| {
            vertex.position[0] == 16 && vertex.face == VoxelFace::Right
        }));
        assert!(!right.opaque.vertices.iter().any(|vertex| {
            vertex.position[0] == 0 && vertex.face == VoxelFace::Left
        }));
    }

    #[test]
    fn transparent_material_suppresses_only_matching_internal_faces() {
        let matching = mesh_block_section_with_materials(
            &Pair {
                right: VoxelId::new(4),
            },
            &TestMaterials,
            SectionCoord::new(0, 0, 0),
        );
        assert_eq!(matching.transparent.face_count(), 10);

        let different = mesh_block_section_with_materials(
            &Pair {
                right: VoxelId::new(5),
            },
            &TestMaterials,
            SectionCoord::new(0, 0, 0),
        );
        assert_eq!(different.transparent.face_count(), 12);
    }

    #[test]
    fn geometry_is_split_by_render_class() {
        struct ThreeBlocks;

        impl VoxelSource for ThreeBlocks {
            fn voxel(&self, coord: VoxelCoord) -> VoxelId {
                match (coord.x, coord.y, coord.z) {
                    (0, 0, 0) => VoxelId::new(1),
                    (2, 0, 0) => VoxelId::new(2),
                    (4, 0, 0) => VoxelId::new(3),
                    _ => VoxelId::AIR,
                }
            }
        }

        let mesh = mesh_block_section_with_materials(
            &ThreeBlocks,
            &TestMaterials,
            SectionCoord::new(0, 0, 0),
        );
        assert_eq!(mesh.opaque.face_count(), 6);
        assert_eq!(mesh.cutout.face_count(), 6);
        assert_eq!(mesh.transparent.face_count(), 6);
    }

    #[test]
    fn section_bounds_are_world_space_for_negative_sections() {
        let bounds = SectionBounds::from_section(SectionCoord::new(-2, 3, -1));
        assert_eq!(bounds.min, VoxelCoord::new(-32, 48, -16));
        assert_eq!(bounds.max_exclusive, VoxelCoord::new(-16, 64, 0));
    }
}
