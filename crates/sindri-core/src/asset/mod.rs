//! Naming an asset, asking for it, and holding what came back.
//!
//! An [`AssetId`] names an asset without saying where it is; an
//! [`AssetStore`] turns one into an [`AssetHandle`] and tracks what has
//! happened to it since. The store is the only thing that decides a handle is
//! still good, which is why loading can be genuinely asynchronous without a
//! caller ever reading a value that has since been replaced.

mod handle;
mod id;
mod sprite;
mod status;
mod store;

#[cfg(test)]
mod tests;

pub use handle::{AssetHandle, WeakAssetHandle};
pub use id::{AssetId, AssetIdError};
pub use sprite::{SpriteRef, SpriteRefError};
pub use status::{AssetLoadError, AssetLoadErrorKind, AssetStatus};
pub use store::{AssetStore, AssetStoreError};
