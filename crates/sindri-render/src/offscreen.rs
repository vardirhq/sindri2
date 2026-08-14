use std::sync::mpsc;

use thiserror::Error;

const RGBA8_BYTES_PER_PIXEL: u32 = 4;
const COPY_ROW_ALIGNMENT: u32 = 256;

#[derive(Debug)]
pub struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
}

impl OffscreenTarget {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Result<Self, OffscreenError> {
        let unpadded_bytes_per_row = rgba_bytes_per_row(width)?;
        if height == 0 {
            return Err(OffscreenError::InvalidDimensions);
        }
        let padded_bytes_per_row = align_to(unpadded_bytes_per_row, COPY_ROW_ALIGNMENT)
            .ok_or(OffscreenError::DimensionsOverflow)?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sindri offscreen color target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[Self::FORMAT],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self {
            texture,
            view,
            width,
            height,
            padded_bytes_per_row,
        })
    }

    pub fn copy_to_buffer(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<OffscreenReadback, OffscreenError> {
        let buffer_size = u64::from(self.padded_bytes_per_row)
            .checked_mul(u64::from(self.height))
            .ok_or(OffscreenError::DimensionsOverflow)?;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sindri offscreen readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(OffscreenReadback {
            buffer,
            width: self.width,
            height: self.height,
            padded_bytes_per_row: self.padded_bytes_per_row,
        })
    }

    pub const fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }
}

#[derive(Debug)]
pub struct OffscreenReadback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
}

impl OffscreenReadback {
    pub fn read_rgba8(self, device: &wgpu::Device) -> Result<Vec<u8>, OffscreenError> {
        let slice = self.buffer.slice(..);
        let (sender, receiver) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| OffscreenError::DevicePoll(error.to_string()))?;
        receiver
            .recv()
            .map_err(|_| OffscreenError::MappingCallbackDisconnected)??;

        let mapped = slice
            .get_mapped_range()
            .map_err(|error| OffscreenError::BufferAccess(error.to_string()))?;
        let unpadded_bytes_per_row = usize::try_from(rgba_bytes_per_row(self.width)?)
            .map_err(|_| OffscreenError::DimensionsOverflow)?;
        let padded_bytes_per_row = usize::try_from(self.padded_bytes_per_row)
            .map_err(|_| OffscreenError::DimensionsOverflow)?;
        let capacity = unpadded_bytes_per_row
            .checked_mul(
                usize::try_from(self.height).map_err(|_| OffscreenError::DimensionsOverflow)?,
            )
            .ok_or(OffscreenError::DimensionsOverflow)?;
        let mut pixels = Vec::with_capacity(capacity);
        for row in mapped
            .chunks_exact(padded_bytes_per_row)
            .take(usize::try_from(self.height).map_err(|_| OffscreenError::DimensionsOverflow)?)
        {
            pixels.extend_from_slice(&row[..unpadded_bytes_per_row]);
        }
        drop(mapped);
        self.buffer.unmap();
        Ok(pixels)
    }
}

fn rgba_bytes_per_row(width: u32) -> Result<u32, OffscreenError> {
    if width == 0 {
        return Err(OffscreenError::InvalidDimensions);
    }
    width
        .checked_mul(RGBA8_BYTES_PER_PIXEL)
        .ok_or(OffscreenError::DimensionsOverflow)
}

fn align_to(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded / alignment * alignment)
}

#[derive(Debug, Error)]
pub enum OffscreenError {
    #[error("offscreen width and height must both be non-zero")]
    InvalidDimensions,
    #[error("offscreen dimensions overflow the supported byte count")]
    DimensionsOverflow,
    #[error("GPU buffer mapping failed: {0}")]
    BufferMap(#[from] wgpu::BufferAsyncError),
    #[error("GPU device polling failed: {0}")]
    DevicePoll(String),
    #[error("GPU readback buffer access failed: {0}")]
    BufferAccess(String),
    #[error("GPU mapping callback disconnected")]
    MappingCallbackDisconnected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_pitch_is_aligned_for_gpu_copies() {
        assert_eq!(align_to(4, COPY_ROW_ALIGNMENT), Some(256));
        assert_eq!(align_to(256, COPY_ROW_ALIGNMENT), Some(256));
        assert_eq!(align_to(260, COPY_ROW_ALIGNMENT), Some(512));
    }

    #[test]
    fn row_pitch_rejects_zero_and_overflow() {
        assert!(matches!(
            rgba_bytes_per_row(0),
            Err(OffscreenError::InvalidDimensions)
        ));
        assert!(matches!(
            rgba_bytes_per_row(u32::MAX),
            Err(OffscreenError::DimensionsOverflow)
        ));
    }
}
