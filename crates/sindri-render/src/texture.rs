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
///
/// A slot and a generation, the same shape as an `EntityId`, and for the same
/// reason. A registry that reuses the slot of a released texture would otherwise
/// let an old handle draw whatever landed there next, which is worse than
/// drawing nothing: a stale binding would show a real texture, plausibly, and
/// the wrong one. The generation makes a released handle resolve to the missing
/// checker, which is the answer this type has always promised.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextureId {
    index: u32,
    generation: u32,
}

impl TextureId {
    /// Creates a handle for the first use of `index`.
    ///
    /// Handles normally come from [`TextureRegistry::insert`]. Minting one
    /// directly is safe because a registry draws a slot or generation it does
    /// not know as the missing texture, which is what makes binding testable
    /// without a GPU.
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            generation: 0,
        }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug)]
struct TextureSlot {
    generation: u32,
    texture: Option<Texture2D>,
}

/// The textures a frame may draw with.
///
/// Every registry has a fallback at [`TextureRegistry::MISSING`], so a draw
/// whose texture failed to load still renders something obviously wrong rather
/// than failing the frame or silently drawing the previous texture.
///
/// Slots are reused as textures are released, because a registry that only grew
/// would hold every texture a session ever loaded — which stopped being
/// theoretical the moment hot reload made replacing one a keystroke.
#[derive(Debug)]
pub struct TextureRegistry {
    slots: Vec<TextureSlot>,
    free: Vec<u32>,
}

impl TextureRegistry {
    /// The magenta-and-black texture drawn in place of a missing one.
    ///
    /// Slot zero, and it is never released, so this handle is valid for the
    /// life of the registry.
    pub const MISSING: TextureId = TextureId {
        index: 0,
        generation: 0,
    };

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
            slots: vec![TextureSlot {
                generation: 0,
                texture: Some(missing),
            }],
            free: Vec::new(),
        }
    }

    /// Adds a texture and returns the handle that draws it.
    ///
    /// Reuses the slot of a released texture when there is one. The slot's
    /// generation was already moved on by the release, so the handle returned
    /// here can never be mistaken for the one that used to live there.
    pub fn insert(&mut self, texture: Texture2D) -> TextureId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.texture = Some(texture);
            return TextureId {
                index,
                generation: slot.generation,
            };
        }
        let index = u32::try_from(self.slots.len()).expect("texture count exceeded u32::MAX");
        self.slots.push(TextureSlot {
            generation: 0,
            texture: Some(texture),
        });
        TextureId {
            index,
            generation: 0,
        }
    }

    /// Releases a texture, freeing it on the GPU, and reports whether it held
    /// one.
    ///
    /// Every handle to it becomes stale immediately — the slot's generation
    /// moves on whether or not anything is put back in it — so a binding nobody
    /// updated draws the missing checker rather than the next texture to occupy
    /// the slot.
    ///
    /// The fallback is not releasable. It is what every stale handle resolves
    /// to, so releasing it would leave a registry with nothing to answer with.
    pub fn remove(&mut self, id: TextureId) -> bool {
        if id == Self::MISSING {
            return false;
        }
        let Some(slot) = self.slot_mut(id) else {
            return false;
        };
        if slot.texture.take().is_none() {
            return false;
        }
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index());
        true
    }

    /// Returns the texture for `id`, falling back to the missing texture.
    ///
    /// A stale, released, or foreign handle draws as missing rather than
    /// panicking, so one bad reference cannot take down a frame.
    pub fn get(&self, id: TextureId) -> &Texture2D {
        self.slot(id)
            .and_then(|slot| slot.texture.as_ref())
            .unwrap_or_else(|| {
                self.slots[0]
                    .texture
                    .as_ref()
                    .expect("the fallback texture is never released")
            })
    }

    /// How many textures the registry is holding, fallback included.
    pub fn len(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.texture.is_some())
            .count()
    }

    pub fn is_empty(&self) -> bool {
        // The fallback always occupies slot zero.
        false
    }

    /// Every handle the registry currently answers for.
    pub fn ids(&self) -> impl Iterator<Item = TextureId> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.texture.is_some())
            .map(|(index, slot)| TextureId {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                generation: slot.generation,
            })
    }

    fn slot(&self, id: TextureId) -> Option<&TextureSlot> {
        let slot = self.slots.get(id.index() as usize)?;
        (slot.generation == id.generation()).then_some(slot)
    }

    fn slot_mut(&mut self, id: TextureId) -> Option<&mut TextureSlot> {
        let slot = self.slots.get_mut(id.index() as usize)?;
        (slot.generation == id.generation()).then_some(slot)
    }
}
