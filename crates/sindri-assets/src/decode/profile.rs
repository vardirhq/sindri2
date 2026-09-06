//! Reading a reusable profile out of fetched bytes.

use sindri_core::{AssetLoadErrorKind, ProfileDocument};

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProfileAssetDecoder;

impl AssetDecoder for ProfileAssetDecoder {
    type Asset = ProfileDocument;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "profile",
                AssetLoadErrorKind::InvalidData,
                format!("not UTF-8: {error}"),
            )
        })?;
        ProfileDocument::from_json(text).map_err(|error| {
            AssetDecodeError::new(
                id,
                "profile",
                AssetLoadErrorKind::InvalidData,
                error.to_string(),
            )
        })
    }
}
