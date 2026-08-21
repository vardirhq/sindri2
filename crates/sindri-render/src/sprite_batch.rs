use std::borrow::Cow;

use glam::Mat4;
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{
    DepthTarget, MeshBuffers, SpriteBlendMode, TextureId, TextureRegistry, TexturedVertex, UvRect,
};

const SHADER: &str = include_str!("sprite_batch.wgsl");
const DEFAULT_CAPACITY: u32 = 64;
const VERTICES: [TexturedVertex; 4] = [
    TexturedVertex::new([-0.5, -0.5, 0.0], [0.0, 1.0]),
    TexturedVertex::new([0.5, -0.5, 0.0], [1.0, 1.0]),
    TexturedVertex::new([0.5, 0.5, 0.0], [1.0, 0.0]),
    TexturedVertex::new([-0.5, 0.5, 0.0], [0.0, 0.0]),
];
const INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

/// What a batch of sprites does about the depth the opaque stage wrote.
///
/// Sprites never write depth under either of these: blending is order
/// dependent, so a depth write would make the result depend on draw order
/// twice. What differs is whether something in front can hide them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpriteDepth {
    /// Draws over the world whatever the depth buffer holds. A screen-space
    /// overlay is not in the world, so nothing in the world may occlude it.
    #[default]
    Ignore,
    /// Hidden by opaque geometry nearer the camera, which is what being in the
    /// world means.
    Test,
}

impl SpriteDepth {
    const fn compare(self) -> wgpu::CompareFunction {
        match self {
            Self::Ignore => wgpu::CompareFunction::Always,
            Self::Test => wgpu::CompareFunction::Less,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
    uv_rect: [f32; 4],
}

impl SpriteInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4
    ];

