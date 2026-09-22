use crate::{
    DefaultVoxelMaterials, RenderClass, SECTION_EDGE, SectionCoord, VoxelCoord, VoxelId,
    VoxelMaterialSource, VoxelSource,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
            Self::Bottom => [[x, y, front], [x, y, z], [right, y, z], [right, y, front]],
            Self::Top => [
                [x, top, z],
                [x, top, front],
                [right, top, front],
                [right, top, z],
            ],
            Self::Back => [[right, y, z], [x, y, z], [x, top, z], [right, top, z]],
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
    /// 0..=3 open-neighbour samples around this corner, used for mesh-time AO.
    pub ambient_occlusion: u8,
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

    fn push_face(
        &mut self,
        local: [u8; 3],
        material: VoxelId,
        face: VoxelFace,
        ambient_occlusion: [u8; 4],
    ) {
        // Texture images use a top-left origin. Face corners begin along the
        // bottom edge, so V=1 belongs to the first two vertices.
        const UVS: [[u8; 2]; 4] = [[0, 1], [1, 1], [1, 0], [0, 0]];
        let base =
            u32::try_from(self.vertices.len()).expect("one section mesh fits in u32 indices");
        for ((position, uv), ambient_occlusion) in face
            .corners(local)
            .into_iter()
            .zip(UVS)
            .zip(ambient_occlusion)
        {
            self.vertices.push(BlockVertex {
                position,
                normal: face.normal(),
                uv,
                material,
                face,
                ambient_occlusion,
            });
        }
        if ambient_occlusion[0] + ambient_occlusion[2] > ambient_occlusion[1] + ambient_occlusion[3]
        {
            self.indices.extend_from_slice(&[
                base,
                base + 1,
                base + 3,
                base + 1,
                base + 2,
                base + 3,
            ]);
        } else {
            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
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
                    let neighbour_coord = VoxelCoord::new(voxel.x + dx, voxel.y + dy, voxel.z + dz);
                    let neighbour = source.voxel(neighbour_coord);
                    let visible = neighbour.is_air()
                        || !materials
                            .material(neighbour)
                            .blocks_face(neighbour, voxel_id);
                    if visible {
                        let ambient_occlusion =
                            face_ambient_occlusion(source, materials, voxel, voxel_id, face, local);
                        mesh.part_mut(material.render_class).push_face(
                            local,
                            voxel_id,
                            face,
                            ambient_occlusion,
                        );
                    }
                }
            }
        }
    }
    mesh
}

fn face_ambient_occlusion(
    source: &impl VoxelSource,
    materials: &impl VoxelMaterialSource,
    voxel: VoxelCoord,
    voxel_id: VoxelId,
    face: VoxelFace,
    local: [u8; 3],
) -> [u8; 4] {
    let normal = face.normal();
    face.corners(local).map(|corner| {
        let mut tangents = [[0_i32; 3]; 2];
        let mut next = 0;
        for axis in 0..3 {
            if normal[axis] == 0 {
                tangents[next][axis] = if corner[axis] == local[axis] { -1 } else { 1 };
                next += 1;
            }
        }
        let base = [
            voxel.x + i32::from(normal[0]),
            voxel.y + i32::from(normal[1]),
            voxel.z + i32::from(normal[2]),
        ];
        let occupied = |offset: [i32; 3]| {
            let neighbour = source.voxel(VoxelCoord::new(
                base[0] + offset[0],
                base[1] + offset[1],
                base[2] + offset[2],
            ));
            !neighbour.is_air()
                && materials
                    .material(neighbour)
                    .blocks_face(neighbour, voxel_id)
        };
        let side_a = occupied(tangents[0]);
        let side_b = occupied(tangents[1]);
        let corner_offset = [
            tangents[0][0] + tangents[1][0],
            tangents[0][1] + tangents[1][1],
            tangents[0][2] + tangents[1][2],
        ];
        let diagonal = occupied(corner_offset);
        if side_a && side_b {
            0
        } else {
            3 - u8::from(side_a) - u8::from(side_b) - u8::from(diagonal)
        }
    })
}
