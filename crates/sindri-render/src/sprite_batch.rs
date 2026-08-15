use std::borrow::Cow;

use glam::Mat4;
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::{MeshBuffers, SpriteBlendMode, Texture2D, TexturedVertex};

const SHADER: &str = include_str!("sprite_batch.wgsl");
const DEFAULT_CAPACITY: u32 = 64;
const VERTICES: [TexturedVertex; 4] = [
    TexturedVertex::new([-0.5, -0.5, 0.0], [0.0, 1.0]),
    TexturedVertex::new([0.5, -0.5, 0.0], [1.0, 1.0]),
    TexturedVertex::new([0.5, 0.5, 0.0], [1.0, 0.0]),
    TexturedVertex::new([-0.5, 0.5, 0.0], [0.0, 0.0]),
];
const INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
}

impl SpriteInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4
    ];

    pub fn new(model: Mat4, tint: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            tint,
        }
    }

    /// The per-instance tint in straight `[r, g, b, a]` order.
    pub const fn tint(self) -> [f32; 4] {
        self.tint
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
}

#[derive(Debug)]
pub struct SpriteBatchRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    mesh: MeshBuffers,
    instances: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,
    texture: Texture2D,
    blend_mode: SpriteBlendMode,
    stats: SpriteBatchStats,
}

impl SpriteBatchRenderer {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        texture: Texture2D,
    ) -> Self {
        Self::with_blend_mode(device, target_format, texture, SpriteBlendMode::Alpha)
    }

    pub fn with_blend_mode(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        texture: Texture2D,
        blend_mode: SpriteBlendMode,
    ) -> Self {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sindri sprite batch uniform"),
            contents: bytemuck::bytes_of(&BatchUniform {
                view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let instances = create_instance_buffer(device, DEFAULT_CAPACITY)
            .expect("default sprite batch capacity fits a GPU buffer");
        let bind_group_layout = create_bind_group_layout(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri sprite batch bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(texture.sampler()),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sindri sprite batch shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sindri sprite batch pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform,
            mesh: MeshBuffers::new(device, "Sindri sprite batch quad", &VERTICES, &INDICES),
            instances,
            instance_capacity: DEFAULT_CAPACITY,
            instance_count: 0,
            texture,
            blend_mode,
            stats: SpriteBatchStats::default(),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[SpriteInstance],
    ) -> Result<SpriteBatchStats, SpriteBatchError> {
        let instance_count =
            u32::try_from(instances.len()).map_err(|_| SpriteBatchError::TooManyInstances)?;
        if instance_count > self.instance_capacity {
            let new_capacity = instance_count
                .checked_next_power_of_two()
                .ok_or(SpriteBatchError::TooManyInstances)?;
            self.instances = create_instance_buffer(device, new_capacity)?;
            self.instance_capacity = new_capacity;
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(instances));
        }
        self.instance_count = instance_count;
        self.stats = SpriteBatchStats::for_sprite_count(instance_count);
        Ok(self.stats)
    }

    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        view_projection: Mat4,
    ) {
        if self.instance_count == 0 {
            return;
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&BatchUniform {
                view_projection: view_projection.to_cols_array_2d(),
            }),
        );
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
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.slice(..));
        self.mesh.draw_instances(&mut pass, 0..self.instance_count);
    }

    pub const fn stats(&self) -> SpriteBatchStats {
        self.stats
    }

    pub const fn texture(&self) -> &Texture2D {
        &self.texture
    }

    pub const fn blend_mode(&self) -> SpriteBlendMode {
        self.blend_mode
    }

    pub const fn instance_capacity(&self) -> u32 {
        self.instance_capacity
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
