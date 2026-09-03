//! Bloom: the light a bright thing throws into the dark around it.
//!
//! The one effect that decides whether neon strokes on black read as light or as
//! clip art. Without it a bright ring is a coloured line; with it the ring is a
//! source and the black around it is lit, which is the whole difference between
//! a vector drawing and a game that looks like it is glowing.
//!
//! It is opt in and sits *outside* [`crate::encode_prepared_frame`] rather than
//! inside it, because it changes where a frame is drawn: the scene goes to a
//! target this owns, and this then writes the scene plus its glow to wherever
//! the host actually wanted it. A host that does not want bloom keeps drawing
//! straight to its surface and pays nothing.
//!
//! ```no_run
//! # use sindri_render::{Bloom, BloomSettings};
//! # fn wrap(device: &wgpu::Device, queue: &wgpu::Queue,
//! #         encoder: &mut wgpu::CommandEncoder, surface: &wgpu::TextureView) {
//! let mut bloom = Bloom::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
//! bloom.resize(device, 1920, 1080);
//! // Draw the frame into `bloom.scene_view()` instead of the surface, then:
//! bloom.resolve(device, queue, encoder, surface, BloomSettings::default());
//! # }
//! ```

mod chain;

use std::borrow::Cow;

use chain::Chain;

/// How much light, from how bright, and how far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BloomSettings {
    /// How bright a colour has to be before it glows at all, on the linear 0-1
    /// scale the shader works in.
    ///
    /// Below one, so ordinary bright colours glow. Neon on black is the case
    /// this exists for and none of it is over-bright: a mint stroke is about
    /// `0.9` at its brightest channel, so a threshold at or above one would
    /// leave the whole picture matte.
    pub threshold: f32,
    /// The band either side of the threshold over which a colour ramps into
    /// glowing, rather than starting to at a stroke.
    ///
    /// A hard cutoff makes the glow switch on and off as something pulses or
    /// fades across the threshold, which reads as a flicker.
    pub knee: f32,
    /// How much of the blurred light is added back.
    pub intensity: f32,
    /// How many times the blur is run. Each pass is a horizontal and a vertical
    /// sweep, and each roughly doubles how far the light reaches.
    ///
    /// Three is a glow around a thing. Six is a haze over the whole screen.
    pub passes: u32,
}

impl Default for BloomSettings {
    fn default() -> Self {
        // Tuned by shooting the shape specimen at a spread of settings and
        // looking at them. Higher intensity than this washes a saturated stroke
        // out towards white — the coral pentagon loses its coral first, which
        // is the tell — because a colour already near the top of the 0-1 range
        // has nowhere to go but grey when light is added to it.
        Self {
            threshold: 0.65,
            knee: 0.20,
            intensity: 0.45,
            passes: 3,
        }
    }
}

impl BloomSettings {
    /// The settings with every number forced into a range that draws something.
    ///
    /// A negative threshold makes every pixel including black glow, a zero knee
    /// divides by nothing, and zero passes leaves an unblurred bright-pass to be
    /// added back — which is not a glow but a second, brighter copy of the
    /// picture. None of those is worth a caller-facing error, so they are
    /// clamped and drawn.
    #[must_use]
    fn sane(self) -> Self {
        Self {
            threshold: self.threshold.max(0.0),
            knee: self.knee.clamp(1.0e-4, 1.0),
            intensity: self.intensity.max(0.0),
            passes: self.passes.clamp(1, MAX_PASSES),
        }
    }
}

/// Past this the blur has stopped being a glow and started being a fog, and the
/// cost is real: every pass is two more full-screen draws.
const MAX_PASSES: u32 = 8;

/// How much smaller the blur chain is than the screen.
///
/// A blur is the one effect that is better for being cheap. The downsample is
/// itself a blur, and the taps that follow reach four times as far across the
/// picture for the same cost — so a quarter-resolution chain is both faster and
/// *wider* than a full-resolution one.
const DOWNSCALE: u32 = 4;

/// The scene, its glow, and the passes between.
pub struct Bloom {
    pipelines: Pipelines,
    sampler: wgpu::Sampler,
    chain: Option<Chain>,
    output_format: wgpu::TextureFormat,
}

struct Pipelines {
    bright: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    sample_layout: wgpu::BindGroupLayout,
    glow_layout: wgpu::BindGroupLayout,
}

impl Bloom {
    /// The format the scene is drawn in, and so the format the frame's
    /// renderers must be built for.
    ///
    /// sRGB, like every other colour target here: the scene is a picture, and a
    /// picture is stored encoded. The chain's own targets are not — see
    /// [`chain::Chain::FORMAT`].
    pub const SCENE_FORMAT: wgpu::TextureFormat = crate::COLOR_TARGET_FORMAT;

