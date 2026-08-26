//! Plain text, which must be UTF-8 to be text.

use sindri_core::AssetLoadErrorKind;

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

/// Decodes an asset that is just text, such as a `.decay` script.
///
/// Deliberately knows nothing about what the text says. A script is source to
/// whoever compiles it and bytes to the pipeline that fetches it, and keeping
/// the seam there is what lets `sindri-decay` do no I/O at all.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextAssetDecoder;

impl AssetDecoder for TextAssetDecoder {
    type Asset = String;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        String::from_utf8(bytes.as_slice().to_vec()).map_err(|error| {
            AssetDecodeError::new(
                id,
                "text",
                AssetLoadErrorKind::InvalidData,
                error.to_string(),
            )
        })
    }
}
