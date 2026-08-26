//! The store itself: what it holds, and the transitions it allows.

use std::{
    collections::BTreeMap,
    marker::PhantomData,
    sync::{Arc, Weak},
};

use thiserror::Error;

use super::handle::{AssetHandle, AssetLease};
use super::id::AssetId;
use super::status::{AssetLoadError, AssetLoadErrorKind, AssetStatus};

#[derive(Debug)]
enum StoredAsset<T> {
    Queued,
    Loading,
    Ready(T),
    Failed(AssetLoadError),
}

impl<T> StoredAsset<T> {
    const fn status(&self) -> AssetStatus {
        match self {
            Self::Queued => AssetStatus::Queued,
            Self::Loading => AssetStatus::Loading,
            Self::Ready(_) => AssetStatus::Ready,
            Self::Failed(_) => AssetStatus::Failed,
        }
    }
}

#[derive(Debug)]
struct AssetEntry<T> {
    generation: u64,
    lease: Weak<AssetLease>,
    state: StoredAsset<T>,
}

/// Renderer- and executor-independent storage for one asset type.
///
/// The store owns loaded values while handles own liveness. Requests for an
/// already-live ID coalesce onto the same generation. Call [`Self::collect_unused`]
/// at an explicit maintenance point to release entries without strong handles.
#[derive(Debug)]
pub struct AssetStore<T> {
    entries: BTreeMap<AssetId, AssetEntry<T>>,
    next_generation: u64,
}

impl<T> Default for AssetStore<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_generation: 0,
        }
    }
}

impl<T> AssetStore<T> {
    pub fn request(&mut self, id: AssetId) -> AssetHandle<T> {
        if let Some(entry) = self.entries.get(&id)
            && let Some(lease) = entry.lease.upgrade()
        {
            return AssetHandle {
                id,
                generation: entry.generation,
                lease,
                marker: PhantomData,
            };
        }

        let generation = self.allocate_generation();
        let lease = Arc::new(AssetLease);
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.generation = generation;
            entry.lease = Arc::downgrade(&lease);
        } else {
            self.entries.insert(
                id.clone(),
                AssetEntry {
                    generation,
                    lease: Arc::downgrade(&lease),
                    state: StoredAsset::Queued,
                },
            );
        }

        AssetHandle {
            id,
            generation,
            lease,
            marker: PhantomData,
        }
    }

    pub fn status(&self, handle: &AssetHandle<T>) -> Result<AssetStatus, AssetStoreError> {
        Ok(self.entry(handle)?.state.status())
    }

    pub fn status_by_id(&self, id: &AssetId) -> Option<AssetStatus> {
        self.entries.get(id).map(|entry| entry.state.status())
    }

    pub fn begin_loading(&mut self, handle: &AssetHandle<T>) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Queued, StoredAsset::Loading)
    }

    pub fn complete(&mut self, handle: &AssetHandle<T>, value: T) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Loading, StoredAsset::Ready(value))
    }

    pub fn fail(
        &mut self,
        handle: &AssetHandle<T>,
        kind: AssetLoadErrorKind,
        message: impl Into<String>,
    ) -> Result<(), AssetStoreError> {
        let error = AssetLoadError::new(handle.id.clone(), kind, message);
        self.transition(handle, AssetStatus::Loading, StoredAsset::Failed(error))
    }

    pub fn retry(&mut self, handle: &AssetHandle<T>) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Failed, StoredAsset::Queued)
    }

    /// Marks a ready asset as needing loading again.
    ///
    /// What hot reload does: the bytes on disk changed, so the value held is
    /// out of date, and the fact that it loaded successfully is exactly why it
    /// must load again. Separate from [`Self::retry`] because the two start
    /// from opposite states and mean opposite things — one is "that did not
    /// work", the other is "that worked and is now stale".
    pub fn reload(&mut self, handle: &AssetHandle<T>) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Ready, StoredAsset::Queued)
    }

    pub fn get(&self, handle: &AssetHandle<T>) -> Result<Option<&T>, AssetStoreError> {
        Ok(match &self.entry(handle)?.state {
            StoredAsset::Ready(value) => Some(value),
            _ => None,
        })
    }

    pub fn error(
        &self,
        handle: &AssetHandle<T>,
    ) -> Result<Option<&AssetLoadError>, AssetStoreError> {
        Ok(match &self.entry(handle)?.state {
            StoredAsset::Failed(error) => Some(error),
            _ => None,
        })
    }

    pub fn collect_unused(&mut self) -> Vec<AssetId> {
        let unused = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.lease.strong_count() == 0)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in &unused {
            self.entries.remove(id);
        }
        unused
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("asset handle generation exhausted u64::MAX");
        generation
    }

    fn transition(
        &mut self,
        handle: &AssetHandle<T>,
        expected: AssetStatus,
        next: StoredAsset<T>,
    ) -> Result<(), AssetStoreError> {
        let entry = self.entry_mut(handle)?;
        let actual = entry.state.status();
        if actual != expected {
            return Err(AssetStoreError::InvalidTransition {
                id: handle.id.clone(),
                from: actual,
                to: next.status(),
            });
        }
        entry.state = next;
        Ok(())
    }

    fn entry(&self, handle: &AssetHandle<T>) -> Result<&AssetEntry<T>, AssetStoreError> {
        self.entries
            .get(&handle.id)
            .filter(|entry| entry.matches(handle))
            .ok_or_else(|| AssetStoreError::InvalidHandle(handle.id.clone()))
    }

    fn entry_mut(
        &mut self,
        handle: &AssetHandle<T>,
    ) -> Result<&mut AssetEntry<T>, AssetStoreError> {
        self.entries
            .get_mut(&handle.id)
            .filter(|entry| entry.matches(handle))
            .ok_or_else(|| AssetStoreError::InvalidHandle(handle.id.clone()))
    }
}

impl<T> AssetEntry<T> {
    fn matches(&self, handle: &AssetHandle<T>) -> bool {
        self.generation == handle.generation
            && self
                .lease
                .upgrade()
                .is_some_and(|lease| Arc::ptr_eq(&lease, &handle.lease))
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssetStoreError {
    #[error("invalid or stale handle for asset '{0}'")]
    InvalidHandle(AssetId),
    #[error("asset '{id}' cannot transition from {from} to {to}")]
    InvalidTransition {
        id: AssetId,
        from: AssetStatus,
        to: AssetStatus,
    },
}
