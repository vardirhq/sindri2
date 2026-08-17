use sindri_core::{
    AssetHandle, AssetId, AssetLoadError, AssetLoadErrorKind, AssetStore, AssetStoreError,
    SceneDocument,
};
use thiserror::Error;

use crate::{AssetBytes, AssetLoadCompletion, AssetLoadRequest};

pub trait AssetDecoder {
    type Asset;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError>;
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} while decoding {asset_type} asset '{id}': {message}")]
pub struct AssetDecodeError {
    id: AssetId,
    asset_type: &'static str,
    kind: AssetLoadErrorKind,
    message: String,
}

impl AssetDecodeError {
    pub fn new(
        id: AssetId,
        asset_type: &'static str,
        kind: AssetLoadErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            asset_type,
            kind,
            message: message.into(),
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn asset_type(&self) -> &'static str {
        self.asset_type
    }

    pub const fn kind(&self) -> AssetLoadErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<AssetDecodeError> for AssetLoadError {
    fn from(error: AssetDecodeError) -> Self {
        Self::new(
            error.id,
            error.kind,
            format!("could not decode {}: {}", error.asset_type, error.message),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureAsset {
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

impl TextureAsset {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }

    pub fn into_rgba8(self) -> Vec<u8> {
        self.rgba8
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureAssetDecoder;

impl AssetDecoder for TextureAssetDecoder {
    type Asset = TextureAsset;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let image = image::load_from_memory(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "texture",
                image_error_kind(&error),
                error.to_string(),
            )
        })?;
        let image = image.into_rgba8();
        Ok(TextureAsset {
            width: image.width(),
            height: image.height(),
            rgba8: image.into_raw(),
        })
    }
}

fn image_error_kind(error: &image::ImageError) -> AssetLoadErrorKind {
    match error {
        image::ImageError::Unsupported(_) => AssetLoadErrorKind::UnsupportedFormat,
        image::ImageError::Decoding(_) | image::ImageError::Limits(_) => {
            AssetLoadErrorKind::InvalidData
        }
        image::ImageError::IoError(_) => AssetLoadErrorKind::Io,
        image::ImageError::Encoding(_) | image::ImageError::Parameter(_) => {
            AssetLoadErrorKind::Other
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SceneAssetDecoder;

impl AssetDecoder for SceneAssetDecoder {
    type Asset = SceneDocument;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let scene = serde_json::from_slice::<SceneDocument>(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "scene",
                AssetLoadErrorKind::InvalidData,
                format!("invalid JSON: {error}"),
            )
        })?;
        scene.validate().map_err(|error| {
            AssetDecodeError::new(
                id,
                "scene",
                AssetLoadErrorKind::InvalidData,
                error.to_string(),
            )
        })?;
        Ok(scene)
    }
}

#[derive(Debug)]
pub struct DecodedAssetCompletion<T> {
    request: AssetLoadRequest,
    result: Result<T, AssetLoadError>,
}

impl<T> DecodedAssetCompletion<T> {
    pub fn request(&self) -> &AssetLoadRequest {
        &self.request
    }

    pub fn result(&self) -> Result<&T, &AssetLoadError> {
        self.result.as_ref()
    }

    pub fn into_result(self) -> Result<T, AssetLoadError> {
        self.result
    }

    pub fn apply(
        self,
        store: &mut AssetStore<T>,
        handle: &AssetHandle<T>,
    ) -> Result<(), AssetCompletionApplyError> {
        if !self.request.matches(handle) {
            return Err(AssetCompletionApplyError::Stale {
                id: self.request.id().clone(),
                completed_generation: self.request.generation(),
                current_generation: handle.generation(),
            });
        }

        match self.result {
            Ok(asset) => store.complete(handle, asset)?,
            Err(error) => store.fail(handle, error.kind(), error.message())?,
        }
        Ok(())
    }
}

pub fn decode_completion<D: AssetDecoder>(
    completion: AssetLoadCompletion,
    decoder: &D,
) -> DecodedAssetCompletion<D::Asset> {
    let (request, result) = completion.into_parts();
    let result = result
        .map_err(AssetLoadError::from)
        .and_then(|bytes| decoder.decode(bytes).map_err(AssetLoadError::from));
    DecodedAssetCompletion { request, result }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssetCompletionApplyError {
    #[error(
        "stale completion for asset '{id}' generation {completed_generation}; current generation is {current_generation}"
    )]
    Stale {
        id: AssetId,
        completed_generation: u64,
        current_generation: u64,
    },
    #[error(transparent)]
    Store(#[from] AssetStoreError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    #[cfg(not(target_arch = "wasm32"))]
    use std::{thread, time::Duration};

    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    use sindri_core::{AssetStatus, SceneEntity, SceneEntityId, SceneMetadata};

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
        let json = br#"{
            "format_version": 2,
            "entities": [{ "id": "child", "parent": "missing" }]
        }"#;
        let error = SceneAssetDecoder
            .decode(AssetBytes::new(id("scenes/broken.json"), json.to_vec()))
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
}
