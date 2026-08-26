//! The asset type and id the store tests use.

use crate::AssetId;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Texture(pub(super) &'static str);

pub(super) fn texture_id() -> AssetId {
    AssetId::new("textures/player.png").unwrap()
}
