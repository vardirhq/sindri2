//! Images: decoded to RGBA8, whatever the file said they were.

use sindri_core::AssetLoadErrorKind;

use crate::AssetBytes;

use super::{AssetDecodeError, AssetDecoder};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureAsset {
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

impl TextureAsset {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }

    pub fn into_rgba8(self) -> Vec<u8> {
        self.rgba8
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureAssetDecoder;

impl AssetDecoder for TextureAssetDecoder {
    type Asset = TextureAsset;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let image = image::load_from_memory(bytes.as_slice()).map_err(|error| {
            AssetDecodeError::new(
                id.clone(),
                "texture",
                image_error_kind(&error),
                error.to_string(),
            )
        })?;
        let image = image.into_rgba8();
        Ok(TextureAsset {
            width: image.width(),
            height: image.height(),
            rgba8: image.into_raw(),
        })
    }
}

fn image_error_kind(error: &image::ImageError) -> AssetLoadErrorKind {
    match error {
        image::ImageError::Unsupported(_) => AssetLoadErrorKind::UnsupportedFormat,
        image::ImageError::Decoding(_) | image::ImageError::Limits(_) => {
            AssetLoadErrorKind::InvalidData
        }
        image::ImageError::IoError(_) => AssetLoadErrorKind::Io,
        image::ImageError::Encoding(_) | image::ImageError::Parameter(_) => {
            AssetLoadErrorKind::Other
        }
    }
}
