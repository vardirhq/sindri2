//! What each decoder accepts, and what it refuses.

#[test]
fn a_font_is_validated_and_names_its_family() {
    let decoder = super::FontAssetDecoder;
    let id: AssetId = "fonts/Inter.ttf".parse().unwrap();
    let font = decoder
        .decode(AssetBytes::new(
            id.clone(),
            include_bytes!("../../../../editor/assets/Inter.ttf").to_vec(),
        ))
        .unwrap();
    assert_eq!(font.family(), "Inter");
    assert!(!font.bytes().is_empty());
    assert!(
        decoder
            .decode(AssetBytes::new(id, b"not a font".to_vec()))
            .is_err()
    );
}

#[test]
fn text_decodes_as_utf8_and_refuses_what_is_not() {
    let decoder = super::TextAssetDecoder;
    let id: AssetId = "scripts/spin.decay".parse().unwrap();
    assert_eq!(
        decoder
            .decode(AssetBytes::new(id.clone(), b"script Spin {}".to_vec()))
            .unwrap(),
        "script Spin {}"
    );
    assert!(
        decoder
            .decode(AssetBytes::new(id, vec![0xff, 0xfe]))
            .is_err()
    );
}

use std::collections::BTreeMap;

#[cfg(not(target_arch = "wasm32"))]
use std::{thread, time::Duration};

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use sindri_core::{AssetStatus, SceneDocument, SceneEntity, SceneEntityId, SceneMetadata};

use super::texture::TextureAssetDecoder;
use super::*;

#[cfg(not(target_arch = "wasm32"))]
use crate::{AssetLoadQueue, AssetLoadQueueConfig, MemoryAssetSource};

fn id(value: &str) -> AssetId {
    AssetId::new(value).unwrap()
}

fn png_bytes() -> Vec<u8> {
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(
            &[255, 0, 0, 255, 0, 255, 0, 128],
            2,
            1,
            ExtendedColorType::Rgba8,
        )
        .unwrap();
    encoded
}

fn scene() -> SceneDocument {
    SceneDocument {
        metadata: SceneMetadata {
            name: Some("Loaded room".into()),
            editor: BTreeMap::new(),
        },
        entities: vec![SceneEntity {
            name: Some("Player".into()),
            ..SceneEntity::new(SceneEntityId::new("player").unwrap())
        }],
        ..SceneDocument::default()
    }
}

#[test]
fn decodes_png_pixels_to_rgba8() {
    let texture = TextureAssetDecoder
        .decode(AssetBytes::new(id("textures/test.png"), png_bytes()))
        .unwrap();

    assert_eq!(texture.width(), 2);
    assert_eq!(texture.height(), 1);
    assert_eq!(texture.rgba8(), &[255, 0, 0, 255, 0, 255, 0, 128]);
}

#[test]
fn rejects_invalid_texture_data_with_asset_context() {
    let error = TextureAssetDecoder
        .decode(AssetBytes::new(id("textures/broken.png"), vec![1, 2, 3]))
        .unwrap_err();

    assert_eq!(error.id(), &id("textures/broken.png"));
    assert_eq!(error.kind(), AssetLoadErrorKind::UnsupportedFormat);
}

#[test]
fn decodes_and_validates_scene_json() {
    let expected = scene();
    let bytes = serde_json::to_vec(&expected).unwrap();
    let decoded = SceneAssetDecoder
        .decode(AssetBytes::new(id("scenes/room.json"), bytes))
        .unwrap();
    assert_eq!(decoded, expected);
}

#[test]
fn rejects_structurally_invalid_scenes() {
    // Written against whatever the current format is, so a version bump
    // does not turn this into a test that the version was rejected.
    let json = format!(
        r#"{{
        "format_version": {},
        "entities": [{{ "id": "child", "parent": "missing" }}]
    }}"#,
        sindri_core::SCENE_FORMAT_VERSION
    );
    let error = SceneAssetDecoder
        .decode(AssetBytes::new(id("scenes/broken.json"), json.into_bytes()))
        .unwrap_err();

    assert_eq!(error.kind(), AssetLoadErrorKind::InvalidData);
    assert!(error.message().contains("missing parent"));
}

#[test]
fn decoded_completion_applies_only_to_its_handle_generation() {
    let mut store = AssetStore::<SceneDocument>::default();
    let expired = store.request(id("scenes/room.json"));
    store.begin_loading(&expired).unwrap();
    let request = AssetLoadRequest::new(&expired);
    drop(expired);
    store.collect_unused();

    let replacement = store.request(id("scenes/room.json"));
    let completion = DecodedAssetCompletion {
        request,
        result: Ok(scene()),
    };
    assert!(matches!(
        completion.apply(&mut store, &replacement),
        Err(AssetCompletionApplyError::Stale { .. })
    ));
    assert_eq!(store.status(&replacement).unwrap(), AssetStatus::Queued);
}

#[test]
fn decoded_completion_drives_the_store_to_ready() {
    let mut store = AssetStore::<SceneDocument>::default();
    let handle = store.request(id("scenes/room.json"));
    store.begin_loading(&handle).unwrap();
    let completion = DecodedAssetCompletion {
        request: AssetLoadRequest::new(&handle),
        result: Ok(scene()),
    };

    completion.apply(&mut store, &handle).unwrap();
    assert_eq!(store.status(&handle).unwrap(), AssetStatus::Ready);
    assert_eq!(
        store
            .get(&handle)
            .unwrap()
            .unwrap()
            .metadata
            .name
            .as_deref(),
        Some("Loaded room")
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_scene_bytes_decode_and_apply_end_to_end() {
    let expected = scene();
    let mut source = MemoryAssetSource::default();
    source.insert(AssetBytes::new(
        id("scenes/room.json"),
        serde_json::to_vec(&expected).unwrap(),
    ));
    let mut queue = AssetLoadQueue::new(source, AssetLoadQueueConfig::new(1, 4)).unwrap();
    let mut store = AssetStore::<SceneDocument>::default();
    let handle = store.request(id("scenes/room.json"));

    queue.enqueue(AssetLoadRequest::new(&handle)).unwrap();
    store.begin_loading(&handle).unwrap();
    let completion = (0..100)
        .find_map(|_| {
            let completion = queue.drain().pop();
            if completion.is_none() {
                thread::sleep(Duration::from_millis(5));
            }
            completion
        })
        .expect("timed out waiting for queued scene bytes");

    decode_completion(completion, &SceneAssetDecoder)
        .apply(&mut store, &handle)
        .unwrap();
    assert_eq!(store.status(&handle).unwrap(), AssetStatus::Ready);
    assert_eq!(store.get(&handle).unwrap(), Some(&expected));
}
