//! Reading a prefab document out of the bytes a host fetched.

use sindri_core::{AssetLoadErrorKind, PrefabDocument};

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

/// Turns fetched bytes into a prefab, or says why they are not one.
///
/// Separate from the scene decoder although both parse JSON entities: the two
/// documents have separate format versions and separate histories, and a
/// decoder that accepted either would let a scene be spawned as a prefab and a
/// prefab be opened as a world.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrefabAssetDecoder;

impl AssetDecoder for PrefabAssetDecoder {
    type Asset = PrefabDocument;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "prefab",
                AssetLoadErrorKind::InvalidData,
                format!("not UTF-8: {error}"),
            )
        })?;
        // `from_json` validates as it reads — the one-root rule included — so a
        // prefab that would fail to spawn is refused when it arrives rather
        // than on the frame a script asks for it.
        PrefabDocument::from_json(text).map_err(|error| {
            AssetDecodeError::new(
                id,
                "prefab",
                AssetLoadErrorKind::InvalidData,
                error.to_string(),
            )
        })
    }
}
