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

/// One more look a tile may have, and how often it should appear.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TileVariant {
    pub faces: TileFaces,
    /// How often relative to the tile's own faces and its other variants.
    ///
    /// Four plain patches to one with a flower is a tile of weight four and a
    /// variant of weight one. Zero is a look that is defined and never chosen,
    /// which is a useful thing to be able to say while working.
    #[serde(default = "one_weight", skip_serializing_if = "is_one")]
    pub weight: u32,
}

/// What one stable tile ID means.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TileDefinition {
    pub faces: TileFaces,
    /// How much of its cell this tile fills, from its floor upward.
    ///
    /// One is the whole cell and the default, so every tile written before
    /// heights existed keeps meaning exactly what it did. A half fills the
    /// bottom half: a slab. The value decides what this tile hides, how high
    /// something standing on it stands, and nothing about the art — a tile's
    /// faces are baked at its own height, the way a slab in a voxel game is a
    /// different block rather than a squashed one.
    ///
    /// Bounded at one because a taller tile would reach into the cell above,
    /// and every part of the system that asks what occupies a cell — occlusion,
    /// collision, the surface a walker walks on — assumes a cell's contents
    /// stay inside it. Something two blocks tall is two cells.
    #[serde(default = "full_height", skip_serializing_if = "is_full_height")]
    pub height: f32,
    /// Whether this tile hides a neighbouring tile's shared face.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub occludes: bool,
    /// Whether this tile's top holds anything up.
    ///
    /// Water supports and is not walkable, which is the distinction one flag
    /// could not make. A pond and a hole are not the same place: a boat, a
    /// pier or a lily floats on one and falls through the other, and before
    /// this a pond had no surface at all, so three rocks standing off Gather's
    /// shore had to be given a sandbar to stand on.
    ///
    /// False is decoration: something drawn in a cell that nothing rests on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub supports: bool,
    /// Other looks this tile may have, chosen by where the cell is.
    ///
    /// One tile ID with several looks, rather than several IDs that mean the
    /// same thing. A scene then stores what a cell *is* and the renderer
    /// decides what it looks like, which is the difference between a map that
    /// says "grass" five hundred times and one where an author hand-scattered
    /// five kinds of grass and has to keep doing it.
    ///
    /// Chosen per cell rather than per face, because a block whose top came
    /// from one look and whose side came from another is not a block.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<TileVariant>,
    /// How often this tile's own faces are chosen against its variants.
    #[serde(default = "one_weight", skip_serializing_if = "is_one")]
    pub weight: u32,
    /// Whether a walker can stand on that top.
    ///
    /// The narrower question, and the one navigation asks. Implies `supports`,
    /// because standing on something is resting on it; a tile that is walkable
    /// and unsupporting is rejected rather than guessed at.
    ///
    /// `solid` was the old name for this and still reads, because renaming a
    /// field is not the same as changing what a document meant.
    #[serde(default = "yes", alias = "solid", skip_serializing_if = "is_true")]
    pub walkable: bool,
}

impl TileDefinition {
    /// The faces to draw for a cell at this coordinate.
    ///
    /// The tile's own faces when it has no variants, which is every tile
    /// written before variants existed. Otherwise a deterministic choice from
    /// where the cell is, the tile's name and the volume's seed -- so the same
    /// cell looks the same on every machine and after every reload, and two
    /// different tiles in one cell do not vary in lockstep.
    ///
    /// Falls back to its own faces when every weight is zero. Something has to
    /// be drawn, and refusing here would leave a hole in the map over a
    /// tile-set mistake.
    #[must_use]
    pub fn faces_at(&self, seed: u64, coord: [i32; 3], tile: &str) -> &TileFaces {
        if self.variants.is_empty() {
            return &self.faces;
        }
        let mut weights = Vec::with_capacity(self.variants.len() + 1);
        weights.push(self.weight);
        weights.extend(self.variants.iter().map(|variant| variant.weight));
        #[allow(clippy::cast_possible_wrap)]
        let hash = crate::stable_hash_with(
            &[
                i64::from(coord[0]),
                i64::from(coord[1]),
                i64::from(coord[2]),
                seed as i64,
            ],
            tile,
        );
        match crate::weighted_index(hash, &weights) {
            Some(0) | None => &self.faces,
            Some(index) => &self.variants[index - 1].faces,
        }
    }

