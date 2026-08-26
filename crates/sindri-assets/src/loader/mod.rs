//! Loading one kind of asset, end to end.
//!
//! Every piece of this has existed since the asset foundation landed: a store
//! that owns values and coalesces requests for the same ID, a bounded queue
//! carrying a handle generation so a late completion cannot overwrite a
//! replacement, and decoders that turn bytes into values. Nothing in the
//! workspace used any of it. The demo's badge is `include_bytes!` and the editor
//! bound two textures a demo crate handed it, so the only part of the pipeline
//! with a caller was the decoder.
//!
//! The reason is visible in the shape: loading one texture correctly is six
//! steps in a particular order — request a handle, enqueue the request, move the
//! entry to loading, drain, decode, apply against the handle that is still
//! current — and getting one of them wrong fails quietly rather than loudly. So
//! this is those six steps, written once, with the ordering inside it rather
//! than in a comment every caller has to obey.
//!
//! GPU upload deliberately stays outside. A loader that owned a device could not
//! be tested without one, and the host is the only thing that has one.

use std::collections::{BTreeMap, BTreeSet};

use sindri_core::{AssetHandle, AssetId, AssetLoadError, AssetStatus, AssetStore, AssetStoreError};
use thiserror::Error;

use crate::{
    AssetCompletionApplyError, AssetDecoder, AssetLoadQueue, AssetLoadQueueConfig,
    AssetLoadQueueCreateError, AssetLoadQueueError, AssetLoadRequest, AssetManifest, AssetSource,
    decode_completion,
};

#[cfg(test)]
mod tests;

/// What became of an asset between one poll and the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetLoadOutcome {
    /// The asset is in the loader and can be read with [`AssetLoader::get`].
    Ready(AssetId),
    /// It will not arrive, and this says why.
    ///
    /// Carried out rather than left in the store to be discovered, because the
    /// thing a host does about a failed asset — say so, draw a placeholder — it
    /// does once, when the failure happens.
    Failed(AssetLoadError),
}

