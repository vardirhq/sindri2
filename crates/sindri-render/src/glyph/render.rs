//! Drawing glyph quads: one pipeline, one atlas, one batch per text pass.
//!
//! Its own pipeline rather than the sprite one, and that is what a distance
//! field buys. A sprite shader multiplies a picture by a tint, which is all a
//! coverage mask can be asked for. A field is read against thresholds, so the
//! same quad can be an edge found exactly at the size it is drawn, an outline a
//! stroke's width outside that edge, and a shadow softened by however much —
//! none of which is a second texture or a second bake.
//!
//! Everything else is the sprite batch's shape, deliberately: instanced quads,
//! one uniform slot per batch per submission, the camera coming from the pass.
//! Text is geometry, so it is drawn the way geometry is drawn.

use std::borrow::Cow;

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::{DepthTarget, MeshBuffers, SpriteBlendMode, Texture2D, TexturedVertex};

use super::instance::{GlyphInstance, GlyphUniform};

const SHADER: &str = include_str!("glyph.wgsl");

const DEFAULT_CAPACITY: u32 = 256;

const VERTICES: [TexturedVertex; 4] = [
    TexturedVertex::new([-0.5, -0.5, 0.0], [0.0, 1.0]),
    TexturedVertex::new([0.5, -0.5, 0.0], [1.0, 1.0]),
    TexturedVertex::new([0.5, 0.5, 0.0], [1.0, 0.0]),
    TexturedVertex::new([-0.5, 0.5, 0.0], [0.0, 0.0]),
];

const INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

/// One batch's own GPU resources.
///
/// Per batch for the reason the sprite batch's are: `queue.write_buffer` stages
/// a write that lands before the command buffer executes, so a renderer with one
/// uniform buffer would draw every pass of the frame through the last pass's
/// camera.
struct Batch {
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: u32,
    /// The atlas build this slot's bind group names. A grown atlas is a new
    /// texture, and a bind group naming the old one would sample freed memory.
    bound: Option<(u64, wgpu::BindGroup)>,
}

pub struct GlyphRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    mesh: MeshBuffers,
    batches: Vec<Batch>,
    next: usize,
}

impl GlyphRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = create_bind_group_layout(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sindri glyph shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sindri glyph pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sindri glyph pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(TexturedVertex::layout()),
                    Some(GlyphInstance::layout()),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(SpriteBlendMode::Alpha.blend_state()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            // Text reads the depth buffer and never writes it, which is what
            // every other transparent thing here does and for the same reason:
            // blending is order dependent, and a depth write would make the
            // result depend on draw order twice.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTarget::FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            mesh: MeshBuffers::new(device, "Sindri glyph quad", &VERTICES, &INDICES),
            batches: Vec::new(),
            next: 0,
        }
    }

    /// Starts a submission, so the next batch takes the first slot again.
    pub fn begin_submission(&mut self) {
        self.next = 0;
    }

    /// How many batch slots the renderer is holding.
    #[must_use]
    pub fn batch_slots(&self) -> usize {
        self.batches.len()
    }

    /// Draws one text pass through its own camera.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth_target: &DepthTarget,
        atlas: &Texture2D,
        build: u64,
        view_projection: Mat4,
        instances: &[GlyphInstance],
    ) -> Result<(), GlyphDrawError> {
        let instance_count =
            u32::try_from(instances.len()).map_err(|_| GlyphDrawError::TooManyGlyphs)?;
        if instance_count == 0 {
            return Ok(());
        }
        let slot = self.reserve(device, instance_count)?;
        {
            let batch = &self.batches[slot];
            queue.write_buffer(
                &batch.uniform,
                0,
                bytemuck::bytes_of(&GlyphUniform {
                    view_projection: view_projection.to_cols_array_2d(),
                }),
            );
            queue.write_buffer(&batch.instances, 0, bytemuck::cast_slice(instances));
        }
        self.bind_atlas(device, atlas, build, slot);

        let batch = &self.batches[slot];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sindri glyph pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target.view(),
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(
            0,
            &batch
                .bound
                .as_ref()
                .expect("the bind group was just created")
                .1,
            &[],
        );
        pass.set_vertex_buffer(1, batch.instances.slice(..));
        self.mesh.draw_instances(&mut pass, 0..instance_count);
        Ok(())
    }

    /// The slot this submission's next batch draws from, grown to hold
    /// `capacity` glyphs.
    fn reserve(&mut self, device: &wgpu::Device, capacity: u32) -> Result<usize, GlyphDrawError> {
        let slot = self.next;
        self.next += 1;
        if slot == self.batches.len() {
            let wanted = DEFAULT_CAPACITY.max(capacity);
            self.batches.push(Batch {
                uniform: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Sindri glyph uniform"),
                    contents: bytemuck::bytes_of(&GlyphUniform {
                        view_projection: Mat4::IDENTITY.to_cols_array_2d(),
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                }),
                instances: create_instance_buffer(device, wanted)?,
                capacity: wanted,
                bound: None,
            });
        }
        let batch = &mut self.batches[slot];
        if capacity > batch.capacity {
            let grown = capacity
                .checked_next_power_of_two()
                .ok_or(GlyphDrawError::TooManyGlyphs)?;
            batch.instances = create_instance_buffer(device, grown)?;
            batch.capacity = grown;
            // The bind group names the uniform, not the instances, so it
            // survives the instance buffer being replaced.
        }
        Ok(slot)
    }

    /// Builds this slot's bind group when it has none for this atlas build.
    fn bind_atlas(&mut self, device: &wgpu::Device, atlas: &Texture2D, build: u64, slot: usize) {
        if self.batches[slot]
            .bound
            .as_ref()
            .is_some_and(|(bound, _)| *bound == build)
        {
            return;
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri glyph bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.batches[slot].uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas.sampler()),
                },
            ],
        });
        self.batches[slot].bound = Some((build, bind_group));
    }
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri glyph bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
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

fn create_instance_buffer(
    device: &wgpu::Device,
    capacity: u32,
) -> Result<wgpu::Buffer, GlyphDrawError> {
    let stride = u64::try_from(std::mem::size_of::<GlyphInstance>())
        .map_err(|_| GlyphDrawError::BufferSizeOverflow)?;
    let size = u64::from(capacity)
        .checked_mul(stride)
        .ok_or(GlyphDrawError::BufferSizeOverflow)?;
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Sindri glyph instances"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }))
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum GlyphDrawError {
    #[error("a text pass contains more glyphs than the renderer can address")]
    TooManyGlyphs,
    #[error("glyph instance buffer size overflowed")]
    BufferSizeOverflow,
}