    /// Every face set this tile may draw, its own first.
    ///
    /// What a texture or sheet scan wants: a variant's sprites have to be
    /// loaded whether or not this particular scene happens to choose them.
    pub fn all_faces(&self) -> impl Iterator<Item = &TileFaces> {
        std::iter::once(&self.faces).chain(self.variants.iter().map(|variant| &variant.faces))
    }

    /// Whether this tile fills its cell all the way to the top.
    ///
    /// The question occlusion asks most often, and asking it here keeps the
    /// float comparison in one place rather than in every caller.
    #[must_use]
    pub fn fills_cell(&self) -> bool {
        self.height >= 1.0
    }

    /// Whether this tile hides a face of something `height` tall beside it.
    ///
    /// A neighbour hides a side face only when it is at least as tall as the
    /// face it would cover. A full block beside a slab hides the slab's side;
    /// a slab beside a full block leaves the block's upper half showing, which
    /// is the whole reason a slab reads as a slab.
    #[must_use]
    pub fn hides_side_of(&self, height: f32) -> bool {
        self.occludes && self.height >= height
    }
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

            if !definition.height.is_finite() || definition.height <= 0.0 || definition.height > 1.0
            {
                return Err(TileSetError::InvalidHeight {
                    tile: tile.clone(),
                    height: definition.height,
                });
            }
            if definition.walkable && !definition.supports {
                return Err(TileSetError::WalkableWithoutSupport(tile.clone()));
            }
            // Every look, not just the first: a variant with a broken sprite
            // is a hole that appears in whichever cells happen to choose it,
            // which is the worst kind of thing to find later.
            for faces in definition.all_faces() {
                if faces.iter().next().is_none() {
                    return Err(TileSetError::TileWithoutFaces(tile.clone()));
                }
                for (face, visual) in faces.iter() {
                    validate_visual(tile, face, visual)?;
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn tile(&self, id: &str) -> Option<&TileDefinition> {
        self.tiles.get(id)
    }
}

// `PartialEq` without `Eq`: an invalid height is reported as the number that
// was written, and a float has no total equality to offer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TileSetError {
    #[error("tile set JSON is not valid: {message}")]
    Json { message: String },
    #[error("tile set format version {found} is not supported; expected {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("a tile set must define at least one tile")]
    Empty,
    #[error("a tile ID cannot be empty")]
    EmptyTileId,
    #[error("tile `{0}` is walkable but supports nothing; standing on a tile rests on it")]
    WalkableWithoutSupport(String),
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
    #[error("tile `{tile}` has height {height}, which must be above zero and at most one cell")]
    InvalidHeight { tile: String, height: f32 },
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
    if !visual
        .size
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    {
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

const fn one_weight() -> u32 {
    1
}

// By reference because `skip_serializing_if` hands one over, the same reason
// `is_true` beside it takes one.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_one(weight: &u32) -> bool {
    *weight == 1
}

const fn full_height() -> f32 {
    1.0
}

/// Whether to leave `height` out of the serialized form.
///
/// Compared on the bits rather than within a margin: the question is whether
/// this is the default that can be omitted and read back identically, and a
/// height a hair under one is a real height that has to be written down.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_full_height(value: &f32) -> bool {
    value.to_bits() == full_height().to_bits()
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
        assert!(grass.supports);
        assert!(grass.walkable);
        assert!(grass.occludes);
        assert!(grass.faces.get(TileFace::Top).is_some());
    }

    /// `solid` was the old name for `walkable`, and a document that used it
    /// still means what it meant.
    #[test]
    fn the_old_solid_field_still_reads_as_walkable() {
        let set = TileSetDocument::from_json(
            r#"{
              "format_version": 1,
              "tiles": {
                "water": {
                  "solid": false,
                  "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 0.5] } }
                }
              }
            }"#,
        )
        .unwrap();
        let water = set.tile("water").unwrap();
        assert!(!water.walkable, "solid: false meant you cannot stand there");
        assert!(
            water.supports,
            "and said nothing about whether anything rests on it, so the \
             pond holds a boat up rather than being a hole"
        );
    }

    /// Standing on something rests on it, so the pair cannot disagree.
    #[test]
    fn a_walkable_tile_that_supports_nothing_is_rejected() {
        let error = TileSetDocument::from_json(
            r#"{
              "format_version": 1,
              "tiles": {
                "ghost": {
                  "supports": false,
                  "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 0.5] } }
                }
              }
            }"#,
        )
        .unwrap_err();
        assert!(
            matches!(error, TileSetError::WalkableWithoutSupport(ref tile) if tile == "ghost"),
            "{error:?}"
        );
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
