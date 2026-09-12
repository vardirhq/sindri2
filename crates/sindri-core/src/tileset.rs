//! Reusable semantic tiles and the baked faces that represent them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::SpriteRef;

pub const TILESET_FORMAT_VERSION: u32 = 1;
pub const TILESET_SUFFIX: &str = ".tileset.json";

/// One face of an axis-aligned logical tile cell.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TileFace {
    Bottom,
    North,
    West,
    East,
    South,
    Top,
}

impl TileFace {
    /// The cell whose presence may hide this face.
    #[must_use]
    pub const fn neighbour_offset(self) -> [i32; 3] {
        match self {
            Self::Bottom => [0, 0, -1],
            Self::North => [0, -1, 0],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
            Self::South => [0, 1, 0],
            Self::Top => [0, 0, 1],
        }
    }
}

/// One pre-rendered 2D face.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TileFaceVisual {
    /// Texture or named sheet sprite, such as `blocks.png#grass_top`.
    pub sprite: String,
    /// Quad size in the tile grid's local world units.
    pub size: [f32; 2],
    /// Quad-centre offset from the projected cell centre.
    #[serde(default, skip_serializing_if = "is_zero_vec")]
    pub offset: [f32; 2],
}

/// Optional visuals for every face a view may expose.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TileFaces {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<TileFaceVisual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub north: Option<TileFaceVisual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub west: Option<TileFaceVisual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east: Option<TileFaceVisual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub south: Option<TileFaceVisual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<TileFaceVisual>,
}

impl TileFaces {
    pub fn iter(&self) -> impl Iterator<Item = (TileFace, &TileFaceVisual)> {
        [
            (TileFace::Bottom, self.bottom.as_ref()),
            (TileFace::North, self.north.as_ref()),
            (TileFace::West, self.west.as_ref()),
            (TileFace::East, self.east.as_ref()),
            (TileFace::South, self.south.as_ref()),
            (TileFace::Top, self.top.as_ref()),
        ]
        .into_iter()
        .filter_map(|(face, visual)| visual.map(|visual| (face, visual)))
    }

    #[must_use]
    pub fn get(&self, face: TileFace) -> Option<&TileFaceVisual> {
        match face {
            TileFace::Bottom => self.bottom.as_ref(),
            TileFace::North => self.north.as_ref(),
            TileFace::West => self.west.as_ref(),
            TileFace::East => self.east.as_ref(),
            TileFace::South => self.south.as_ref(),
            TileFace::Top => self.top.as_ref(),
        }
    }
}

/// What one stable tile ID means.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TileDefinition {
    pub faces: TileFaces,
    /// Whether this tile hides a neighbouring tile's shared face.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub occludes: bool,
    /// Whether collision generation treats the logical cell as solid.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub solid: bool,
}

/// A reusable project asset shared by scenes and tile volumes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TileSetDocument {
    pub format_version: u32,
    pub tiles: BTreeMap<String, TileDefinition>,
}

impl TileSetDocument {
    pub fn from_json(json: &str) -> Result<Self, TileSetError> {
        let document: Self = serde_json::from_str(json).map_err(|error| TileSetError::Json {
            message: error.to_string(),
        })?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), TileSetError> {
        if self.format_version != TILESET_FORMAT_VERSION {
            return Err(TileSetError::UnsupportedVersion {
                found: self.format_version,
                supported: TILESET_FORMAT_VERSION,
            });
        }
        if self.tiles.is_empty() {
            return Err(TileSetError::Empty);
        }
        for (tile, definition) in &self.tiles {
            if tile.trim().is_empty() {
                return Err(TileSetError::EmptyTileId);
            }
            if definition.faces.iter().next().is_none() {
                return Err(TileSetError::TileWithoutFaces(tile.clone()));
            }
            for (face, visual) in definition.faces.iter() {
                validate_visual(tile, face, visual)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn tile(&self, id: &str) -> Option<&TileDefinition> {
        self.tiles.get(id)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TileSetError {
    #[error("tile set JSON is not valid: {message}")]
    Json { message: String },
    #[error("tile set format version {found} is not supported; expected {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("a tile set must define at least one tile")]
    Empty,
    #[error("a tile ID cannot be empty")]
    EmptyTileId,
    #[error("tile `{0}` does not define any face visuals")]
    TileWithoutFaces(String),
    #[error("tile `{tile}` has an invalid {face:?} sprite reference `{sprite}`")]
    InvalidSprite {
        tile: String,
        face: TileFace,
        sprite: String,
    },
    #[error("tile `{tile}` has a non-finite or non-positive {face:?} visual size")]
    InvalidSize { tile: String, face: TileFace },
    #[error("tile `{tile}` has a non-finite {face:?} visual offset")]
    InvalidOffset { tile: String, face: TileFace },
}

fn validate_visual(
    tile: &str,
    face: TileFace,
    visual: &TileFaceVisual,
) -> Result<(), TileSetError> {
    if SpriteRef::parse(&visual.sprite).is_err() {
        return Err(TileSetError::InvalidSprite {
            tile: tile.to_owned(),
            face,
            sprite: visual.sprite.clone(),
        });
    }
    if !visual.size.iter().all(|value| value.is_finite() && *value > 0.0) {
        return Err(TileSetError::InvalidSize {
            tile: tile.to_owned(),
            face,
        });
    }
    if !visual.offset.iter().all(|value| value.is_finite()) {
        return Err(TileSetError::InvalidOffset {
            tile: tile.to_owned(),
            face,
        });
    }
    Ok(())
}

const fn yes() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_true(value: &bool) -> bool {
    *value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero_vec(value: &[f32; 2]) -> bool {
    value[0] == 0.0 && value[1] == 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_tile_set_keeps_semantics_separate_from_faces() {
        let set = TileSetDocument::from_json(
            r#"{
              "format_version": 1,
              "tiles": { "grass": {
                "faces": {
                  "top": { "sprite": "blocks.png#grass_top", "size": [1.0, 0.5] },
                  "south": { "sprite": "blocks.png#earth_south", "size": [1.0, 0.5], "offset": [0.0, -0.25] }
                }
              } }
            }"#,
        )
        .unwrap();
        let grass = set.tile("grass").unwrap();
        assert!(grass.solid);
        assert!(grass.occludes);
        assert!(grass.faces.get(TileFace::Top).is_some());
    }

    #[test]
    fn invalid_visual_geometry_is_rejected_at_asset_decode() {
        let error = TileSetDocument::from_json(
            r#"{
              "format_version": 1,
              "tiles": { "grass": { "faces": {
                "top": { "sprite": "blocks.png#grass", "size": [1.0, 0.0] }
              } } }
            }"#,
        )
        .unwrap_err();
        assert!(matches!(error, TileSetError::InvalidSize { .. }));
    }
}
