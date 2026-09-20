use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Vec3};
use sindri_render::{CachedMeshId, CachedTexturedMeshUpload, FrameCommand, TextureId};
use sindri_voxel::{
    RenderClass, SectionBounds, SectionCoord, SectionMeshCache, SectionMeshJob, SectionMeshKey,
};

use super::{CompiledVoxelSection, VoxelRenderError};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BatchKey {
    mesh: SectionMeshKey,
    texture: TextureId,
}

#[derive(Clone, Debug)]
struct ResidentBatch {
    key: BatchKey,
    cache: CachedMeshId,
    texture: TextureId,
    triangles: usize,
}

#[derive(Clone, Debug)]
struct ResidentSection {
    bounds: SectionBounds,
    batches: Vec<ResidentBatch>,
    triangles: usize,
}

/// Revision-aware bridge from compiled voxel sections to persistent GPU draws.
#[derive(Clone, Debug, Default)]
pub struct VoxelRenderBridge {
    sections: SectionMeshCache<ResidentSection>,
    identities: BTreeMap<BatchKey, CachedMeshId>,
    uploads: BTreeMap<BatchKey, CachedTexturedMeshUpload>,
    releases: Vec<CachedMeshId>,
    next_identity: u64,
    triangles: usize,
}

impl VoxelRenderBridge {
    /// Records newer mesh work while leaving the current section drawable.
    pub fn schedule(&mut self, job: SectionMeshJob) -> bool {
        self.sections.schedule(job)
    }

    /// Installs a completed CPU mesh if it matches the latest scheduled job.
    ///
    /// Returns `false` for a superseded result. Its geometry never reaches the
    /// renderer cache.
    pub fn finish(
        &mut self,
        job: SectionMeshJob,
        compiled: CompiledVoxelSection,
    ) -> Result<bool, VoxelRenderError> {
        if compiled.section != job.key.section {
            return Err(VoxelRenderError::SectionMismatch {
                expected: job.key.section,
                actual: compiled.section,
            });
        }
        if self.sections.pending_revision(job.key) != Some(job.revision) {
            return Ok(false);
        }
        if let Some(batch) = compiled
            .batches
            .iter()
            .find(|batch| batch.render_class != RenderClass::Opaque)
        {
            return Err(VoxelRenderError::UnsupportedRenderClass(batch.render_class));
        }

        let (resident, uploads) = self.prepare(job.key, compiled);
        let new_keys: BTreeSet<_> = resident.batches.iter().map(|batch| batch.key).collect();
        let new_triangles = resident.triangles;
        let previous = match self.sections.finish(job, resident) {
            Ok(previous) => previous,
            Err(_) => return Ok(false),
        };
        if let Some(previous) = previous {
            self.triangles = self.triangles.saturating_sub(previous.triangles);
            self.release_missing(previous, &new_keys);
        }
        self.triangles = self.triangles.saturating_add(new_triangles);
        self.uploads.extend(uploads);
        Ok(true)
    }

    /// Draw commands for one resident representation.
    ///
    /// A newly finished revision moves its replacement geometry into the first
    /// command. Later frames carry no replacement and reuse the GPU buffers.
    pub fn draw_commands(&mut self, key: SectionMeshKey) -> Vec<FrameCommand> {
        let Some(revision) = self.sections.revision(key) else {
            return Vec::new();
        };
        let Some(section) = self.sections.get(key) else {
            return Vec::new();
        };
        let model = section_model(key.section);
        section
            .batches
            .iter()
            .map(|batch| FrameCommand::CachedTexturedMesh {
                model,
                texture: batch.texture,
                cache: batch.cache,
                revision: revision.value(),
                replacement: self.uploads.remove(&batch.key),
            })
            .collect()
    }

    /// Releases every profile and GPU batch belonging to a departed section.
    pub fn remove_section(&mut self, section: SectionCoord) {
        for resident in self.sections.remove_section(section) {
            self.triangles = self.triangles.saturating_sub(resident.triangles);
            for batch in resident.batches {
                self.uploads.remove(&batch.key);
                if self.identities.remove(&batch.key).is_some() {
                    self.releases.push(batch.cache);
                }
            }
        }
    }

    /// Moves queued release operations into frame commands.
    pub fn take_release_commands(&mut self) -> Vec<FrameCommand> {
        self.releases
            .drain(..)
            .map(|cache| FrameCommand::ReleaseCachedTexturedMesh { cache })
            .collect()
    }

    #[must_use]
    pub fn bounds(&self, key: SectionMeshKey) -> Option<SectionBounds> {
        self.sections.get(key).map(|section| section.bounds)
    }

    #[must_use]
    pub fn stats(&self) -> VoxelRenderStats {
        VoxelRenderStats {
            compiled_sections: self.sections.len(),
            batches: self.identities.len(),
            triangles: self.triangles,
            pending_uploads: self.uploads.len(),
            pending_releases: self.releases.len(),
        }
    }

    fn prepare(
        &mut self,
        mesh: SectionMeshKey,
        compiled: CompiledVoxelSection,
    ) -> (
        ResidentSection,
        BTreeMap<BatchKey, CachedTexturedMeshUpload>,
    ) {
        let mut batches = Vec::with_capacity(compiled.batches.len());
        let mut uploads = BTreeMap::new();
        for batch in compiled.batches {
            let key = BatchKey {
                mesh,
                texture: batch.texture,
            };
            let cache = self.identity(key);
            let triangles = batch.triangle_count();
            uploads.insert(key, batch.upload);
            batches.push(ResidentBatch {
                key,
                cache,
                texture: batch.texture,
                triangles,
            });
        }
        let triangles = batches.iter().map(|batch| batch.triangles).sum();
        (
            ResidentSection {
                bounds: compiled.bounds,
                batches,
                triangles,
            },
            uploads,
        )
    }

    fn identity(&mut self, key: BatchKey) -> CachedMeshId {
        if let Some(identity) = self.identities.get(&key) {
            return *identity;
        }
        loop {
            self.next_identity = self.next_identity.wrapping_add(1);
            let candidate = CachedMeshId::new(self.next_identity);
            if !self
                .identities
                .values()
                .any(|identity| *identity == candidate)
            {
                self.identities.insert(key, candidate);
                return candidate;
            }
        }
    }

    fn release_missing(&mut self, previous: ResidentSection, current: &BTreeSet<BatchKey>) {
        for batch in previous.batches {
            if !current.contains(&batch.key) {
                self.uploads.remove(&batch.key);
                if self.identities.remove(&batch.key).is_some() {
                    self.releases.push(batch.cache);
                }
            }
        }
    }
}

fn section_model(section: SectionCoord) -> Mat4 {
    let min = section.min_voxel();
    #[allow(clippy::cast_precision_loss)]
    Mat4::from_translation(Vec3::new(min.x as f32, min.y as f32, min.z as f32))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VoxelRenderStats {
    pub compiled_sections: usize,
    pub batches: usize,
    pub triangles: usize,
    pub pending_uploads: usize,
    pub pending_releases: usize,
}
