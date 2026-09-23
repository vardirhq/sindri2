use std::collections::BTreeMap;

use crate::{MeshBuffers, TexturedVertex};

/// Renderer-local identity for one persistent textured mesh.
///
/// The caller owns the mapping from domain identities such as voxel sections
/// and meshing profiles. Keeping that mapping outside the renderer preserves
/// the render crate's independence from scene and world types.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CachedMeshId(u64);

impl CachedMeshId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// How one cached mesh's surface is drawn, beyond its texture.
///
/// Per draw rather than per vertex, so a lake's ripple or a lava field's
/// glow changes by writing four numbers each frame instead of re-uploading
/// its geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeshSurface {
    /// Added to every texture coordinate: which frame of an animation strip
    /// the faces show, as an offset from the frame they were meshed with.
    pub uv_offset: [f32; 2],
    /// How much the surface lights itself, from nothing to fully self-lit and
    /// brighter: lava glows in shadow, and the bloom pass picks it up.
    pub glow: f32,
    /// Texels less opaque than this are not drawn, so leaves have holes rather
    /// than a solid square. Zero draws every texel.
    pub alpha_cutoff: f32,
}

/// Replacement geometry for one persistent textured mesh.
///
/// Indices are 32-bit because a maximally fragmented 16³ voxel section can
/// exceed the 65,535 vertices addressable by a 16-bit index buffer.
#[derive(Clone, Debug, Default)]
pub struct CachedTexturedMeshUpload {
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u32>,
}

impl CachedTexturedMeshUpload {
    #[must_use]
    pub const fn new(vertices: Vec<TexturedVertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }
}

/// Lifetime counters for the persistent textured-mesh cache.
///
/// Frame instrumentation can subtract consecutive snapshots to report uploads,
/// reuse, stale draws, and misses for one frame without making frame lifetime a
/// renderer concern.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedMeshCacheStats {
    pub entries: usize,
    pub installs: u64,
    pub buffer_uploads: u64,
    pub reused_draws: u64,
    pub stale_draws: u64,
    pub misses: u64,
    pub releases: u64,
}

#[derive(Debug)]
struct CachedMesh {
    revision: u64,
    buffers: Option<MeshBuffers>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CacheDecision {
    Install,
    Reuse,
    DrawStale,
    Miss,
}

fn decide(cached: Option<u64>, requested: u64, has_replacement: bool) -> CacheDecision {
    if has_replacement && cached.is_none_or(|revision| requested > revision) {
        return CacheDecision::Install;
    }
    match cached {
        Some(revision) if revision == requested => CacheDecision::Reuse,
        Some(_) => CacheDecision::DrawStale,
        None => CacheDecision::Miss,
    }
}

#[derive(Debug, Default)]
pub(crate) struct TexturedMeshCache {
    entries: BTreeMap<CachedMeshId, CachedMesh>,
    stats: TexturedMeshCacheStats,
}

impl TexturedMeshCache {
    pub(crate) fn resolve(
        &mut self,
        device: &wgpu::Device,
        id: CachedMeshId,
        revision: u64,
        replacement: Option<&CachedTexturedMeshUpload>,
    ) -> Option<&MeshBuffers> {
        let decision = decide(
            self.entries.get(&id).map(|entry| entry.revision),
            revision,
            replacement.is_some(),
        );
        if decision == CacheDecision::Install {
            let upload = replacement.expect("an install has replacement geometry");
            let buffers = if upload.is_empty() {
                None
            } else {
                self.stats.buffer_uploads = self.stats.buffer_uploads.wrapping_add(1);
                Some(MeshBuffers::new_u32(
                    device,
                    "Sindri cached textured mesh",
                    &upload.vertices,
                    &upload.indices,
                ))
            };
            self.entries.insert(id, CachedMesh { revision, buffers });
            self.stats.installs = self.stats.installs.wrapping_add(1);
        }

        match decision {
            CacheDecision::Install => {}
            CacheDecision::Reuse => {
                self.stats.reused_draws = self.stats.reused_draws.wrapping_add(1);
            }
            CacheDecision::DrawStale => {
                self.stats.stale_draws = self.stats.stale_draws.wrapping_add(1);
            }
            CacheDecision::Miss => {
                self.stats.misses = self.stats.misses.wrapping_add(1);
            }
        }
        self.entries
            .get(&id)
            .and_then(|entry| entry.buffers.as_ref())
    }

    pub(crate) fn release(&mut self, id: CachedMeshId) -> bool {
        if self.entries.remove(&id).is_none() {
            return false;
        }
        self.stats.releases = self.stats.releases.wrapping_add(1);
        true
    }

    pub(crate) fn stats(&self) -> TexturedMeshCacheStats {
        TexturedMeshCacheStats {
            entries: self.entries.len(),
            ..self.stats
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_decisions_keep_old_geometry_until_new_data_arrives() {
        assert_eq!(decide(None, 4, false), CacheDecision::Miss);
        assert_eq!(decide(None, 4, true), CacheDecision::Install);
        assert_eq!(decide(Some(4), 4, false), CacheDecision::Reuse);
        assert_eq!(decide(Some(4), 5, false), CacheDecision::DrawStale);
        assert_eq!(decide(Some(4), 5, true), CacheDecision::Install);
        assert_eq!(decide(Some(5), 4, true), CacheDecision::DrawStale);
    }

    #[test]
    fn releasing_an_unknown_mesh_is_not_counted() {
        let mut cache = TexturedMeshCache::default();
        assert!(!cache.release(CachedMeshId::new(7)));
        assert_eq!(cache.stats(), TexturedMeshCacheStats::default());
    }
}
