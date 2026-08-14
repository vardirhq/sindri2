use std::collections::BTreeSet;

#[cfg(target_arch = "wasm32")]
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

#[cfg(target_arch = "wasm32")]
use futures::{Stream, stream::FuturesUnordered};
use sindri_core::{AssetHandle, AssetId};
use thiserror::Error;

use crate::{AssetBytes, AssetSource, AssetSourceError};

/// Identity carried through an asynchronous load operation.
///
/// The generation prevents a completion for an expired handle from being
/// mistaken for a later request of the same logical asset ID.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AssetLoadRequest {
    id: AssetId,
    generation: u64,
}

impl AssetLoadRequest {
    pub fn new<T>(handle: &AssetHandle<T>) -> Self {
        Self {
            id: handle.id().clone(),
            generation: handle.generation(),
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn matches<T>(&self, handle: &AssetHandle<T>) -> bool {
        self.id == *handle.id() && self.generation == handle.generation()
    }
}

#[derive(Debug)]
pub struct AssetLoadCompletion {
    request: AssetLoadRequest,
    result: Result<AssetBytes, AssetSourceError>,
}

impl AssetLoadCompletion {
    pub fn request(&self) -> &AssetLoadRequest {
        &self.request
    }

    pub fn result(&self) -> Result<&AssetBytes, &AssetSourceError> {
        self.result.as_ref()
    }

    pub fn into_result(self) -> Result<AssetBytes, AssetSourceError> {
        self.result
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetLoadQueueConfig {
    pub max_concurrent: usize,
    pub capacity: usize,
}

impl AssetLoadQueueConfig {
    pub const fn new(max_concurrent: usize, capacity: usize) -> Self {
        Self {
            max_concurrent,
            capacity,
        }
    }
}

impl Default for AssetLoadQueueConfig {
    fn default() -> Self {
        Self::new(2, 64)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssetLoadQueueCreateError {
    #[error("asset load queue concurrency must be greater than zero")]
    ZeroConcurrency,
    #[error("asset load queue capacity must be greater than zero")]
    ZeroCapacity,
    #[cfg(not(target_arch = "wasm32"))]
    #[error("failed to spawn asset I/O worker: {0}")]
    WorkerSpawn(String),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssetLoadQueueError {
    #[error("asset request '{0}' is already queued or loading")]
    Duplicate(AssetId),
    #[error("asset load queue is full (capacity {capacity})")]
    Full { capacity: usize },
    #[error("asset load queue workers have stopped")]
    Closed,
}

/// Bounded cross-platform queue for loading undecoded asset bytes.
///
/// Native builds create and poll source futures on dedicated I/O workers, so a
/// blocking filesystem source never runs on the frame thread. WebAssembly
/// builds retain futures locally and advance them from [`Self::drain`], allowing
/// the browser Fetch API to remain genuinely asynchronous.
pub struct AssetLoadQueue {
    config: AssetLoadQueueConfig,
    outstanding: BTreeSet<AssetLoadRequest>,
    #[cfg(not(target_arch = "wasm32"))]
    task_sender: Option<SyncSender<AssetLoadRequest>>,
    #[cfg(not(target_arch = "wasm32"))]
    completion_receiver: Receiver<AssetLoadCompletion>,
    #[cfg(not(target_arch = "wasm32"))]
    workers: Vec<JoinHandle<()>>,
    #[cfg(target_arch = "wasm32")]
    source: Rc<dyn AssetSource>,
    #[cfg(target_arch = "wasm32")]
    waiting: VecDeque<AssetLoadRequest>,
    #[cfg(target_arch = "wasm32")]
    active: FuturesUnordered<LocalLoadFuture>,
}

#[cfg(target_arch = "wasm32")]
type LocalLoadFuture = Pin<Box<dyn Future<Output = AssetLoadCompletion> + 'static>>;

impl AssetLoadQueue {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<S>(
        source: S,
        config: AssetLoadQueueConfig,
    ) -> Result<Self, AssetLoadQueueCreateError>
    where
        S: AssetSource + Send + Sync + 'static,
    {
        validate_config(config)?;

        let source: Arc<dyn AssetSource + Send + Sync> = Arc::new(source);
        let (task_sender, task_receiver) = mpsc::sync_channel::<AssetLoadRequest>(config.capacity);
        let task_receiver = Arc::new(Mutex::new(task_receiver));
        let (completion_sender, completion_receiver) = mpsc::channel();
        let mut workers = Vec::with_capacity(config.max_concurrent);

        for index in 0..config.max_concurrent {
            let source = Arc::clone(&source);
            let task_receiver = Arc::clone(&task_receiver);
            let completion_sender = completion_sender.clone();
            let spawn = thread::Builder::new()
                .name(format!("sindri-asset-io-{index}"))
                .spawn(move || {
                    loop {
                        let request = {
                            let receiver = task_receiver
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            receiver.recv()
                        };
                        let Ok(request) = request else {
                            break;
                        };
                        let result = futures::executor::block_on(source.load(request.id()));
                        if completion_sender
                            .send(AssetLoadCompletion { request, result })
                            .is_err()
                        {
                            break;
                        }
                    }
                });

            match spawn {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    drop(task_sender);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(AssetLoadQueueCreateError::WorkerSpawn(error.to_string()));
                }
            }
        }

        Ok(Self {
            config,
            outstanding: BTreeSet::new(),
            task_sender: Some(task_sender),
            completion_receiver,
            workers,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new<S>(
        source: S,
        config: AssetLoadQueueConfig,
    ) -> Result<Self, AssetLoadQueueCreateError>
    where
        S: AssetSource + 'static,
    {
        validate_config(config)?;
        Ok(Self {
            config,
            outstanding: BTreeSet::new(),
            source: Rc::new(source),
            waiting: VecDeque::new(),
            active: FuturesUnordered::new(),
        })
    }

    pub fn enqueue(&mut self, request: AssetLoadRequest) -> Result<(), AssetLoadQueueError> {
        if self.outstanding.contains(&request) {
            return Err(AssetLoadQueueError::Duplicate(request.id().clone()));
        }
        if self.outstanding.len() >= self.config.capacity {
            return Err(AssetLoadQueueError::Full {
                capacity: self.config.capacity,
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let sender = self
                .task_sender
                .as_ref()
                .ok_or(AssetLoadQueueError::Closed)?;
            match sender.try_send(request.clone()) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    return Err(AssetLoadQueueError::Full {
                        capacity: self.config.capacity,
                    });
                }
                Err(TrySendError::Disconnected(_)) => return Err(AssetLoadQueueError::Closed),
            }
        }

        #[cfg(target_arch = "wasm32")]
        self.waiting.push_back(request.clone());

        self.outstanding.insert(request);
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<AssetLoadCompletion> {
        #[cfg(not(target_arch = "wasm32"))]
        let completions = self.completion_receiver.try_iter().collect::<Vec<_>>();

        #[cfg(target_arch = "wasm32")]
        let completions = {
            self.start_waiting();
            let waker = Waker::noop();
            let mut context = Context::from_waker(waker);
            let mut completions = Vec::new();
            while let Poll::Ready(Some(completion)) =
                Pin::new(&mut self.active).poll_next(&mut context)
            {
                completions.push(completion);
                self.start_waiting();
            }
            completions
        };

        for completion in &completions {
            self.outstanding.remove(completion.request());
        }
        completions
    }

    pub fn outstanding(&self) -> usize {
        self.outstanding.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outstanding.is_empty()
    }

    pub const fn capacity(&self) -> usize {
        self.config.capacity
    }

    #[cfg(target_arch = "wasm32")]
    fn start_waiting(&mut self) {
        while self.active.len() < self.config.max_concurrent {
            let Some(request) = self.waiting.pop_front() else {
                break;
            };
            let source = Rc::clone(&self.source);
            self.active.push(Box::pin(async move {
                let result = source.load(request.id()).await;
                AssetLoadCompletion { request, result }
            }));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for AssetLoadQueue {
    fn drop(&mut self) {
        self.task_sender.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn validate_config(config: AssetLoadQueueConfig) -> Result<(), AssetLoadQueueCreateError> {
    if config.max_concurrent == 0 {
        return Err(AssetLoadQueueCreateError::ZeroConcurrency);
    }
    if config.capacity == 0 {
        return Err(AssetLoadQueueCreateError::ZeroCapacity);
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::{thread, time::Duration};

    use sindri_core::{AssetStatus, AssetStore};

    use super::*;
    use crate::MemoryAssetSource;

    fn id(value: &str) -> AssetId {
        AssetId::new(value).unwrap()
    }

    fn wait_for_completions(
        queue: &mut AssetLoadQueue,
        expected: usize,
    ) -> Vec<AssetLoadCompletion> {
        let mut completions = Vec::new();
        for _ in 0..100 {
            completions.extend(queue.drain());
            if completions.len() == expected {
                return completions;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out waiting for {expected} asset load completions");
    }

    #[test]
    fn native_workers_load_without_polling_sources_on_the_caller() {
        let mut source = MemoryAssetSource::default();
        source.insert(AssetBytes::new(id("textures/player.png"), vec![1, 2, 3, 4]));
        let mut queue = AssetLoadQueue::new(source, AssetLoadQueueConfig::new(2, 8)).unwrap();
        let mut store = AssetStore::<AssetBytes>::default();
        let handle = store.request(id("textures/player.png"));
        let request = AssetLoadRequest::new(&handle);

        queue.enqueue(request.clone()).unwrap();
        store.begin_loading(&handle).unwrap();
        assert_eq!(queue.outstanding(), 1);
        assert_eq!(store.status(&handle).unwrap(), AssetStatus::Loading);

        let mut completions = wait_for_completions(&mut queue, 1);
        let completion = completions.pop().unwrap();
        assert_eq!(completion.request(), &request);
        let bytes = completion.into_result().unwrap();
        store.complete(&handle, bytes).unwrap();

        assert!(queue.is_empty());
        assert_eq!(store.status(&handle).unwrap(), AssetStatus::Ready);
        assert_eq!(
            store.get(&handle).unwrap().unwrap().as_slice(),
            &[1, 2, 3, 4]
        );
    }

    #[test]
    fn completions_preserve_source_errors_and_handle_generations() {
        let mut queue =
            AssetLoadQueue::new(MemoryAssetSource::default(), AssetLoadQueueConfig::new(1, 4))
                .unwrap();
        let mut store = AssetStore::<AssetBytes>::default();
        let handle = store.request(id("missing.bin"));
        let request = AssetLoadRequest::new(&handle);

        queue.enqueue(request.clone()).unwrap();
        store.begin_loading(&handle).unwrap();
        let completion = wait_for_completions(&mut queue, 1).pop().unwrap();
        let error = completion.into_result().unwrap_err();

        assert!(request.matches(&handle));
        assert_eq!(error.id(), handle.id());
        store
            .fail(&handle, error.kind(), error.to_string())
            .unwrap();
        assert_eq!(store.status(&handle).unwrap(), AssetStatus::Failed);
    }

    #[test]
    fn an_expired_request_cannot_complete_a_replacement_generation() {
        let mut source = MemoryAssetSource::default();
        source.insert(AssetBytes::new(id("shared.bin"), vec![7]));
        let mut queue = AssetLoadQueue::new(source, AssetLoadQueueConfig::new(1, 4)).unwrap();
        let mut store = AssetStore::<AssetBytes>::default();
        let expired = store.request(id("shared.bin"));
        let expired_request = AssetLoadRequest::new(&expired);

        queue.enqueue(expired_request.clone()).unwrap();
        store.begin_loading(&expired).unwrap();
        drop(expired);
        assert_eq!(store.collect_unused(), vec![id("shared.bin")]);

        let replacement = store.request(id("shared.bin"));
        let completion = wait_for_completions(&mut queue, 1).pop().unwrap();

        assert_eq!(completion.request(), &expired_request);
        assert!(!completion.request().matches(&replacement));
        assert_eq!(store.status(&replacement).unwrap(), AssetStatus::Queued);
    }

    #[test]
    fn duplicate_and_over_capacity_requests_are_rejected_without_blocking() {
        let mut queue = AssetLoadQueue::new(
            MemoryAssetSource::default(),
            AssetLoadQueueConfig::new(1, 1),
        )
        .unwrap();
        let mut store = AssetStore::<AssetBytes>::default();
        let first = store.request(id("first.bin"));
        let first_request = AssetLoadRequest::new(&first);

        queue.enqueue(first_request.clone()).unwrap();
        assert_eq!(
            queue.enqueue(first_request),
            Err(AssetLoadQueueError::Duplicate(id("first.bin")))
        );

        let second = store.request(id("second.bin"));
        assert_eq!(
            queue.enqueue(AssetLoadRequest::new(&second)),
            Err(AssetLoadQueueError::Full { capacity: 1 })
        );
    }

    #[test]
    fn invalid_queue_limits_are_reported() {
        assert!(matches!(
            AssetLoadQueue::new(
                MemoryAssetSource::default(),
                AssetLoadQueueConfig::new(0, 1)
            ),
            Err(AssetLoadQueueCreateError::ZeroConcurrency)
        ));
        assert!(matches!(
            AssetLoadQueue::new(
                MemoryAssetSource::default(),
                AssetLoadQueueConfig::new(1, 0)
            ),
            Err(AssetLoadQueueCreateError::ZeroCapacity)
        ));
    }
}
