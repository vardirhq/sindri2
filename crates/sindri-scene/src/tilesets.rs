//! Runtime bindings for reusable tile-set assets.

use std::collections::{BTreeMap, BTreeSet};

use sindri_core::{
    AssetId, SceneComponent, SpriteRef, TileSetDocument, TileSetError, World, sheet_id_for,
};

use crate::TileVolumeComponent;

#[derive(Clone, Debug, Default)]
pub struct TileSetBindings {
    bound: BTreeMap<String, TileSetDocument>,
}

/// Texture references used by a decoded tile set's baked faces.
pub fn tile_set_textures(tile_set: &TileSetDocument) -> BTreeSet<String> {
    tile_set
        .tiles
        .values()
        .flat_map(|tile| tile.faces.iter())
        .filter_map(|(_, visual)| SpriteRef::parse(&visual.sprite).ok())
        .map(|reference| reference.texture().to_owned())
        .collect()
}

/// Sheet IDs and the textures they slice for every named baked face.
pub fn tile_set_sheets(tile_set: &TileSetDocument) -> BTreeMap<AssetId, String> {
    tile_set
        .tiles
        .values()
        .flat_map(|tile| tile.faces.iter())
        .filter_map(|(_, visual)| SpriteRef::parse(&visual.sprite).ok())
        .filter(|reference| reference.sprite().is_some())
        .filter_map(|reference| {
            let texture = reference.asset()?;
            Some((sheet_id_for(&texture)?, reference.texture().to_owned()))
        })
        .collect()
}

impl TileSetBindings {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(
        &mut self,
        reference: impl Into<String>,
        tile_set: TileSetDocument,
    ) -> Result<Option<TileSetDocument>, TileSetError> {
        tile_set.validate()?;
        Ok(self.bound.insert(reference.into(), tile_set))
    }

    pub fn unbind(&mut self, reference: &str) -> Option<TileSetDocument> {
        self.bound.remove(reference)
    }

    #[must_use]
    pub fn get(&self, reference: &str) -> Option<&TileSetDocument> {
        self.bound.get(reference)
    }
}

/// Every tile-set asset named by an active world.
pub fn referenced_tile_sets(world: &World) -> BTreeSet<String> {
    world
        .entities()
        .filter(|(entity, _)| world.is_active(*entity))
        .filter_map(|(_, data)| data.components.get(TileVolumeComponent::TYPE_NAME))
        .filter_map(|payload| payload.get("tileset"))
        .filter_map(serde_json::Value::as_str)
        .filter(|reference| !reference.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use sindri_core::{SCENE_FORMAT_VERSION, SceneDocument, World};

    use super::*;

    #[test]
    fn gathers_tile_sets_without_calling_them_textures() {
        let json = r#"{
              "format_version": $VERSION,
              "entities": [{ "id": "world", "components": {
                "sindri.tile_volume": {
                  "tileset": "tiles/world.tileset.json", "cells": []
                }
              } }]
            }"#
        .replace("$VERSION", &SCENE_FORMAT_VERSION.to_string());
        let scene = SceneDocument::from_json(&json).unwrap();
        let world = World::from_scene(&scene).unwrap().world;
        assert_eq!(
            referenced_tile_sets(&world),
            BTreeSet::from(["tiles/world.tileset.json".to_owned()])
        );
    }
}
