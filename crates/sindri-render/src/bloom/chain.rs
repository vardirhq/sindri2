//! The targets bloom runs through, and one step of the run.
//!
//! Three textures: the scene at full size, and a ping-pong pair at a quarter of
//! it. Every step reads one and writes another, which is why they are a pair —
//! a blur cannot read and write the same texture, and a sweep that tried would
//! be reading texels it had already changed.

use super::{BloomSettings, Pipelines};

/// One texture and the view of it.
struct Target {
    view: wgpu::TextureView,
}

impl Target {
    fn new(
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
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
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[format],
        });
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        }
    }
}

pub(super) struct Chain {
    scene: Target,
    /// Written by the bright pass, then swapped with `pong` on every sweep.
    ping: Target,
    pong: Target,
    width: u32,
    height: u32,
    /// One texel of the chain's own targets, in UV.
    texel: [f32; 2],
    /// Which of the pair holds the current picture. `Cell` because a run is a
    /// sequence of steps over a `&self` chain, and the swap is the only thing
    /// about it that changes.
    current: std::cell::Cell<bool>,
    uniform: wgpu::Buffer,
}

impl Chain {
    /// The format the blur runs in.
    ///
    /// Float and linear, unlike every other colour target here. The chain is not
    /// a picture anyone looks at: it is an intermediate that is written and read
    /// five or ten times in a frame, and an sRGB one would encode and decode at
    /// every step — losing the darks that a glow is mostly made of — and clamp
    /// away anything brighter than white before it had a chance to spread.
    pub(super) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub(super) fn new(device: &wgpu::Device, width: u32, height: u32, downscale: u32) -> Self {
        // At least one texel each way, however small the viewport: a texture of
        // no width is an error rather than an empty picture.
        let small = (width / downscale).max(1);
        let tall = (height / downscale).max(1);
        Self {
            scene: Target::new(
                device,
                "Sindri bloom scene",
                width,
                height,
                super::Bloom::SCENE_FORMAT,
            ),
            ping: Target::new(device, "Sindri bloom ping", small, tall, Self::FORMAT),
            pong: Target::new(device, "Sindri bloom pong", small, tall, Self::FORMAT),
            width,
            height,
            #[allow(clippy::cast_precision_loss)]
            texel: [1.0 / small as f32, 1.0 / tall as f32],
            current: std::cell::Cell::new(false),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Sindri bloom uniform"),
                size: std::mem::size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }

    pub(super) const fn matches(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }

    pub(super) const fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene.view
    }

    /// The chain target holding the current picture, and the one to write next.
    fn pair(&self) -> (&wgpu::TextureView, &wgpu::TextureView) {
        if self.current.get() {
            (&self.pong.view, &self.ping.view)
        } else {
            (&self.ping.view, &self.pong.view)
        }
    }

    /// Runs one step of the chain.
    pub(super) fn run(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &Pipelines,
        sampler: &wgpu::Sampler,
        step: Step<'_>,
    ) {
        let (read, write) = self.pair();
        let (pipeline, source, target, params) = match step {
            // The scene at full size into the chain at a quarter of it: a
            // filtered read at that size *is* the downsample, so the threshold
            // and the shrink are one draw.
            Step::Bright(settings) => (
                &pipelines.bright,
                &self.scene.view,
                write,
                Params {
                    source: [
                        self.texel[0],
                        self.texel[1],
                        settings.threshold,
                        settings.knee,
                    ],
                    blur: [0.0; 4],
                },
            ),
            Step::Blur { direction } => (
                &pipelines.blur,
                read,
                write,
                Params {
                    source: [self.texel[0], self.texel[1], 0.0, 0.0],
                    blur: [direction[0], direction[1], 0.0, 0.0],
                },
            ),
            Step::Composite { target, intensity } => (
                &pipelines.composite,
                &self.scene.view,
                target,
                Params {
                    source: [self.texel[0], self.texel[1], 0.0, 0.0],
                    blur: [0.0, 0.0, intensity, 0.0],
                },
            ),
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));

        let sample = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri bloom sample"),
            layout: &pipelines.sample_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        // Only the composite reads two pictures, and it reads the one the last
        // sweep wrote.
        let glow = matches!(step, Step::Composite { .. }).then(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Sindri bloom glow"),
                layout: &pipelines.glow_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(read),
                }],
            })
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sindri bloom pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Every one of these writes every pixel of its target,
                        // so there is nothing to preserve and clearing is free.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &sample, &[]);
            if let Some(glow) = glow.as_ref() {
                pass.set_bind_group(1, glow, &[]);
            }
            pass.draw(0..3, 0..1);
        }
        // The composite reads the chain and writes somewhere else entirely, so
        // it leaves the pair where it found it.
        if !matches!(step, Step::Composite { .. }) {
            self.current.set(!self.current.get());
        }
    }
}

/// One step of a bloom run.
#[derive(Clone, Copy)]
pub(super) enum Step<'a> {
    /// Keep what is bright enough, at a quarter size.
    Bright(BloomSettings),
    /// Spread it, one axis at a time.
    Blur { direction: [f32; 2] },
    /// Add it back over the scene, into somewhere that is not the chain.
    Composite {
        target: &'a wgpu::TextureView,
        intensity: f32,
    },
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    source: [f32; 4],
    blur: [f32; 4],
}
