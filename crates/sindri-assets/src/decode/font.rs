//! Fonts: validated, and asked what family they declare.
//!
//! A project font is an asset. The family name comes out of the file
//! rather than from an operating-system lookup, which is what makes a
//! project render the same on a machine that has never seen it.

use sindri_core::AssetLoadErrorKind;

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

/// A font kept as bytes so the renderer can load the same face natively and in
/// a browser, with its declared family recorded for deterministic selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontAsset {
    bytes: Vec<u8>,
    family: String,
}

impl FontAsset {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn family(&self) -> &str {
        &self.family
    }
}

/// Validates an OpenType font and records the first face's family.
///
/// Fonts are loaded from bytes rather than from an operating-system database.
/// That is the portability rule: a scene names one asset and the browser sees
/// the same outlines as the native game and editor.
#[derive(Clone, Copy, Debug, Default)]
pub struct FontAssetDecoder;

impl AssetDecoder for FontAssetDecoder {
    type Asset = FontAsset;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let data = bytes.as_slice().to_vec();
        let mut database = fontdb::Database::new();
        database.load_font_data(data.clone());
        let family = database
            .faces()
            .next()
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
            .ok_or_else(|| {
                AssetDecodeError::new(
                    id.clone(),
                    "font",
                    AssetLoadErrorKind::InvalidData,
                    "the bytes contain no readable OpenType face",
                )
            })?;
        Ok(FontAsset {
            bytes: data,
            family,
        })
    }
}
