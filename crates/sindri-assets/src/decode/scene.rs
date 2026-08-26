//! Scene documents, validated as they are decoded.

use sindri_core::{AssetLoadErrorKind, SceneDocument};

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

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
