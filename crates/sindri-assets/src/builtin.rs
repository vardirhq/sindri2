//! Assets the engine ships, so a project can use them without providing them.
//!
//! The first is a set of blocks: grass, dirt, stone, sand, water, lava and the
//! rest, each with its art. A voxel world that names `builtin:blocks` is built
//! from them, which is what makes a new world look like a world before anyone
//! has drawn a texture. A project that wants its own copies it into a
//! `.tileset.json` of its own and edits that.
//!
//! Built-in references start with `builtin:`. A colon cannot appear in an
//! `AssetId`, so no project file can be mistaken for one, and a loader asked
//! for one refuses it rather than looking on disk: a host binds these itself,
//! from the bytes compiled in here, exactly as it binds `procedural:` textures.

pub use sindri_core::BUILTIN_BLOCKS;
use sindri_core::{AssetId, SpriteSheetDocument, TileSetDocument};

use crate::{AssetBytes, AssetDecodeError, AssetDecoder, TextureAsset, TextureAssetDecoder};

/// One built-in texture: the reference a tile set names it by, its bytes, and
/// the sheet that cuts it into named sprites, if it is cut.
#[derive(Clone, Copy, Debug)]
pub struct BuiltinTexture {
    pub reference: &'static str,
    png: &'static [u8],
    sheet: Option<&'static str>,
}

/// Why a built-in asset would not decode. Only a broken build can cause one,
/// and the tests below are what stop that build shipping.
#[derive(Debug, thiserror::Error)]
pub enum BuiltinError {
    #[error("built-in texture `{reference}`: {source}")]
    Texture {
        reference: &'static str,
        source: AssetDecodeError,
    },
    #[error("built-in sheet for `{reference}`: {message}")]
    Sheet {
        reference: &'static str,
        message: String,
    },
    #[error("built-in tile set `{reference}`: {message}")]
    TileSet {
        reference: &'static str,
        message: String,
    },
}

macro_rules! block_art {
    ($file:literal) => {
        BuiltinTexture {
            reference: concat!("builtin:blocks/", $file),
            png: include_bytes!(concat!("../builtin/blocks/", $file)),
            sheet: None,
        }
    };
    ($file:literal, $sheet:literal) => {
        BuiltinTexture {
            reference: concat!("builtin:blocks/", $file),
            png: include_bytes!(concat!("../builtin/blocks/", $file)),
            sheet: Some(include_str!(concat!("../builtin/blocks/", $sheet))),
        }
    };
}

const TEXTURES: [BuiltinTexture; 10] = [
    block_art!("tops.png", "tops.sheet.json"),
    block_art!("sides.png", "sides.sheet.json"),
    block_art!("grass-top.png", "grass-top.sheet.json"),
    block_art!("grass-side.png"),
    block_art!("dirt.png"),
    block_art!("log-end.png"),
    block_art!("log-bark.png"),
    block_art!("leaves.png"),
    block_art!("lava.png", "lava.sheet.json"),
    block_art!("water.png", "water.sheet.json"),
];

const TILE_SETS: [(&str, &str); 1] = [(
    BUILTIN_BLOCKS,
    include_str!("../builtin/blocks/blocks.tileset.json"),
)];

/// Every texture the engine ships.
pub fn builtin_textures() -> impl Iterator<Item = BuiltinTexture> {
    TEXTURES.into_iter()
}

impl BuiltinTexture {
    /// The texture's pixels.
    pub fn decode(&self) -> Result<TextureAsset, BuiltinError> {
        // Only for the decoder's messages: `builtin/blocks/lava.png` is what
        // an error would call it, since the real reference is not an ID.
        let id = AssetId::new(self.reference.replace(':', "/"))
            .expect("a built-in reference without its colon is a valid asset ID");
        TextureAssetDecoder
            .decode(AssetBytes::new(id, self.png.to_vec()))
            .map_err(|source| BuiltinError::Texture {
                reference: self.reference,
                source,
            })
    }

    /// The named sprites this texture is cut into, if any.
    pub fn sheet(&self) -> Option<Result<SpriteSheetDocument, BuiltinError>> {
        self.sheet.map(|json| {
            SpriteSheetDocument::from_json(json).map_err(|error| BuiltinError::Sheet {
                reference: self.reference,
                message: error.to_string(),
            })
        })
    }
}

/// Every tile set the engine ships, by the reference a scene names it with.
pub fn builtin_tile_sets()
-> impl Iterator<Item = Result<(&'static str, TileSetDocument), BuiltinError>> {
    TILE_SETS.into_iter().map(|(reference, json)| {
        TileSetDocument::from_json(json)
            .map(|tile_set| (reference, tile_set))
            .map_err(|error| BuiltinError::TileSet {
                reference,
                message: error.to_string(),
            })
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sindri_core::{SpriteRef, TileFaces};

    use super::*;

    #[test]
    fn every_built_in_texture_and_sheet_decodes() {
        for texture in builtin_textures() {
            let decoded = texture.decode().expect("the texture decodes");
            assert!(decoded.width() > 0 && decoded.height() > 0);
            if let Some(sheet) = texture.sheet() {
                sheet.expect("the sheet parses");
            }
        }
    }

    /// A block naming art the engine does not ship would draw as the missing
    /// checker in every project that used it.
    #[test]
    fn every_built_in_block_names_art_the_engine_ships() {
        let shipped: BTreeSet<&str> = builtin_textures()
            .map(|texture| texture.reference)
            .collect();
        for tile_set in builtin_tile_sets() {
            let (reference, tile_set) = tile_set.expect("the tile set parses");
            tile_set.validate().expect("the tile set is valid");
            for (name, block) in &tile_set.tiles {
                for (_, visual) in block.all_faces().flat_map(TileFaces::iter) {
                    let frames = visual
                        .animation
                        .iter()
                        .flat_map(|animation| animation.frames.iter());
                    for named in std::iter::once(&visual.sprite).chain(frames) {
                        let sprite = SpriteRef::parse(named).expect("a sprite reference");
                        assert!(
                            shipped.contains(sprite.texture()),
                            "{reference} block `{name}` names {named}"
                        );
                    }
                }
            }
        }
    }
}
