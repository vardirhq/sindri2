use crate::{
    FaceOcclusion, RenderClass, SectionBounds, SectionCoord, VoxelCoord, VoxelFace, VoxelId,
    VoxelMaterial, VoxelMaterialSource, VoxelSource, mesh_block_section,
    mesh_block_section_with_materials,
};

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
            3..=5 => VoxelMaterial::transparent(),
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
    assert!(
        !left
            .opaque
            .vertices
            .iter()
            .any(|vertex| { vertex.position[0] == 16 && vertex.face == VoxelFace::Right })
    );
    assert!(
        !right
            .opaque
            .vertices
            .iter()
            .any(|vertex| { vertex.position[0] == 0 && vertex.face == VoxelFace::Left })
    );
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

    let mesh =
        mesh_block_section_with_materials(&ThreeBlocks, &TestMaterials, SectionCoord::new(0, 0, 0));
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

#[test]
fn triangle_winding_matches_each_vertex_normal() {
    let mesh = mesh_block_section(
        &Pair {
            right: VoxelId::AIR,
        },
        SectionCoord::new(0, 0, 0),
    );

    for quad in mesh.opaque.vertices.chunks_exact(4) {
        let edge_a = [
            i16::from(quad[1].position[0]) - i16::from(quad[0].position[0]),
            i16::from(quad[1].position[1]) - i16::from(quad[0].position[1]),
            i16::from(quad[1].position[2]) - i16::from(quad[0].position[2]),
        ];
        let edge_b = [
            i16::from(quad[2].position[0]) - i16::from(quad[0].position[0]),
            i16::from(quad[2].position[1]) - i16::from(quad[0].position[1]),
            i16::from(quad[2].position[2]) - i16::from(quad[0].position[2]),
        ];
        let cross = [
            edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
            edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
            edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
        ];
        assert_eq!(
            cross,
            quad[0].normal.map(i16::from),
            "winding differs for {:?}",
            quad[0].face
        );
    }
}

#[test]
fn face_uvs_use_a_top_left_texture_origin() {
    let mesh = mesh_block_section(
        &Pair {
            right: VoxelId::AIR,
        },
        SectionCoord::new(0, 0, 0),
    );
    for quad in mesh.opaque.vertices.chunks_exact(4) {
        assert_eq!(
            quad.iter().map(|vertex| vertex.uv).collect::<Vec<_>>(),
            [[0, 1], [1, 1], [1, 0], [0, 0]]
        );
    }
}


#[test]
fn voxel_corner_ao_samples_neighbours_outside_the_exposed_face() {
    struct Corner;

    impl VoxelSource for Corner {
        fn voxel(&self, coord: VoxelCoord) -> VoxelId {
            if matches!(
                (coord.x, coord.y, coord.z),
                (0, 0, 0) | (-1, 1, 0) | (0, 1, -1)
            ) {
                VoxelId::new(1)
            } else {
                VoxelId::AIR
            }
        }
    }

    let mesh = mesh_block_section(&Corner, SectionCoord::new(0, 0, 0));
    let top = mesh
        .opaque
        .vertices
        .iter()
        .find(|vertex| vertex.face == VoxelFace::Top && vertex.position == [0, 1, 0])
        .expect("target top corner is emitted");
    assert_eq!(top.ambient_occlusion, 0);

    let open = mesh
        .opaque
        .vertices
        .iter()
        .find(|vertex| vertex.face == VoxelFace::Top && vertex.position == [1, 1, 1])
        .expect("opposite top corner is emitted");
    assert_eq!(open.ambient_occlusion, 3);
}
