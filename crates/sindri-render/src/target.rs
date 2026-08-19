//! Colour and depth targets that are sized, formatted, and rebuilt together.

use crate::{COLOR_TARGET_FORMAT, ClearOperations, DepthTarget};

/// Clears a frame's colour and depth before any pass draws into them.
///
/// Clearing belongs to the frame rather than to whichever renderer happens to
/// draw first. When it belonged to the opaque mesh pass, a second mesh erased
/// the first, and a scene with no mesh at all cleared nothing: its sprites drew
/// over whatever the previous frame left, against a depth buffer no one had
/// filled. A scene of only sprites is what a 2D game is, so that case is the
/// normal one rather than the exception.
///
/// Every pass afterwards loads what this left.
pub fn encode_clear(
    encoder: &mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &DepthTarget,
    clear: ClearOperations,
) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Sindri frame clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: clear.color[0],
                    g: clear.color[1],
                    b: clear.color[2],
                    a: clear.color[3],
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth.view(),
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear.depth),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

/// The format a sampler must read a Sindri colour target through.
///
/// Colour targets are sRGB so the hardware encodes on write, which is half of
/// the [colour round trip](../../../docs/rendering-color.md). The other half is
/// whoever reads the result. A sampler that expects gamma-encoded bytes — egui's
/// does, and says so in its shader — would have the hardware decode them on
/// read, and two decodes against one encode renders perfectly while being the
/// wrong colour: authored orange arrives as `(221, 43, 6)` instead of
/// `(240, 114, 43)`.
///
/// Reading through the linear view of the same bytes converts nothing twice.
#[must_use]
pub fn sampled_format(target: wgpu::TextureFormat) -> wgpu::TextureFormat {
    target.remove_srgb_suffix()
}

/// A colour target a frame draws into and something else samples afterwards,
/// with the depth buffer that belongs to it.
///
/// Colour and depth are one thing here because they are only ever correct
/// together: a resize that rebuilt one and not the other would render against a
/// depth buffer of the wrong size, which is a validation error at best and a
/// wrong picture at worst.
#[derive(Debug)]
pub struct ViewportTarget {
    label: String,
    color: wgpu::Texture,
    attachment: wgpu::TextureView,
    sampled: wgpu::TextureView,
    depth: DepthTarget,
    width: u32,
    height: u32,
}

impl ViewportTarget {
    /// The format the colour texture is stored in.
    pub const FORMAT: wgpu::TextureFormat = COLOR_TARGET_FORMAT;

    pub fn new(device: &wgpu::Device, label: impl Into<String>, width: u32, height: u32) -> Self {
        let label = label.into();
        let width = width.max(1);
        let height = height.max(1);
        let (color, attachment, sampled) = create_color(device, &label, width, height);
        Self {
            label,
            color,
            attachment,
            sampled,
            depth: DepthTarget::new(device, width, height),
            width,
            height,
        }
    }

    /// Rebuilds at a new size, reporting whether anything changed.
    ///
    /// Callers that registered the sampled view elsewhere — egui holds one —
    /// use the answer to know when that registration needs updating.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return false;
        }
        let (color, attachment, sampled) = create_color(device, &self.label, width, height);
        self.color = color;
        self.attachment = attachment;
        self.sampled = sampled;
        self.depth.resize(device, width, height);
        self.width = width;
        self.height = height;
        true
    }

    /// The view a render pass draws into, in the target's own sRGB format.
    pub const fn attachment(&self) -> &wgpu::TextureView {
        &self.attachment
    }

    /// The view a sampler reads, in the linear format. See [`sampled_format`].
    pub const fn sampled(&self) -> &wgpu::TextureView {
        &self.sampled
    }

    pub const fn depth(&self) -> &DepthTarget {
        &self.depth
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }
}

fn create_color(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::TextureView) {
    let sampled_format = sampled_format(ViewportTarget::FORMAT);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: ViewportTarget::FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        // Both formats have to be declared for the two views to be legal.
        view_formats: &[ViewportTarget::FORMAT, sampled_format],
    });
    let attachment = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(label),
        format: Some(ViewportTarget::FORMAT),
        ..Default::default()
    });
    let sampled = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(label),
        format: Some(sampled_format),
        ..Default::default()
    });
    (texture, attachment, sampled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_drawn_into_through_srgb_and_read_through_linear() {
        assert!(
            ViewportTarget::FORMAT.is_srgb(),
            "a linear colour target silently darkens every rendered colour"
        );
        assert!(
            !sampled_format(ViewportTarget::FORMAT).is_srgb(),
            "sampling through an sRGB view decodes bytes the shader expects encoded"
        );
    }

    #[test]
    fn the_two_views_describe_the_same_bytes() {
        // Same texture, two readings of it. If these were different formats
        // rather than one format's two colour spaces, the views would be
        // describing different data and neither half would be right.
        assert_eq!(
            sampled_format(ViewportTarget::FORMAT).add_srgb_suffix(),
            ViewportTarget::FORMAT
        );
    }

    #[test]
    fn a_format_with_no_srgb_variant_is_left_alone() {
        // Nothing in Sindri uses one today, but returning it unchanged is what
        // keeps this a rule about colour space rather than a format rewrite.
        assert_eq!(
            sampled_format(wgpu::TextureFormat::Rgba16Float),
            wgpu::TextureFormat::Rgba16Float
        );
    }
}