    /// A sprite drawn from the whole of its texture.
    pub fn new(model: Mat4, tint: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            tint,
            uv_rect: UvRect::FULL.to_array(),
        }
    }

    /// Draws only part of the texture, which is what a sprite sheet is.
    ///
    /// Per instance rather than per batch, deliberately: a sheet's whole point
    /// is many different frames of one texture, and if the rect belonged to the
    /// batch then every frame would be its own draw call — which is the cost the
    /// sheet exists to avoid.
    #[must_use]
    pub fn with_uv_rect(mut self, uv_rect: UvRect) -> Self {
        self.uv_rect = uv_rect.to_array();
        self
    }

    /// The part of the texture this instance draws.
    pub fn uv_rect(self) -> UvRect {
        UvRect::from_array(self.uv_rect)
    }

    /// The per-instance tint in straight `[r, g, b, a]` order.
    pub const fn tint(self) -> [f32; 4] {
        self.tint
    }

    /// The per-instance model transform.
    pub fn model(self) -> Mat4 {
        Mat4::from_cols_array_2d(&self.model)
    }

    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BatchUniform {
    view_projection: [[f32; 4]; 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpriteBatchStats {
    sprite_count: u32,
    draw_calls: u32,
}

impl SpriteBatchStats {
    fn for_sprite_count(sprite_count: u32) -> Self {
        Self {
            sprite_count,
            draw_calls: u32::from(sprite_count > 0),
        }
    }

    pub const fn sprite_count(self) -> u32 {
        self.sprite_count
    }

    pub const fn draw_calls(self) -> u32 {
        self.draw_calls
    }

    pub const fn draw_calls_saved(self) -> u32 {
        self.sprite_count.saturating_sub(self.draw_calls)
    }

    /// One frame's running total, batch by batch.
    const fn and(self, other: Self) -> Self {
        Self {
            sprite_count: self.sprite_count.saturating_add(other.sprite_count),
            draw_calls: self.draw_calls.saturating_add(other.draw_calls),
        }
    }
}

/// One batch's own GPU resources.
///
/// Per batch rather than per renderer, and that is the whole point.
/// `queue.write_buffer` stages a write that lands *before* the command buffer
/// executes, so a renderer that wrote one uniform buffer once per batch gave
/// every pass in the frame whatever the last batch had put there. A frame with
/// a single sprite batch never notices; a scene with a world and an overlay
/// draws the world through the overlay's camera.
#[derive(Debug)]
struct Batch {
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: u32,
    /// One bind group per texture used in this slot. Keyed per slot because a
    /// bind group names the uniform buffer it reads, and each slot has its own.
    bind_groups: std::collections::HashMap<TextureId, wgpu::BindGroup>,
}

#[derive(Debug)]
pub struct SpriteBatchRenderer {
    /// One pipeline per depth behaviour, because the comparison is pipeline
    /// state: a batch cannot choose between them at draw time otherwise.
    over_the_world: wgpu::RenderPipeline,
    within_the_world: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    mesh: MeshBuffers,
    /// Grown as a frame needs them and reused every frame after.
    batches: Vec<Batch>,
    /// Which slot the next batch of this frame takes.
    next: usize,
    blend_mode: SpriteBlendMode,
    stats: SpriteBatchStats,
}

impl SpriteBatchRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::with_blend_mode(device, target_format, SpriteBlendMode::Alpha)
    }

    pub fn with_blend_mode(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        blend_mode: SpriteBlendMode,
    ) -> Self {
        let bind_group_layout = create_bind_group_layout(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sindri sprite batch shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sindri sprite batch pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = |depth: SpriteDepth| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sindri sprite batch pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        Some(TexturedVertex::layout()),
                        Some(SpriteInstance::layout()),
                    ],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(blend_mode.blend_state()),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTarget::FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(depth.compare()),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        Self {
            over_the_world: pipeline(SpriteDepth::Ignore),
            within_the_world: pipeline(SpriteDepth::Test),
            bind_group_layout,
            mesh: MeshBuffers::new(device, "Sindri sprite batch quad", &VERTICES, &INDICES),
            batches: Vec::new(),
            next: 0,
            blend_mode,
            stats: SpriteBatchStats::default(),
        }
    }

    /// Starts a submission, so the next batch takes the first slot again.
    ///
    /// Without this a long-running host would allocate a slot per batch per
    /// frame forever. Slots are reused rather than freed, so a frame costs
    /// nothing after the first one of its shape.
    ///
    /// A *submission* and not a frame, and the difference is the whole reason
    /// the slots exist: `queue.write_buffer` stages a write that lands when
    /// the queue is next submitted, so reusing a slot is only safe once the
    /// commands that read it have been submitted. Two frames encoded into one
    /// submission — an editor drawing a scene view and a game view, say — must
    /// either submit between them or keep drawing into fresh slots. The editor
    /// submits between them.
    pub fn begin_submission(&mut self) {
        self.next = 0;
        self.stats = SpriteBatchStats::default();
    }

    /// How many batch slots the renderer is holding.
    ///
    /// The high-water mark of one frame's batches, which is what a host would
    /// look at to find out whether a scene is asking for a great many.
    #[must_use]
    pub fn batch_slots(&self) -> usize {
        self.batches.len()
    }

    /// Draws one batch, with its own camera and its own instances.
    ///
    /// The batch's data goes in the slot [`Self::begin_submission`] handed
    /// out, so nothing another batch in the same submission writes can reach
    /// it.
    ///
    /// Preparing and encoding are one call because they were never separable:
    /// a batch's uniform and instance data are only correct for the pass drawn
    /// immediately after they are written, and splitting them left an API whose
    /// only safe use was to call the two in a pair. Now the pair is the call,
    /// and the slot it writes belongs to it.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth_target: &DepthTarget,
        registry: &TextureRegistry,
        texture: TextureId,
        view_projection: Mat4,
        depth: SpriteDepth,
        instances: &[SpriteInstance],
    ) -> Result<SpriteBatchStats, SpriteBatchError> {
        let instance_count =
            u32::try_from(instances.len()).map_err(|_| SpriteBatchError::TooManyInstances)?;
        let stats = SpriteBatchStats::for_sprite_count(instance_count);
        if instance_count == 0 {
            return Ok(stats);
        }

        let slot = self.reserve(device, instance_count)?;
        {
            let batch = &self.batches[slot];
            queue.write_buffer(
                &batch.uniform,
                0,
                bytemuck::bytes_of(&BatchUniform {
                    view_projection: view_projection.to_cols_array_2d(),
                }),
            );
            queue.write_buffer(&batch.instances, 0, bytemuck::cast_slice(instances));
        }
        self.bind_texture(device, registry, slot, texture);
        self.stats = self.stats.and(stats);

        let batch = &self.batches[slot];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sindri sprite batch pass"),
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
                // No depth operations at all: sprites read the buffer and never
                // write it, and saying so here is what makes that a rule rather
                // than a pipeline setting someone could change alone.
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(match depth {
            SpriteDepth::Ignore => &self.over_the_world,
            SpriteDepth::Test => &self.within_the_world,
        });
        pass.set_bind_group(
            0,
            batch
                .bind_groups
                .get(&texture)
                .expect("the bind group was just created"),
            &[],
        );
        pass.set_vertex_buffer(1, batch.instances.slice(..));
        self.mesh.draw_instances(&mut pass, 0..instance_count);
        Ok(stats)
    }

    /// The slot this frame's next batch draws from, grown to hold `capacity`.
    fn reserve(&mut self, device: &wgpu::Device, capacity: u32) -> Result<usize, SpriteBatchError> {
        let slot = self.next;
        self.next += 1;
        if slot == self.batches.len() {
            let wanted = DEFAULT_CAPACITY.max(capacity);
            self.batches.push(Batch {
                uniform: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Sindri sprite batch uniform"),
                    contents: bytemuck::bytes_of(&BatchUniform {
                        view_projection: Mat4::IDENTITY.to_cols_array_2d(),
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                }),
                instances: create_instance_buffer(device, wanted)?,
                capacity: wanted,
                bind_groups: std::collections::HashMap::new(),
            });
        }
        let batch = &mut self.batches[slot];
        if capacity > batch.capacity {
            let grown = capacity
                .checked_next_power_of_two()
                .ok_or(SpriteBatchError::TooManyInstances)?;
            batch.instances = create_instance_buffer(device, grown)?;
            batch.capacity = grown;
            // The bind groups name the uniform, not the instances, so they
            // survive the instance buffer being replaced.
        }
        Ok(slot)
    }

    /// Builds this slot's bind group for `texture` on first use.
    ///
    /// Keyed per slot rather than per renderer because a bind group names the
    /// uniform buffer it reads, and each slot has its own.
    fn bind_texture(
        &mut self,
        device: &wgpu::Device,
        registry: &TextureRegistry,
        slot: usize,
        texture: TextureId,
    ) {
        if self.batches[slot].bind_groups.contains_key(&texture) {
            return;
        }
        let resolved = registry.get(texture);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri sprite batch bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.batches[slot].uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(resolved.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(resolved.sampler()),
                },
            ],
        });
        self.batches[slot].bind_groups.insert(texture, bind_group);
    }

    /// What this frame has drawn since [`Self::begin_submission`], across every
    /// batch. [`Self::draw`] hands back one batch's share of it.
    pub const fn stats(&self) -> SpriteBatchStats {
        self.stats
    }

    pub const fn blend_mode(&self) -> SpriteBlendMode {
        self.blend_mode
    }
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri sprite batch bind group layout"),
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
) -> Result<wgpu::Buffer, SpriteBatchError> {
    let stride = u64::try_from(std::mem::size_of::<SpriteInstance>())
        .map_err(|_| SpriteBatchError::BufferSizeOverflow)?;
    let size = u64::from(capacity)
        .checked_mul(stride)
        .ok_or(SpriteBatchError::BufferSizeOverflow)?;
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Sindri sprite batch instances"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }))
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SpriteBatchError {
    #[error("sprite batch contains more instances than the renderer can address")]
    TooManyInstances,
    #[error("sprite batch instance buffer size overflowed")]
    BufferSizeOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_batch_emits_no_draw_calls() {
        let stats = SpriteBatchStats::for_sprite_count(0);
        assert_eq!(stats.draw_calls(), 0);
        assert_eq!(stats.draw_calls_saved(), 0);
    }

    #[test]
    fn batch_reduces_many_sprites_to_one_draw_call() {
        let stats = SpriteBatchStats::for_sprite_count(128);
        assert_eq!(stats.sprite_count(), 128);
        assert_eq!(stats.draw_calls(), 1);
        assert_eq!(stats.draw_calls_saved(), 127);
    }
}
