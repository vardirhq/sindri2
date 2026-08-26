//! Sprite sheets, validated as they are decoded.

use sindri_core::{AssetLoadErrorKind, SpriteSheetDocument};

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

/// Decodes the sidecar that slices a texture into named sprites.
///
/// Its own decoder rather than text a caller parses, for the reason the scene
/// has one: a sheet that is malformed should fail as the asset it is, naming
/// itself, rather than arriving as a string that fails somewhere later with no
/// idea where it came from.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpriteSheetAssetDecoder;

impl AssetDecoder for SpriteSheetAssetDecoder {
    type Asset = SpriteSheetDocument;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "sprite sheet",
                AssetLoadErrorKind::InvalidData,
                format!("not valid UTF-8: {error}"),
            )
        })?;
        SpriteSheetDocument::from_json(text).map_err(|error| {
            AssetDecodeError::new(
                id,
                "sprite sheet",
                AssetLoadErrorKind::InvalidData,
                error.to_string(),
            )
        })
    }
}
