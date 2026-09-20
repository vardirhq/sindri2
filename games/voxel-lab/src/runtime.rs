use std::collections::BTreeSet;

use sindri_render::FrameCommand;
use sindri_scene::{VoxelRenderBridge, VoxelRenderError, VoxelTextureSource, compile_block_mesh};
use sindri_voxel::{
    MeshingProfile, ResidencyConfig, SectionCoord, SectionMeshKey, VoxelCoord, VoxelId,
    VoxelSource, VoxelWorld, mesh_block_section,
};

const GRASS: VoxelId = VoxelId::new(1);
const DIRT: VoxelId = VoxelId::new(2);
const STONE: VoxelId = VoxelId::new(3);

/// Deterministic filled terrain used to exercise section residency and meshing.
#[derive(Clone, Copy, Debug, Default)]
pub struct LabTerrain;

impl VoxelSource for LabTerrain {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        let height = terrain_height(coord.x, coord.z);
        if coord.y > height {
            VoxelId::AIR
        } else if coord.y == height {
            GRASS
        } else if coord.y >= height - 3 {
            DIRT
        } else {
            STONE
        }
    }
}

/// Observable work performed while producing one Voxel Lab frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VoxelLabStats {
    pub resident_sections: usize,
    pub entering_sections: usize,
    pub leaving_sections: usize,
    pub mesh_jobs: usize,
    pub remeshes: usize,
    pub compiled_sections: usize,
    pub cache_batches: usize,
    pub triangles: usize,
    pub uploads: usize,
    pub releases: usize,
}

/// Renderer commands and counters produced from the authoritative voxel world.
#[derive(Debug)]
pub struct VoxelLabFrame {
    pub commands: Vec<FrameCommand>,
    pub stats: VoxelLabStats,
}

/// Small synchronous driver for the same queue/cache flow an asynchronous host
/// will drain later. Camera motion only queues newly entering sections.
pub struct VoxelLabRuntime {
    world: VoxelWorld<LabTerrain>,
    render: VoxelRenderBridge,
    resident: BTreeSet<SectionCoord>,
}

impl Default for VoxelLabRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelLabRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            world: VoxelWorld::new(LabTerrain, ResidencyConfig::new(1, 0, 0, 0)),
            render: VoxelRenderBridge::default(),
            resident: BTreeSet::new(),
        }
    }

    /// Applies an edit to authoritative voxel data. The next frame drains only
    /// the affected section work, including a resident boundary neighbour.
    pub fn set_voxel(&mut self, coord: VoxelCoord, voxel: VoxelId) -> bool {
        self.world.set_voxel(coord, voxel)
    }

    /// Removes the generated surface voxel at a world column.
    pub fn dig_surface(&mut self, x: i32, z: i32) -> bool {
        self.set_voxel(VoxelCoord::new(x, terrain_height(x, z), z), VoxelId::AIR)
    }

    /// Moves residency to the camera section and drains the current CPU work.
    pub fn frame(
        &mut self,
        focus: SectionCoord,
        textures: &impl VoxelTextureSource,
    ) -> Result<VoxelLabFrame, VoxelRenderError> {
        let delta = self.world.move_focus(focus);
        for section in &delta.left {
            self.resident.remove(section);
            self.render.remove_section(*section);
        }
        self.resident.extend(delta.entered.iter().copied());

        let jobs = self.world.take_mesh_work();
        for job in &jobs {
            if self.render.schedule(*job) {
                let mesh = mesh_block_section(&self.world, job.key.section);
                let compiled = compile_block_mesh(&mesh, textures)?;
                self.render.finish(*job, compiled)?;
            }
        }

        let bridge_stats = self.render.stats();
        let uploads = bridge_stats.pending_uploads;
        let mut commands = self.render.take_release_commands();
        let releases = commands.len();
        for section in &self.resident {
            commands.extend(
                self.render
                    .draw_commands(SectionMeshKey::new(*section, MeshingProfile::Block)),
            );
        }

        Ok(VoxelLabFrame {
            commands,
            stats: VoxelLabStats {
                resident_sections: self.world.resident_len(),
                entering_sections: delta.entered.len(),
                leaving_sections: delta.left.len(),
                mesh_jobs: jobs.len(),
                remeshes: jobs.len().saturating_sub(delta.entered.len()),
                compiled_sections: bridge_stats.compiled_sections,
                cache_batches: bridge_stats.batches,
                triangles: bridge_stats.triangles,
                uploads,
                releases,
            },
        })
    }
}

fn terrain_height(x: i32, z: i32) -> i32 {
    let broad = (x.div_euclid(7) + z.div_euclid(9)).rem_euclid(5);
    let detail = (x.wrapping_mul(31) ^ z.wrapping_mul(17)).rem_euclid(3);
    3 + broad + detail
}

#[cfg(test)]
mod tests {
    use sindri_render::{TextureId, UvRect};
    use sindri_scene::VoxelTexture;

    use super::*;

    fn texture(_voxel: VoxelId, _face: sindri_voxel::VoxelFace) -> VoxelTexture {
        VoxelTexture::new(TextureId::new(1), UvRect::FULL)
    }

    #[test]
    fn settled_camera_produces_zero_mesh_work_or_uploads() {
        let mut lab = VoxelLabRuntime::new();
        let focus = SectionCoord::new(0, 0, 0);
        let first = lab.frame(focus, &texture).unwrap();
        assert_eq!(first.stats.entering_sections, 9);
        assert_eq!(first.stats.mesh_jobs, 9);
        assert!(first.stats.uploads > 0);

        let settled = lab.frame(focus, &texture).unwrap();
        assert_eq!(settled.stats.entering_sections, 0);
        assert_eq!(settled.stats.mesh_jobs, 0);
        assert_eq!(settled.stats.remeshes, 0);
        assert_eq!(settled.stats.uploads, 0);
    }

    #[test]
    fn boundary_edit_remeshes_both_resident_sections() {
        let mut lab = VoxelLabRuntime::new();
        let focus = SectionCoord::new(0, 0, 0);
        lab.frame(focus, &texture).unwrap();
        assert!(lab.set_voxel(VoxelCoord::new(15, 4, 4), VoxelId::AIR));

        let edited = lab.frame(focus, &texture).unwrap();
        assert_eq!(edited.stats.entering_sections, 0);
        assert_eq!(edited.stats.mesh_jobs, 2);
        assert_eq!(edited.stats.remeshes, 2);
    }

    #[test]
    fn moving_one_section_only_meshes_the_entering_edge() {
        let mut lab = VoxelLabRuntime::new();
        lab.frame(SectionCoord::new(0, 0, 0), &texture).unwrap();

        let moved = lab.frame(SectionCoord::new(1, 0, 0), &texture).unwrap();
        assert_eq!(moved.stats.entering_sections, 3);
        assert_eq!(moved.stats.leaving_sections, 3);
        assert_eq!(moved.stats.mesh_jobs, 3);
        assert_eq!(moved.stats.remeshes, 0);
        assert!(moved.stats.releases > 0);
    }
}
