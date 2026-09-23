use sindri_render::{FrameCommand, TextureId, UvRect};
use sindri_voxel::{
    MeshingProfile, SectionCoord, SectionMeshJob, SectionMeshKey, SectionMeshRevision, VoxelCoord,
    VoxelFace, VoxelId, VoxelSource, mesh_block_section,
};

use super::*;

struct OneBlock;

impl VoxelSource for OneBlock {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        if coord == VoxelCoord::new(0, 0, 0) {
            VoxelId::new(1)
        } else {
            VoxelId::AIR
        }
    }
}

fn job(revision: u64) -> SectionMeshJob {
    SectionMeshJob::new(
        SectionMeshKey::new(SectionCoord::new(0, 0, 0), MeshingProfile::Block),
        SectionMeshRevision::new(revision),
    )
}

fn texture(voxel: VoxelId, face: VoxelFace) -> VoxelTexture {
    assert_eq!(voxel, VoxelId::new(1));
    if face == VoxelFace::Top {
        VoxelTexture::new(
            TextureId::new(2),
            UvRect::cell(1, 0, 2, 1).expect("right atlas cell"),
        )
    } else {
        VoxelTexture::new(TextureId::new(1), UvRect::FULL)
    }
}

#[test]
fn semantic_faces_compile_into_deterministic_texture_batches() {
    let mesh = mesh_block_section(&OneBlock, SectionCoord::new(0, 0, 0));
    let compiled = compile_block_mesh(&mesh, &texture).expect("valid block geometry");

    assert_eq!(compiled.batches.len(), 2);
    assert_eq!(compiled.triangle_count(), 12);
    assert_eq!(compiled.batches[0].texture, TextureId::new(1));
    assert_eq!(compiled.batches[0].triangle_count(), 10);
    assert_eq!(compiled.batches[1].texture, TextureId::new(2));
    assert_eq!(compiled.batches[1].triangle_count(), 2);
    assert!(
        compiled.batches[1]
            .upload
            .vertices
            .iter()
            .all(|vertex| vertex.uv[0] >= 0.5 && vertex.uv[0] <= 1.0)
    );
}

#[test]
fn unchanged_sections_reuse_ids_and_only_new_revisions_carry_uploads() {
    let mesh = mesh_block_section(&OneBlock, SectionCoord::new(0, 0, 0));
    let mut bridge = VoxelRenderBridge::default();
    let first = job(1);
    assert!(bridge.schedule(first));
    assert!(
        bridge
            .finish(first, compile_block_mesh(&mesh, &texture).unwrap())
            .unwrap()
    );

    let initial = bridge.draw_commands(first.key, &|_| sindri_render::MeshSurface::default());
    let initial_ids = cached_ids(&initial);
    assert_eq!(initial_ids.len(), 2);
    assert!(initial.iter().all(has_replacement));
    assert!(
        bridge
            .draw_commands(first.key, &|_| sindri_render::MeshSurface::default())
            .iter()
            .all(no_replacement)
    );

    let replacement = job(2);
    assert!(bridge.schedule(replacement));
    assert!(
        bridge
            .finish(replacement, compile_block_mesh(&mesh, &texture).unwrap())
            .unwrap()
    );
    let replaced =
        bridge.draw_commands(replacement.key, &|_| sindri_render::MeshSurface::default());
    assert_eq!(cached_ids(&replaced), initial_ids);
    assert!(replaced.iter().all(has_replacement));
}

#[test]
fn superseded_results_never_allocate_or_upload_renderer_entries() {
    let mesh = mesh_block_section(&OneBlock, SectionCoord::new(0, 0, 0));
    let mut bridge = VoxelRenderBridge::default();
    let old = job(3);
    let current = job(4);
    assert!(bridge.schedule(old));
    assert!(bridge.schedule(current));
    assert!(
        !bridge
            .finish(old, compile_block_mesh(&mesh, &texture).unwrap())
            .unwrap()
    );
    assert_eq!(bridge.stats(), VoxelRenderStats::default());
}

#[test]
fn leaving_residency_releases_every_gpu_batch() {
    let mesh = mesh_block_section(&OneBlock, SectionCoord::new(0, 0, 0));
    let mut bridge = VoxelRenderBridge::default();
    let current = job(5);
    bridge.schedule(current);
    bridge
        .finish(current, compile_block_mesh(&mesh, &texture).unwrap())
        .unwrap();
    bridge.draw_commands(current.key, &|_| sindri_render::MeshSurface::default());

    bridge.remove_section(current.key.section);
    let releases = bridge.take_release_commands();
    assert_eq!(releases.len(), 2);
    assert!(matches!(
        releases.as_slice(),
        [
            FrameCommand::ReleaseCachedTexturedMesh { .. },
            FrameCommand::ReleaseCachedTexturedMesh { .. }
        ]
    ));
    assert_eq!(bridge.stats(), VoxelRenderStats::default());
}

fn cached_ids(commands: &[FrameCommand]) -> Vec<sindri_render::CachedMeshId> {
    commands
        .iter()
        .map(|command| match command {
            FrameCommand::CachedTexturedMesh { cache, .. } => *cache,
            _ => panic!("voxel draw produced a non-cache command"),
        })
        .collect()
}

fn has_replacement(command: &FrameCommand) -> bool {
    matches!(
        command,
        FrameCommand::CachedTexturedMesh {
            replacement: Some(_),
            ..
        }
    )
}

fn no_replacement(command: &FrameCommand) -> bool {
    matches!(
        command,
        FrameCommand::CachedTexturedMesh {
            replacement: None,
            ..
        }
    )
}
