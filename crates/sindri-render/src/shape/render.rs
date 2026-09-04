//! Drawing shape quads: one pipeline per blend mode, one batch per shape pass.
//!
//! The glyph renderer's shape, minus the atlas. A shape samples nothing, so
//! there is no texture to bind and no build number to invalidate a bind group
//! against — a slot's bind group names its uniform and never changes.
//!
//! Two pipelines rather than one because the blend mode has to be chosen when
//! the pipeline is built, and both are wanted: a stroke over the picture behind
//! it is alpha, and light thrown into the dark is additive. Both are built up
//! front because they are cheap and a pass that picked one would otherwise stall
//! the first time a scene used the other.

use std::borrow::Cow;

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::{DepthTarget, MeshBuffers, SpriteBlendMode, TexturedVertex};

use super::instance::{ShapeInstance, ShapeUniform};

const SHADER: &str = include_str!("shape.wgsl");

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
/// Per batch for the reason the glyph and sprite batches' are:
/// `queue.write_buffer` stages a write that lands before the command buffer
/// executes, so a renderer with one uniform buffer would draw every pass of the
/// frame through the last pass's camera.
struct Batch {
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: u32,
    bind_group: wgpu::BindGroup,
}

pub struct ShapeRenderer {
    /// Alpha and additive, in that order.
    pipelines: [wgpu::RenderPipeline; 2],
    bind_group_layout: wgpu::BindGroupLayout,
    mesh: MeshBuffers,
    batches: Vec<Batch>,
    next: usize,
}

impl ShapeRenderer {
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = create_bind_group_layout(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sindri shape shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sindri shape pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = |blend: SpriteBlendMode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sindri shape pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        Some(TexturedVertex::layout()),
                        Some(ShapeInstance::layout()),
                    ],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(blend.blend_state()),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                // Read the depth buffer, never write it: the same rule every
                // other transparent thing here follows, because blending is
                // order dependent and a depth write would make the result depend
                // on draw order twice.
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
            })
        };

        Self {
            // The shader writes premultiplied colour, which is what lets one
            // shader serve both: premultiplied over is a normal composite, and
            // premultiplied added is light.
            pipelines: [
                pipeline(SpriteBlendMode::PremultipliedAlpha),
                pipeline(SpriteBlendMode::Additive),
            ],
            bind_group_layout,
            mesh: MeshBuffers::new(device, "Sindri shape quad", &VERTICES, &INDICES),
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

    /// Draws one shape pass through its own camera.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth_target: &DepthTarget,
        blend: ShapeBlend,
        view_projection: Mat4,
        instances: &[ShapeInstance],
    ) -> Result<(), ShapeDrawError> {
        let instance_count =
            u32::try_from(instances.len()).map_err(|_| ShapeDrawError::TooManyShapes)?;
        if instance_count == 0 {
            return Ok(());
        }
        let slot = self.reserve(device, instance_count)?;
        let batch = &self.batches[slot];
        queue.write_buffer(
            &batch.uniform,
            0,
            bytemuck::bytes_of(&ShapeUniform {
                view_projection: view_projection.to_cols_array_2d(),
            }),
        );
        queue.write_buffer(&batch.instances, 0, bytemuck::cast_slice(instances));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sindri shape pass"),
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
        pass.set_pipeline(&self.pipelines[blend as usize]);
        pass.set_bind_group(0, &batch.bind_group, &[]);
        pass.set_vertex_buffer(1, batch.instances.slice(..));
        self.mesh.draw_instances(&mut pass, 0..instance_count);
        Ok(())
    }

    /// The slot this submission's next batch draws from, grown to hold
    /// `capacity` shapes.
    fn reserve(&mut self, device: &wgpu::Device, capacity: u32) -> Result<usize, ShapeDrawError> {
        let slot = self.next;
        self.next += 1;
        if slot == self.batches.len() {
            let wanted = DEFAULT_CAPACITY.max(capacity);
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sindri shape uniform"),
                contents: bytemuck::bytes_of(&ShapeUniform {
                    view_projection: Mat4::IDENTITY.to_cols_array_2d(),
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Sindri shape bind group"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            });
            self.batches.push(Batch {
                uniform,
                instances: create_instance_buffer(device, wanted)?,
                capacity: wanted,
                bind_group,
            });
        }
        let batch = &mut self.batches[slot];
        if capacity > batch.capacity {
            let grown = capacity
                .checked_next_power_of_two()
                .ok_or(ShapeDrawError::TooManyShapes)?;
            batch.instances = create_instance_buffer(device, grown)?;
            batch.capacity = grown;
            // The bind group names the uniform, not the instances, so it
            // survives the instance buffer being replaced.
        }
        Ok(slot)
    }
}

/// How a batch of shapes meets what is already on the target.
///
/// Its own two-value type rather than [`SpriteBlendMode`], because these are the
/// two the shape shader's premultiplied output is correct for. Opaque would
/// throw away the antialiasing that is the whole point of the distance field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShapeBlend {
    /// Drawn over what is behind it.
    #[default]
    Over,
    /// Added to what is behind it: light rather than paint.
    Add,
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri shape bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn create_instance_buffer(
    device: &wgpu::Device,
    capacity: u32,
) -> Result<wgpu::Buffer, ShapeDrawError> {
    let stride = u64::try_from(std::mem::size_of::<ShapeInstance>())
        .map_err(|_| ShapeDrawError::BufferSizeOverflow)?;
    let size = u64::from(capacity)
        .checked_mul(stride)
        .ok_or(ShapeDrawError::BufferSizeOverflow)?;
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Sindri shape instances"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }))
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShapeDrawError {
    #[error("a shape pass contains more shapes than the renderer can address")]
    TooManyShapes,
    #[error("shape instance buffer size overflowed")]
    BufferSizeOverflow,
}
