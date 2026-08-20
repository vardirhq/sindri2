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
    AssetLoadQueueCreateError, AssetLoadQueueError, AssetLoadRequest, AssetSource,
    decode_completion,
};

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
        })
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
            let decoded = decode_completion(completion, &self.decoder);
            let id = decoded.request().id().clone();
            let Some(handle) = self.handles.get(&id) else {
                // Released while it was in flight. Nothing wants it now.
                continue;
            };
            // Read before applying, because applying consumes it, and whether
            // this is news of an arrival or of a failure is the whole answer.
            let failure = decoded.result().err().cloned();
            match decoded.apply(&mut self.store, handle) {
                Ok(()) => outcomes.push(match failure {
                    Some(error) => AssetLoadOutcome::Failed(error),
                    None => AssetLoadOutcome::Ready(id),
                }),
                // A newer request for the same ID replaced this one. The
                // generation token exists for exactly this, and the right answer
                // is silence: the replacement is still coming.
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

#[cfg(test)]
mod tests {
    use sindri_core::AssetLoadErrorKind;

    use super::*;
    use crate::{AssetBytes, AssetDecodeError, MemoryAssetSource};

    /// A decoder that turns bytes into their length, so a test can tell which
    /// asset arrived without carrying an image around.
    #[derive(Clone, Copy, Debug, Default)]
    struct CountingDecoder;

    impl AssetDecoder for CountingDecoder {
        type Asset = usize;

        fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
            if bytes.as_slice() == b"bad" {
                return Err(AssetDecodeError::new(
                    bytes.id().clone(),
                    "counted",
                    AssetLoadErrorKind::InvalidData,
                    "the bytes say so",
                ));
            }
            Ok(bytes.as_slice().len())
        }
    }

    fn id(value: &str) -> AssetId {
        AssetId::new(value).expect("test asset IDs are valid")
    }

    fn loader(files: &[(&str, &[u8])]) -> AssetLoader<CountingDecoder> {
        let mut source = MemoryAssetSource::default();
        for (name, bytes) in files {
            source.insert(AssetBytes::new(id(name), bytes.to_vec()));
        }
        AssetLoader::new(source, AssetLoadQueueConfig::default(), CountingDecoder)
            .expect("the queue starts")
    }

    /// Drains until the loader has nothing outstanding, so a test does not
    /// depend on how many frames the I/O workers took.
    fn settle(loader: &mut AssetLoader<CountingDecoder>) -> Vec<AssetLoadOutcome> {
        let mut outcomes = Vec::new();
        for _ in 0..10_000 {
            outcomes.extend(loader.poll());
            if loader.outstanding() == 0 {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(loader.outstanding(), 0, "the loader never settled");
        outcomes
    }

    #[test]
    fn an_asset_that_loads_becomes_readable() {
        let mut loader = loader(&[("textures/badge.png", b"twelve bytes")]);
        loader.request(id("textures/badge.png")).unwrap();
        assert_eq!(
            loader.status(&id("textures/badge.png")),
            Some(AssetStatus::Loading)
        );

        let outcomes = settle(&mut loader);
        assert_eq!(
            outcomes,
            [AssetLoadOutcome::Ready(id("textures/badge.png"))]
        );
        assert_eq!(loader.get(&id("textures/badge.png")), Some(&12));
        assert_eq!(
            loader.status(&id("textures/badge.png")),
            Some(AssetStatus::Ready)
        );
    }

    /// The property the caller depends on: a scene naming one texture from
    /// twenty entities asks twenty times and loads once.
    #[test]
    fn asking_for_the_same_asset_twice_loads_it_once() {
        let mut loader = loader(&[("shared.png", b"bytes")]);
        for _ in 0..20 {
            loader.request(id("shared.png")).unwrap();
        }
        assert_eq!(loader.outstanding(), 1);
        assert_eq!(settle(&mut loader).len(), 1);
    }

    /// A missing file and undecodable bytes are both answers, and both name the
    /// asset they are about.
    #[test]
    fn a_failure_is_reported_once_and_names_the_asset() {
        let mut loader = loader(&[("bad.png", b"bad")]);
        loader.request(id("bad.png")).unwrap();
        loader.request(id("absent.png")).unwrap();

        let outcomes = settle(&mut loader);
        let mut failures: Vec<String> = outcomes
            .iter()
            .map(|outcome| outcome.id().to_string())
            .collect();
        failures.sort();
        assert_eq!(failures, ["absent.png", "bad.png"]);
        assert!(
            outcomes
                .iter()
                .all(|outcome| matches!(outcome, AssetLoadOutcome::Failed(_)))
        );
        assert_eq!(loader.status(&id("bad.png")), Some(AssetStatus::Failed));
        assert!(loader.error(&id("bad.png")).is_some());
    }

    /// Asking again for something that already failed is not a new load, or a
    /// caller that requests everything each frame retries forever.
    #[test]
    fn a_failure_is_not_retried_by_asking_again() {
        let mut loader = loader(&[]);
        loader.request(id("absent.png")).unwrap();
        settle(&mut loader);

        loader.request(id("absent.png")).unwrap();
        assert_eq!(loader.outstanding(), 0, "asking again did not reload it");

        loader.retry(&id("absent.png")).unwrap();
        assert_eq!(loader.outstanding(), 1, "retrying did");
        settle(&mut loader);
    }

    /// Retrying something that is fine must not restart it.
    #[test]
    fn retrying_a_working_asset_does_nothing() {
        let mut loader = loader(&[("fine.png", b"bytes")]);
        loader.request(id("fine.png")).unwrap();
        settle(&mut loader);

        loader.retry(&id("fine.png")).unwrap();
        assert_eq!(loader.outstanding(), 0);
        assert_eq!(loader.get(&id("fine.png")), Some(&5));
    }

    /// What a host needs when a scene is replaced: the list of assets nothing
    /// refers to any more, so it can drop what it built from them.
    #[test]
    fn retaining_releases_what_is_no_longer_wanted() {
        let mut loader = loader(&[("kept.png", b"kept"), ("dropped.png", b"dropped")]);
        loader.request(id("kept.png")).unwrap();
        loader.request(id("dropped.png")).unwrap();
        settle(&mut loader);

        let released = loader.retain(&BTreeSet::from([id("kept.png")]));
        assert_eq!(released, [id("dropped.png")]);
        assert_eq!(loader.get(&id("kept.png")), Some(&4));
        assert_eq!(loader.get(&id("dropped.png")), None);
        assert_eq!(loader.status(&id("dropped.png")), None);
        assert_eq!(
            loader.requested().collect::<Vec<_>>(),
            [&id("kept.png")],
            "and the loader is no longer holding it"
        );
    }

    /// What hot reload is: an asset that already loaded is loaded again, and the
    /// new bytes replace the old value.
    #[test]
    fn reloading_a_ready_asset_loads_it_again() {
        let mut source = MemoryAssetSource::default();
        source.insert(AssetBytes::new(id("art.png"), b"four".to_vec()));
        let mut loader = AssetLoader::new(source, AssetLoadQueueConfig::default(), CountingDecoder)
            .expect("the queue starts");

        loader.request(id("art.png")).unwrap();
        settle(&mut loader);
        assert_eq!(loader.get(&id("art.png")), Some(&4));

        loader.reload(&id("art.png")).unwrap();
        assert_eq!(loader.outstanding(), 1, "asking again is a new load");
        assert_eq!(
            settle(&mut loader),
            [AssetLoadOutcome::Ready(id("art.png"))],
            "and it is reported as arriving, so a host can take it"
        );
        assert_eq!(loader.get(&id("art.png")), Some(&4));
    }

    /// Unlike `request`, which is idempotent on purpose, `reload` is the caller
    /// saying the held value is wrong.
    #[test]
    fn reloading_is_not_refused_the_way_re_requesting_is() {
        let mut loader = loader(&[("art.png", b"bytes")]);
        loader.request(id("art.png")).unwrap();
        settle(&mut loader);

        loader.request(id("art.png")).unwrap();
        assert_eq!(loader.outstanding(), 0, "asking again does nothing");
        loader.reload(&id("art.png")).unwrap();
        assert_eq!(loader.outstanding(), 1, "saying it is stale does");
        settle(&mut loader);
    }

    /// A file that changes while its first read is in flight must not be
    /// enqueued twice.
    #[test]
    fn reloading_something_still_loading_leaves_it_alone() {
        let mut loader = loader(&[("art.png", b"bytes")]);
        loader.request(id("art.png")).unwrap();
        loader.reload(&id("art.png")).unwrap();
        assert_eq!(loader.outstanding(), 1);
        assert_eq!(settle(&mut loader).len(), 1);
    }

    /// An asset released while it was still loading must not come back and take
    /// up residence again.
    #[test]
    fn a_completion_for_a_released_asset_is_discarded() {
        let mut loader = loader(&[("leaving.png", b"bytes")]);
        loader.request(id("leaving.png")).unwrap();
        loader.retain(&BTreeSet::new());

        let outcomes = settle(&mut loader);
        assert!(
            outcomes.is_empty(),
            "nothing wanted it, so there is no news"
        );
        assert_eq!(loader.get(&id("leaving.png")), None);
        assert_eq!(loader.status(&id("leaving.png")), None);
    }
}
