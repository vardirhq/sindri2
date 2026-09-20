use std::collections::BTreeMap;

use crate::SectionCoord;

/// Which meshing strategy produced a section's compiled representation.
///
/// The profile is part of the cache identity: block, smooth, and hybrid
/// presentations may coexist over the same voxel data without overwriting one
/// another. Custom profiles are game-defined identities, not renderer types.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MeshingProfile {
    Block,
    Smooth,
    Hybrid,
    Custom(u32),
}

/// Stable identity of one compiled section representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SectionMeshKey {
    pub section: SectionCoord,
    pub profile: MeshingProfile,
}

impl SectionMeshKey {
    #[must_use]
    pub const fn new(section: SectionCoord, profile: MeshingProfile) -> Self {
        Self { section, profile }
    }
}

/// Monotonic identity of the voxel and halo state a mesh was requested for.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SectionMeshRevision(u64);

impl SectionMeshRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// One revisioned section-meshing request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SectionMeshJob {
    pub key: SectionMeshKey,
    pub revision: SectionMeshRevision,
}

impl SectionMeshJob {
    #[must_use]
    pub const fn new(key: SectionMeshKey, revision: SectionMeshRevision) -> Self {
        Self { key, revision }
    }
}

#[derive(Clone, Debug)]
struct CachedMesh<T> {
    revision: SectionMeshRevision,
    value: T,
}

/// Persistent compiled section meshes with asynchronous replacement semantics.
///
/// `T` may be CPU geometry, a renderer-owned GPU handle, or a bridge type that
/// contains both. Scheduling a replacement never removes the current value, so
/// callers can keep drawing old geometry until the matching result finishes.
/// Results for superseded revisions are rejected rather than replacing newer
/// terrain.
#[derive(Clone, Debug)]
pub struct SectionMeshCache<T> {
    entries: BTreeMap<SectionMeshKey, CachedMesh<T>>,
    pending: BTreeMap<SectionMeshKey, SectionMeshRevision>,
}

impl<T> Default for SectionMeshCache<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }
}

impl<T> SectionMeshCache<T> {
    /// Records a job if neither the cache nor the pending work is as new.
    pub fn schedule(&mut self, job: SectionMeshJob) -> bool {
        if self
            .entries
            .get(&job.key)
            .is_some_and(|entry| entry.revision >= job.revision)
            || self
                .pending
                .get(&job.key)
                .is_some_and(|revision| *revision >= job.revision)
        {
            return false;
        }
        self.pending.insert(job.key, job.revision);
        true
    }

    /// Installs a finished result only when it matches the latest request.
    ///
    /// The replaced value is returned on success so a renderer can defer its
    /// destruction if required. A stale result is returned in `Err` unchanged.
    pub fn finish(&mut self, job: SectionMeshJob, value: T) -> Result<Option<T>, T> {
        if self.pending.get(&job.key) != Some(&job.revision) {
            return Err(value);
        }
        self.pending.remove(&job.key);
        Ok(self
            .entries
            .insert(
                job.key,
                CachedMesh {
                    revision: job.revision,
                    value,
                },
            )
            .map(|entry| entry.value))
    }

    #[must_use]
    pub fn get(&self, key: SectionMeshKey) -> Option<&T> {
        self.entries.get(&key).map(|entry| &entry.value)
    }

    #[must_use]
    pub fn revision(&self, key: SectionMeshKey) -> Option<SectionMeshRevision> {
        self.entries.get(&key).map(|entry| entry.revision)
    }

    #[must_use]
    pub fn pending_revision(&self, key: SectionMeshKey) -> Option<SectionMeshRevision> {
        self.pending.get(&key).copied()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Releases every presentation of a section and cancels its pending work.
    pub fn remove_section(&mut self, section: SectionCoord) -> Vec<T> {
        self.pending.retain(|key, _| key.section != section);
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|key| key.section == section)
            .copied()
            .collect();
        keys.into_iter()
            .filter_map(|key| self.entries.remove(&key).map(|entry| entry.value))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(section: SectionCoord, profile: MeshingProfile, revision: u64) -> SectionMeshJob {
        SectionMeshJob::new(
            SectionMeshKey::new(section, profile),
            SectionMeshRevision::new(revision),
        )
    }

    #[test]
    fn old_geometry_stays_available_until_replacement_finishes() {
        let section = SectionCoord::new(2, -1, 4);
        let first = job(section, MeshingProfile::Block, 1);
        let second = job(section, MeshingProfile::Block, 2);
        let mut cache = SectionMeshCache::default();

        assert!(cache.schedule(first));
        assert_eq!(cache.finish(first, "old"), Ok(None));
        assert!(cache.schedule(second));
        assert_eq!(cache.get(first.key), Some(&"old"));
        assert_eq!(cache.finish(second, "new"), Ok(Some("old")));
        assert_eq!(cache.get(first.key), Some(&"new"));
    }

    #[test]
    fn superseded_results_cannot_replace_newer_geometry() {
        let section = SectionCoord::new(0, 0, 0);
        let old = job(section, MeshingProfile::Block, 8);
        let new = job(section, MeshingProfile::Block, 9);
        let mut cache = SectionMeshCache::default();

        assert!(cache.schedule(old));
        assert!(cache.schedule(new));
        assert_eq!(cache.finish(old, "stale"), Err("stale"));
        assert_eq!(cache.finish(new, "current"), Ok(None));
        assert_eq!(cache.get(new.key), Some(&"current"));
    }

    #[test]
    fn profiles_are_independent_and_leave_together() {
        let section = SectionCoord::new(-3, 2, 7);
        let block = job(section, MeshingProfile::Block, 3);
        let smooth = job(section, MeshingProfile::Smooth, 4);
        let custom = job(section, MeshingProfile::Custom(12), 5);
        let mut cache = SectionMeshCache::default();
        assert!(cache.schedule(block));
        cache.finish(block, "block").unwrap();
        assert!(cache.schedule(smooth));
        cache.finish(smooth, "smooth").unwrap();
        assert!(cache.schedule(custom));

        assert_eq!(cache.len(), 2);
        let mut removed = cache.remove_section(section);
        removed.sort_unstable();
        assert_eq!(removed, vec!["block", "smooth"]);
        assert!(cache.is_empty());
        assert_eq!(cache.pending_revision(custom.key), None);
        assert_eq!(cache.finish(custom, "late"), Err("late"));
    }

    #[test]
    fn unchanged_jobs_are_not_scheduled_again() {
        let current = job(SectionCoord::new(1, 0, 1), MeshingProfile::Block, 5);
        let mut cache = SectionMeshCache::default();
        assert!(cache.schedule(current));
        assert!(!cache.schedule(current));
        cache.finish(current, ()).unwrap();
        assert!(!cache.schedule(current));
    }
}
