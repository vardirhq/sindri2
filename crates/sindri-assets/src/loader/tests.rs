//! What a loader does with a second request, a failure, and a reload.

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

/// What a manifest is for: bytes that are not what the project promised are
/// refused, rather than decoded into a picture from last week.
#[test]
fn bytes_that_disagree_with_the_manifest_are_refused() {
    let mut manifest = AssetManifest::new();
    manifest.insert(id("art.png"), b"what the project shipped");

    let mut loader = loader(&[("art.png", b"something else entirely")]).with_manifest(manifest);
    loader.request(id("art.png")).unwrap();

    let outcomes = settle(&mut loader);
    assert_eq!(outcomes.len(), 1);
    let AssetLoadOutcome::Failed(error) = &outcomes[0] else {
        panic!("a substituted asset must not load: {outcomes:?}");
    };
    assert_eq!(error.id(), &id("art.png"));
    assert_eq!(loader.status(&id("art.png")), Some(AssetStatus::Failed));
    assert_eq!(loader.get(&id("art.png")), None);
}

/// The bytes the manifest describes load exactly as they would without one,
/// and an asset it never mentions is not forbidden for being absent.
#[test]
fn a_manifest_does_not_stand_in_the_way_of_what_matches_it() {
    let mut manifest = AssetManifest::new();
    manifest.insert(id("listed.png"), b"listed bytes");

    let mut loader = loader(&[
        ("listed.png", b"listed bytes"),
        ("unlisted.png", b"generated later"),
    ])
    .with_manifest(manifest);
    loader.request(id("listed.png")).unwrap();
    loader.request(id("unlisted.png")).unwrap();

    let outcomes = settle(&mut loader);
    assert!(
        outcomes
            .iter()
            .all(|outcome| matches!(outcome, AssetLoadOutcome::Ready(_))),
        "{outcomes:?}"
    );
    assert_eq!(loader.get(&id("listed.png")), Some(&12));
    assert_eq!(loader.get(&id("unlisted.png")), Some(&15));
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
