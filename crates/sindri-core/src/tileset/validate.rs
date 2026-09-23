//! Checking one face's visual: its sprite, its size and offset, and its
//! animation, each refused with the face it belongs to.

use crate::SpriteRef;

use super::{TileFace, TileFaceVisual, TileSetError};

pub(super) fn validate_visual(
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
    if let Some(animation) = &visual.animation {
        let broken = animation
            .frames
            .iter()
            .find(|frame| SpriteRef::parse(frame).is_err());
        if let Some(frame) = broken {
            return Err(TileSetError::InvalidSprite {
                tile: tile.to_owned(),
                face,
                sprite: frame.clone(),
            });
        }
        if animation.frames.is_empty() || !(animation.fps.is_finite() && animation.fps > 0.0) {
            return Err(TileSetError::InvalidAnimation {
                tile: tile.to_owned(),
                face,
            });
        }
    }
    if !visual.offset.iter().all(|value| value.is_finite()) {
        return Err(TileSetError::InvalidOffset {
            tile: tile.to_owned(),
            face,
        });
    }
    Ok(())
}
