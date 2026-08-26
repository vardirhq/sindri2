//! A named region of a sheet, and how one is written down.

use std::{fmt, hash::Hash};

use thiserror::Error;

use super::id::AssetId;

/// A texture, and optionally which named part of it to draw.
///
/// Written as `textures/tiles.png#floor`: the path before the `#`, the sprite's
/// name after it. Without a fragment it names the whole image.
///
/// `#` is a *rejected* character in [`AssetId`], and that is the argument for
/// using it here rather than against. It is reserved precisely so a fragment
/// cannot leak into a path that becomes a URL, so splitting it off at the
/// boundary — exactly as a URL does — leaves the asset ID a pure path and gives
/// the fragment somewhere to live. Nothing that resolves an asset ever sees it.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpriteRef {
    texture: String,
    sprite: Option<String>,
}

impl SpriteRef {
    /// Parses `textures/tiles.png#floor`, or a plain path for the whole image.
    pub fn parse(reference: &str) -> Result<Self, SpriteRefError> {
        let (path, sprite) = match reference.split_once('#') {
            Some((path, name)) => {
                if name.is_empty() {
                    return Err(SpriteRefError::EmptySprite);
                }
                if name.contains('#') {
                    return Err(SpriteRefError::SecondFragment);
                }
                (path, Some(name.to_owned()))
            }
            None => (reference, None),
        };
        if path.is_empty() {
            return Err(SpriteRefError::EmptyTexture);
        }
        Ok(Self {
            texture: path.to_owned(),
            sprite,
        })
    }

    /// The reference as a whole names its texture, which is what a host binds
    /// and what the renderer is asked for.
    ///
    /// A string rather than an [`AssetId`], because not every texture is a
    /// file: `procedural:checkerboard` is generated, and the colon that makes
    /// it un-parseable as an asset ID is exactly what marks it as generated.
    #[must_use]
    pub fn texture(&self) -> &str {
        &self.texture
    }

    /// The asset behind the texture, or `None` when nothing loads it.
    pub fn asset(&self) -> Option<AssetId> {
        AssetId::new(self.texture.clone()).ok()
    }

    /// Which part of the image, or `None` for all of it.
    #[must_use]
    pub fn sprite(&self) -> Option<&str> {
        self.sprite.as_deref()
    }

    /// The sheet this reference needs loaded, which is only ever the one its
    /// own fragment names a sprite in.
    ///
    /// A reference with no fragment needs no sheet, so an unsliced texture is
    /// not asked for a sidecar that does not exist. That is what keeps a
    /// missing sheet an error worth reporting rather than the ordinary case.
    #[must_use]
    pub fn sheet(&self) -> Option<AssetId> {
        self.sprite.as_ref()?;
        crate::sheet_id_for(&self.asset()?)
    }
}

impl fmt::Display for SpriteRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.sprite {
            Some(sprite) => write!(formatter, "{}#{sprite}", self.texture),
            None => formatter.write_str(self.texture.as_str()),
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SpriteRefError {
    #[error("a sprite reference must name a texture")]
    EmptyTexture,
    #[error("a sprite reference's `#` must be followed by a name")]
    EmptySprite,
    #[error("a sprite reference names one sprite, so it holds one `#`")]
    SecondFragment,
}
