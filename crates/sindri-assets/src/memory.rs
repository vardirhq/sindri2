use std::collections::BTreeMap;

use sindri_core::{AssetId, AssetLoadErrorKind};

use crate::{AssetBytes, AssetSource, AssetSourceError, AssetSourceFuture};

#[derive(Clone, Debug, Default)]
pub struct MemoryAssetSource {
    assets: BTreeMap<AssetId, AssetBytes>,
}

impl MemoryAssetSource {
    pub fn insert(&mut self, asset: AssetBytes) -> Option<AssetBytes> {
        self.assets.insert(asset.id().clone(), asset)
    }

    pub fn remove(&mut self, id: &AssetId) -> Option<AssetBytes> {
        self.assets.remove(id)
    }

    pub fn contains(&self, id: &AssetId) -> bool {
        self.assets.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

impl AssetSource for MemoryAssetSource {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn load<'a>(&'a self, id: &'a AssetId) -> AssetSourceFuture<'a> {
        Box::pin(async move {
            self.assets.get(id).cloned().ok_or_else(|| {
                AssetSourceError::new(
                    id.clone(),
                    self.name(),
                    AssetLoadErrorKind::NotFound,
                    "the logical ID is not present",
                )
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> AssetId {
        AssetId::new("scenes/room.json").unwrap()
    }

    #[test]
    fn memory_source_implements_the_object_safe_async_contract() {
        let mut source = MemoryAssetSource::default();
        source.insert(
            AssetBytes::new(id(), br#"{"format_version":1}"#.to_vec())
                .with_content_type("application/json"),
        );
        let source: Box<dyn AssetSource> = Box::new(source);
        let asset_id = id();

        let loaded = pollster::block_on(source.load(&asset_id)).unwrap();
        assert_eq!(loaded.id(), &asset_id);
        assert_eq!(loaded.content_type(), Some("application/json"));
        assert_eq!(loaded.as_slice(), br#"{"format_version":1}"#);
    }

    #[test]
    fn memory_source_returns_a_contextual_missing_error() {
        let source = MemoryAssetSource::default();
        let asset_id = id();
        let error = pollster::block_on(source.load(&asset_id)).unwrap_err();

        assert_eq!(error.id(), &asset_id);
        assert_eq!(error.source_name(), "memory");
        assert_eq!(error.kind(), AssetLoadErrorKind::NotFound);
    }
}
