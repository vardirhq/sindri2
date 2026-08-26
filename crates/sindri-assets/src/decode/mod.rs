//! Turning bytes into an asset.
//!
//! One file per asset kind, each with the decoder for it. A new kind is
//! a file here and its `AssetDecoder` impl; nothing already decoding
//! changes.

mod font;
mod scene;
mod sheet;
mod text;
mod texture;

#[cfg(test)]
mod tests;

pub use font::{FontAsset, FontAssetDecoder};
pub use scene::SceneAssetDecoder;
pub use sheet::SpriteSheetAssetDecoder;
pub use text::TextAssetDecoder;
pub use texture::{TextureAsset, TextureAssetDecoder};

use sindri_core::{
    AssetHandle, AssetId, AssetLoadError, AssetLoadErrorKind, AssetStore, AssetStoreError,
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
