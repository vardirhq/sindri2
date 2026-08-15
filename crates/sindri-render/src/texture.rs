use thiserror::Error;
use wgpu::util::DeviceExt;

#[derive(Debug)]
pub struct Texture2D {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl Texture2D {
    pub fn from_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<Self, TextureError> {
        let expected = rgba_byte_len(width, height)?;
        if pixels.len() != expected {
            return Err(TextureError::IncorrectByteLength {
                expected,
                actual: pixels.len(),
            });
        }

        // Source pixels are authored in sRGB; see `crate::COLOR_TARGET_FORMAT`.
        let format = crate::COLOR_TARGET_FORMAT;
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sindri texture sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
            width,
            height,
            format,
        })
    }

    pub fn checkerboard(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        size: u32,
        cells: u32,
        colors: [[u8; 4]; 2],
    ) -> Result<Self, TextureError> {
        if cells == 0 {
            return Err(TextureError::InvalidDimensions);
        }
        let capacity = rgba_byte_len(size, size)?;
        let mut pixels = Vec::with_capacity(capacity);
        let cell_size = (size / cells).max(1);
        for y in 0..size {
            for x in 0..size {
                let index = usize::from(!(x / cell_size + y / cell_size).is_multiple_of(2));
                pixels.extend_from_slice(&colors[index]);
            }
        }
        Self::from_rgba8(device, queue, label, size, size, &pixels)
    }

    pub const fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub const fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub const fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

fn rgba_byte_len(width: u32, height: u32) -> Result<usize, TextureError> {
    if width == 0 || height == 0 {
        return Err(TextureError::InvalidDimensions);
    }
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(TextureError::DimensionsOverflow)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TextureError {
    #[error("texture width and height must both be non-zero")]
    InvalidDimensions,
    #[error("texture dimensions overflow the supported byte count")]
    DimensionsOverflow,
    #[error("RGBA texture data has {actual} bytes; expected {expected}")]
    IncorrectByteLength { expected: usize, actual: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_length_is_four_bytes_per_pixel() {
        assert_eq!(rgba_byte_len(3, 2), Ok(24));
    }

    #[test]
    fn rejects_zero_and_overflowing_dimensions() {
        assert_eq!(rgba_byte_len(0, 4), Err(TextureError::InvalidDimensions));
        assert_eq!(
            rgba_byte_len(u32::MAX, u32::MAX),
            Err(TextureError::DimensionsOverflow)
        );
    }
}

/// A renderer-level handle to an uploaded texture.
///
/// Deliberately not an asset ID: `sindri-render` knows nothing about assets or
/// scenes, so the layer that owns both maps one to the other.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextureId(u32);

impl TextureId {
    /// Creates a handle for `index`.
    ///
    /// Handles normally come from [`TextureRegistry::insert`]. Minting one
    /// directly is safe because a registry draws an index it does not know as
    /// the missing texture, which is what makes binding testable without a GPU.
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// The textures a frame may draw with.
///
/// Every registry has a fallback at [`TextureRegistry::MISSING`], so a draw
/// whose texture failed to load still renders something obviously wrong rather
/// than failing the frame or silently drawing the previous texture.
#[derive(Debug)]
pub struct TextureRegistry {
    textures: Vec<Texture2D>,
}

impl TextureRegistry {
    /// The magenta-and-black texture drawn in place of a missing one.
    pub const MISSING: TextureId = TextureId(0);

    /// Creates a registry containing only the missing-texture fallback.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let missing = Texture2D::checkerboard(
            device,
            queue,
            "Sindri missing texture",
            64,
            8,
            [[255, 0, 255, 255], [26, 26, 26, 255]],
        )
        .expect("the fallback texture has valid dimensions");
        Self {
            textures: vec![missing],
        }
    }

    /// Adds a texture and returns the handle that draws it.
    pub fn insert(&mut self, texture: Texture2D) -> TextureId {
        let index = u32::try_from(self.textures.len()).expect("texture count exceeded u32::MAX");
        self.textures.push(texture);
        TextureId(index)
    }

    /// Returns the texture for `id`, falling back to the missing texture.
    ///
    /// A stale or foreign handle draws as missing rather than panicking, so one
    /// bad reference cannot take down a frame.
    pub fn get(&self, id: TextureId) -> &Texture2D {
        self.textures
            .get(id.index() as usize)
            .unwrap_or_else(|| &self.textures[0])
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        // The fallback always occupies slot zero.
        false
    }

    pub fn ids(&self) -> impl Iterator<Item = TextureId> {
        (0..u32::try_from(self.textures.len()).unwrap_or(u32::MAX)).map(TextureId)
    }
}