    /// `output_format` is the format of the target [`Self::resolve`] will write
    /// to — a surface's negotiated format, usually.
    #[must_use]
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sindri bloom shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("bloom.wgsl"))),
        });
        let sample_layout = sample_layout(device);
        let glow_layout = glow_layout(device);
        let build = |label: &str,
                     entry: &str,
                     format: wgpu::TextureFormat,
                     layouts: &[Option<&wgpu::BindGroupLayout>]| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: layouts,
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                // No depth anywhere in the chain: these are picture-to-picture
                // passes over a full-screen triangle, and there is nothing in
                // front of anything.
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            pipelines: Pipelines {
                bright: build(
                    "Sindri bloom bright pass",
                    "fs_bright",
                    Chain::FORMAT,
                    &[Some(&sample_layout)],
                ),
                blur: build(
                    "Sindri bloom blur pass",
                    "fs_blur",
                    Chain::FORMAT,
                    &[Some(&sample_layout)],
                ),
                composite: build(
                    "Sindri bloom composite",
                    "fs_composite",
                    output_format,
                    &[Some(&sample_layout), Some(&glow_layout)],
                ),
                sample_layout,
                glow_layout,
            },
            // Linear and clamped: the chain is read between sizes, so the taps
            // have to interpolate, and a repeating edge would wrap one side of
            // the screen's light onto the other.
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Sindri bloom sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..wgpu::SamplerDescriptor::default()
            }),
            chain: None,
            output_format,
        }
    }

    /// The format [`Self::resolve`] writes.
    #[must_use]
    pub const fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format
    }

    /// Sizes the scene target and the chain to this viewport, rebuilding them
    /// only when the size actually changed.
    ///
    /// A zero on either axis leaves everything alone: a viewport with no area
    /// draws nothing, and a texture of no width is an error rather than an
    /// empty picture.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.matches(width, height))
        {
            return;
        }
        self.chain = Some(Chain::new(device, width, height, DOWNSCALE));
    }

    /// Where a frame should be drawn so that it can glow.
    ///
    /// `None` before the first [`Self::resize`], which is the one state a host
    /// can reach by drawing before it has said how big anything is.
    #[must_use]
    pub fn scene_view(&self) -> Option<&wgpu::TextureView> {
        self.chain.as_ref().map(Chain::scene_view)
    }

    /// Reads the scene, and writes it plus its glow to `target`.
    ///
    /// Does nothing without a scene to read — see [`Self::scene_view`].
    pub fn resolve(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        settings: BloomSettings,
    ) {
        let Some(chain) = self.chain.as_ref() else {
            return;
        };
        let settings = settings.sane();

        // Bright pass: the scene at full size into the chain at a quarter of it.
        // One draw does both the threshold and the downsample, because a
        // filtered read at a quarter size *is* the downsample.
        chain.run(
            device,
            queue,
            encoder,
            &self.pipelines,
            &self.sampler,
            chain::Step::Bright(settings),
        );
        for pass in 0..settings.passes {
            // Each sweep reaches twice as far as the last, so a few passes cover
            // a wide glow without a kernel wide enough to need one tap per texel
            // of it.
            #[allow(clippy::cast_precision_loss)]
            let reach = (1 << pass) as f32;
            chain.run(
                device,
                queue,
                encoder,
                &self.pipelines,
                &self.sampler,
                chain::Step::Blur {
                    direction: [reach, 0.0],
                },
            );
            chain.run(
                device,
                queue,
                encoder,
                &self.pipelines,
                &self.sampler,
                chain::Step::Blur {
                    direction: [0.0, reach],
                },
            );
        }
        chain.run(
            device,
            queue,
            encoder,
            &self.pipelines,
            &self.sampler,
            chain::Step::Composite {
                target,
                intensity: settings.intensity,
            },
        );
    }
}

fn sample_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri bloom sample layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn glow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri bloom glow layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::{BloomSettings, MAX_PASSES};

    /// Every setting that would draw something other than a glow is clamped
    /// rather than refused.
    ///
    /// None of these is worth failing a frame over, and each has an obvious
    /// nearest sane value: a negative threshold would make black itself glow, a
    /// zero knee divides by nothing, and zero blur passes would add an
    /// unblurred bright-pass back over the scene — a second brighter copy of
    /// the picture rather than light around it.
    #[test]
    fn settings_that_would_not_draw_a_glow_are_clamped() {
        let absurd = BloomSettings {
            threshold: -3.0,
            knee: 0.0,
            intensity: -1.0,
            passes: 0,
        }
        .sane();
        assert!(absurd.threshold >= 0.0);
        assert!(absurd.knee > 0.0);
        assert!(absurd.intensity >= 0.0);
        assert_eq!(absurd.passes, 1);

        let excessive = BloomSettings {
            passes: 500,
            knee: 9.0,
            ..BloomSettings::default()
        }
        .sane();
        assert_eq!(excessive.passes, MAX_PASSES);
        assert!(excessive.knee <= 1.0);
    }

    /// The default glows on the colours this exists for.
    ///
    /// Neon on black is not over-bright — a mint stroke peaks around 0.9 on its
    /// brightest channel — so a threshold at or above one would leave the whole
    /// picture matte, which is the failure that looks like bloom is broken
    /// rather than off.
    #[test]
    fn the_default_threshold_is_below_an_ordinary_bright_colour() {
        let settings = BloomSettings::default();
        assert!(settings.threshold < 0.9, "{settings:?}");
        assert!(settings.passes >= 1);
        assert!(settings.intensity > 0.0);
    }
}
