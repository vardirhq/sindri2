//! What the queue does with a request that outlives its handle.

use std::{thread, time::Duration};

use sindri_core::{AssetStatus, AssetStore};

use super::*;
use crate::MemoryAssetSource;

fn id(value: &str) -> AssetId {
    AssetId::new(value).unwrap()
}

fn wait_for_completions(queue: &mut AssetLoadQueue, expected: usize) -> Vec<AssetLoadCompletion> {
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
    let mut queue = AssetLoadQueue::new(
        MemoryAssetSource::default(),
        AssetLoadQueueConfig::new(1, 4),
    )
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
