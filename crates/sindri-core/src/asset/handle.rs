//! Handles into a store, strong and weak.
//!
//! A handle is bound to the store that made it and to a generation, so a
//! stale one is refused rather than silently reading whatever now sits in
//! its place.

use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    sync::{Arc, Weak},
};

use super::id::AssetId;

#[derive(Debug)]
pub struct AssetLease;

/// A strong, type-safe reference to one logical asset request.
///
/// Cloning a handle keeps the asset live. The store may reclaim the loaded
/// value only after every strong handle for that request has been dropped.
pub struct AssetHandle<T> {
    pub(super) id: AssetId,
    pub(super) generation: u64,
    pub(super) lease: Arc<AssetLease>,
    pub(super) marker: PhantomData<fn() -> T>,
}

impl<T> AssetHandle<T> {
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn downgrade(&self) -> WeakAssetHandle<T> {
        WeakAssetHandle {
            id: self.id.clone(),
            generation: self.generation,
            lease: Arc::downgrade(&self.lease),
            marker: PhantomData,
        }
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.lease)
    }
}

impl<T> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            generation: self.generation,
            lease: Arc::clone(&self.lease),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for AssetHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AssetHandle")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl<T> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && Arc::ptr_eq(&self.lease, &other.lease)
    }
}

impl<T> Eq for AssetHandle<T> {}

impl<T> Hash for AssetHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
        Arc::as_ptr(&self.lease).hash(state);
    }
}

/// A non-owning typed asset reference.
///
/// Upgrading succeeds only while at least one strong handle from the same
/// generation remains alive.
pub struct WeakAssetHandle<T> {
    id: AssetId,
    generation: u64,
    lease: Weak<AssetLease>,
    marker: PhantomData<fn() -> T>,
}

impl<T> WeakAssetHandle<T> {
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn upgrade(&self) -> Option<AssetHandle<T>> {
        self.lease.upgrade().map(|lease| AssetHandle {
            id: self.id.clone(),
            generation: self.generation,
            lease,
            marker: PhantomData,
        })
    }
}

impl<T> Clone for WeakAssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            generation: self.generation,
            lease: self.lease.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for WeakAssetHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeakAssetHandle")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .field("alive", &(self.lease.strong_count() > 0))
            .finish()
    }
}
