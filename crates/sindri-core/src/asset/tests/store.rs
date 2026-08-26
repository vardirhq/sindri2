//! Requesting, loading, failing, and letting go.

use crate::{AssetLoadErrorKind, AssetStatus, AssetStore, AssetStoreError};

use super::support::{Texture, texture_id};

#[test]
fn duplicate_requests_share_one_live_generation() {
    let mut assets = AssetStore::<Texture>::default();
    let first = assets.request(texture_id());
    let second = assets.request(texture_id());

    assert_eq!(first, second);
    assert_eq!(first.strong_count(), 2);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets.status(&first).unwrap(), AssetStatus::Queued);
}

#[test]
fn load_state_is_explicit_and_checked() {
    let mut assets = AssetStore::<Texture>::default();
    let handle = assets.request(texture_id());

    assets.begin_loading(&handle).unwrap();
    assert_eq!(assets.status(&handle).unwrap(), AssetStatus::Loading);
    assert!(matches!(
        assets.retry(&handle),
        Err(AssetStoreError::InvalidTransition { .. })
    ));
    assets.complete(&handle, Texture("rgba pixels")).unwrap();
    assert_eq!(assets.status(&handle).unwrap(), AssetStatus::Ready);
    assert_eq!(assets.get(&handle).unwrap(), Some(&Texture("rgba pixels")));
}

#[test]
fn failures_keep_actionable_asset_context_and_can_retry() {
    let mut assets = AssetStore::<Texture>::default();
    let handle = assets.request(texture_id());
    assets.begin_loading(&handle).unwrap();
    assets
        .fail(
            &handle,
            AssetLoadErrorKind::NotFound,
            "no source contained this logical ID",
        )
        .unwrap();

    let error = assets.error(&handle).unwrap().unwrap();
    assert_eq!(error.id(), handle.id());
    assert_eq!(error.kind(), AssetLoadErrorKind::NotFound);
    assert!(error.to_string().contains("textures/player.png"));

    assets.retry(&handle).unwrap();
    assert_eq!(assets.status(&handle).unwrap(), AssetStatus::Queued);
    assert!(assets.error(&handle).unwrap().is_none());
}

#[test]
fn values_live_until_the_last_strong_handle_is_collected() {
    let mut assets = AssetStore::<Texture>::default();
    let first = assets.request(texture_id());
    let second = first.clone();
    let weak = first.downgrade();

    drop(first);
    assert!(assets.collect_unused().is_empty());
    assert!(weak.upgrade().is_some());

    drop(second);
    assert_eq!(assets.collect_unused(), vec![texture_id()]);
    assert!(assets.is_empty());
    assert!(weak.upgrade().is_none());
}

#[test]
fn a_new_request_cannot_revive_an_expired_weak_handle() {
    let mut assets = AssetStore::<Texture>::default();
    let first = assets.request(texture_id());
    let weak = first.downgrade();
    let first_generation = first.generation();
    drop(first);

    let replacement = assets.request(texture_id());
    assert_ne!(replacement.generation(), first_generation);
    assert!(weak.upgrade().is_none());
}

#[test]
fn handles_are_bound_to_the_store_that_created_them() {
    let mut first_store = AssetStore::<Texture>::default();
    let first = first_store.request(texture_id());
    let mut second_store = AssetStore::<Texture>::default();
    let second = second_store.request(texture_id());

    assert_ne!(first, second);
    assert!(matches!(
        second_store.status(&first),
        Err(AssetStoreError::InvalidHandle(_))
    ));
}
