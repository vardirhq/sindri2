//! Tile sets, validated as reusable semantic assets.

use sindri_core::{AssetLoadErrorKind, TileSetDocument};

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

#[derive(Clone, Copy, Debug, Default)]
pub struct TileSetAssetDecoder;

impl AssetDecoder for TileSetAssetDecoder {
    type Asset = TileSetDocument;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "tile set",
                AssetLoadErrorKind::InvalidData,
                format!("not valid UTF-8: {error}"),
            )
        })?;
        TileSetDocument::from_json(text).map_err(|error| {
            AssetDecodeError::new(
                id,
                "tile set",
                AssetLoadErrorKind::InvalidData,
                error.to_string(),
            )
        })
    }
}