impl AssetLoadOutcome {
    pub fn id(&self) -> &AssetId {
        match self {
            Self::Ready(id) => id,
            Self::Failed(error) => error.id(),
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssetLoaderError {
    #[error(transparent)]
    Queue(#[from] AssetLoadQueueError),
    #[error(transparent)]
    Store(#[from] AssetStoreError),
}

/// A store, a queue, and a decoder, driven in the one order that works.
///
/// The loader holds a strong handle for every asset it has been asked for, which
/// is what keeps the store's entries alive: the store releases anything no
/// handle refers to, and without an owner every asset would be collectable the
/// moment it arrived. [`Self::retain`] is how a host says which of them it still
/// wants, and it reports what that released so the host can drop whatever it
/// built on top — a GPU texture, most obviously.
pub struct AssetLoader<D: AssetDecoder> {
    store: AssetStore<D::Asset>,
    queue: AssetLoadQueue,
    decoder: D,
    handles: BTreeMap<AssetId, AssetHandle<D::Asset>>,
    /// What each asset is supposed to be, if the project said.
    ///
    /// Checked against the bytes that arrive, before anything decodes them: a
    /// truncated response or a stale cache entry usually still decodes, and the
    /// result is a picture from last week rather than an error.
    manifest: Option<AssetManifest>,
}

impl<D: AssetDecoder> AssetLoader<D> {
    /// Builds a loader reading through `source`.
    ///
    /// Native builds start the queue's I/O workers here, so a blocking
    /// filesystem read never happens on the thread drawing frames.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<S>(
        source: S,
        config: AssetLoadQueueConfig,
        decoder: D,
    ) -> Result<Self, AssetLoadQueueCreateError>
    where
        S: AssetSource + Send + Sync + 'static,
    {
        Ok(Self {
            store: AssetStore::default(),
            queue: AssetLoadQueue::new(source, config)?,
            decoder,
            handles: BTreeMap::new(),
            manifest: None,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new<S>(
        source: S,
        config: AssetLoadQueueConfig,
        decoder: D,
    ) -> Result<Self, AssetLoadQueueCreateError>
    where
        S: AssetSource + 'static,
    {
        Ok(Self {
            store: AssetStore::default(),
            queue: AssetLoadQueue::new(source, config)?,
            decoder,
            handles: BTreeMap::new(),
            manifest: None,
        })
    }

    /// Holds every arriving asset to what the project said it would be.
    ///
    /// Optional because a project need not have a manifest, and an asset the
    /// manifest does not mention still loads: this is a promise about what was
    /// listed, not a claim that nothing else exists.
    #[must_use]
    pub fn with_manifest(mut self, manifest: AssetManifest) -> Self {
        self.manifest = Some(manifest);
        self
    }

    /// Asks for an asset, if it has not been asked for already.
    ///
    /// Idempotent, because the caller for this is a scene naming the same
    /// texture from twenty entities: asking twenty times must cost one load.
    /// That includes an asset that already failed — a failure is an answer, and
    /// re-requesting it every frame would retry forever. [`Self::retry`] is how
    /// a caller says it wants another go.
    pub fn request(&mut self, id: AssetId) -> Result<(), AssetLoaderError> {
        if self.handles.contains_key(&id) {
            return Ok(());
        }
        self.start(id)
    }

    /// Asks again for an asset that failed.
    ///
    /// Does nothing to one that is loading or ready, so a caller retrying
    /// everything cannot restart a load that is already working.
    pub fn retry(&mut self, id: &AssetId) -> Result<(), AssetLoaderError> {
        if self.status(id) != Some(AssetStatus::Failed) {
            return Ok(());
        }
        self.handles.remove(id);
        self.start(id.clone())
    }

    /// Loads an asset again, whatever it holds now.
    ///
    /// What hot reload calls when the file behind an asset has changed: the
    /// value held is out of date, and having loaded successfully is exactly why
    /// it must load again. An asset already in flight is left alone — the read
    /// under way may be picking up the new bytes anyway, and if it is not, the
    /// next save reports the change again.
    pub fn reload(&mut self, id: &AssetId) -> Result<(), AssetLoaderError> {
        if self.status(id) == Some(AssetStatus::Loading) {
            return Ok(());
        }
        self.handles.remove(id);
        self.start(id.clone())
    }

    /// The six steps, in the order that works.
    fn start(&mut self, id: AssetId) -> Result<(), AssetLoaderError> {
        let handle = self.store.request(id.clone());
        // Taking out a new handle does not reset the entry behind it: an asset
        // that failed is still failed and a loaded one is still loaded, and
        // neither can go straight to loading. This is the trap that makes the
        // sequence worth having in one place — nothing about `request` suggests
        // the state survived.
        match self.store.status(&handle)? {
            AssetStatus::Failed => self.store.retry(&handle)?,
            AssetStatus::Ready => self.store.reload(&handle)?,
            AssetStatus::Queued | AssetStatus::Loading => {}
        }
        // Enqueued before the entry moves to loading, so a queue that is full
        // leaves nothing claiming to be in flight. The handle is dropped on the
        // way out and the store reclaims the entry, which is what makes a
        // rejected request safe to make again.
        self.queue.enqueue(AssetLoadRequest::new(&handle))?;
        self.store.begin_loading(&handle)?;
        self.handles.insert(id, handle);
        Ok(())
    }

    /// Takes delivery of whatever finished, and says what changed.
    ///
    /// Called once a frame. Decoding happens here rather than on the I/O worker
    /// because a decoder is chosen by the caller and may not be `Send`; the
    /// bytes crossing the boundary are the part that had to be asynchronous.
    pub fn poll(&mut self) -> Vec<AssetLoadOutcome> {
        let mut outcomes = Vec::new();
        for completion in self.queue.drain() {
            let id = completion.request().id().clone();
            let Some(handle) = self.handles.get(&id) else {
                // Released while it was in flight. Nothing wants it now.
                continue;
            };
            // A newer request for the same ID replaced this one. The generation
            // token exists for exactly this, and the right answer is silence:
            // the replacement is still coming.
            if !completion.request().matches(handle) {
                continue;
            }
            // The manifest decides whether these are the asset's bytes before
            // anything decodes them. A truncated response or a stale cache entry
            // usually still decodes, and the result is a picture from last week.
            if let Some(mismatch) = self.manifest.as_ref().and_then(|manifest| {
                completion
                    .result()
                    .ok()
                    .and_then(|bytes| manifest.verify(&id, bytes.as_slice()).err())
            }) {
                let _ = self.store.fail(handle, mismatch.kind(), mismatch.message());
                outcomes.push(AssetLoadOutcome::Failed(mismatch));
                continue;
            }
            let decoded = decode_completion(completion, &self.decoder);
            // Read before applying, because applying consumes it, and whether
            // this is news of an arrival or of a failure is the whole answer.
            let failure = decoded.result().err().cloned();
            match decoded.apply(&mut self.store, handle) {
                Ok(()) => outcomes.push(match failure {
                    Some(error) => AssetLoadOutcome::Failed(error),
                    None => AssetLoadOutcome::Ready(id),
                }),
                // Ruled out above, and still handled: silence is the right
                // answer to a completion whose replacement is on its way.
                Err(AssetCompletionApplyError::Stale { .. }) => {}
                Err(AssetCompletionApplyError::Store(error)) => {
                    outcomes.push(AssetLoadOutcome::Failed(AssetLoadError::new(
                        id,
                        sindri_core::AssetLoadErrorKind::Other,
                        error.to_string(),
                    )));
                }
            }
        }
        outcomes
    }

    /// The asset, once it is ready.
    pub fn get(&self, id: &AssetId) -> Option<&D::Asset> {
        let handle = self.handles.get(id)?;
        self.store.get(handle).ok().flatten()
    }

    pub fn status(&self, id: &AssetId) -> Option<AssetStatus> {
        self.store.status_by_id(id)
    }

    pub fn error(&self, id: &AssetId) -> Option<&AssetLoadError> {
        let handle = self.handles.get(id)?;
        self.store.error(handle).ok().flatten()
    }

    /// Keeps only the assets named, and reports which were released.
    ///
    /// The returned IDs are the ones nothing refers to any more, which is what
    /// a host needs to know to drop whatever it built from them. An asset still
    /// in flight is released too — the completion arrives and is discarded,
    /// rather than the loader holding a scene's worth of textures because they
    /// were requested before the scene was replaced.
    pub fn retain(&mut self, keep: &BTreeSet<AssetId>) -> Vec<AssetId> {
        self.handles.retain(|id, _| keep.contains(id));
        self.store.collect_unused()
    }

    /// How many loads have not come back yet.
    pub fn outstanding(&self) -> usize {
        self.queue.outstanding()
    }

    /// Every asset the loader is holding, whatever state it is in.
    pub fn requested(&self) -> impl ExactSizeIterator<Item = &AssetId> {
        self.handles.keys()
    }
}
