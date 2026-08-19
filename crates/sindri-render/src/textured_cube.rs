use std::borrow::Cow;

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::{DepthTarget, MeshBuffers, TextureId, TextureRegistry, TexturedVertex};

const SHADER: &str = include_str!("textured_cube.wgsl");

const FACE_UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

const VERTICES: [TexturedVertex; 24] = [
    TexturedVertex::new([-1.0, -1.0, 1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, -1.0, 1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, 1.0, 1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, 1.0, 1.0], FACE_UVS[3]),
    TexturedVertex::new([1.0, -1.0, -1.0], FACE_UVS[0]),
    TexturedVertex::new([-1.0, -1.0, -1.0], FACE_UVS[1]),
    TexturedVertex::new([-1.0, 1.0, -1.0], FACE_UVS[2]),
    TexturedVertex::new([1.0, 1.0, -1.0], FACE_UVS[3]),
    TexturedVertex::new([1.0, -1.0, 1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, -1.0, -1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, 1.0, -1.0], FACE_UVS[2]),
    TexturedVertex::new([1.0, 1.0, 1.0], FACE_UVS[3]),
    TexturedVertex::new([-1.0, -1.0, -1.0], FACE_UVS[0]),
    TexturedVertex::new([-1.0, -1.0, 1.0], FACE_UVS[1]),
    TexturedVertex::new([-1.0, 1.0, 1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, 1.0, -1.0], FACE_UVS[3]),
    TexturedVertex::new([-1.0, 1.0, 1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, 1.0, 1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, 1.0, -1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, 1.0, -1.0], FACE_UVS[3]),
    TexturedVertex::new([-1.0, -1.0, -1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, -1.0, -1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, -1.0, 1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, -1.0, 1.0], FACE_UVS[3]),
];

const INDICES: [u16; 36] = [
    0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16, 17, 18,
    18, 19, 16, 20, 21, 22, 22, 23, 20,
];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CubeUniform {
    model_view_projection: [[f32; 4]; 4],
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri textured cube bind group layout"),
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

fn create_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Sindri textured cube shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Sindri textured cube pipeline layout"),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Sindri textured cube pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(TexturedVertex::layout())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(target_format.into())],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DepthTarget::FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[derive(Debug)]
pub struct TexturedCubeRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// One bind group per texture, built on first use and kept for reuse.
    bind_groups: std::collections::HashMap<TextureId, wgpu::BindGroup>,
    current: TextureId,
    uniform: wgpu::Buffer,
    mesh: MeshBuffers,
}

/// The GPU handles and texture a draw resolves against.
///
/// Bundled because a draw needs all of them together, and threading four more
/// parameters through every encode call obscures what is actually being drawn.
#[derive(Clone, Copy)]
pub struct DrawContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub textures: &'a TextureRegistry,
    pub texture: TextureId,
}

impl TexturedCubeRenderer {
    /// Textures come from the frame's [`TextureRegistry`] rather than being
    /// baked in, so one renderer draws every mesh in a scene.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sindri textured cube uniform"),
            contents: bytemuck::bytes_of(&CubeUniform {
                model_view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = create_bind_group_layout(device);
        let pipeline = create_pipeline(device, target_format, &bind_group_layout);
        Self {
            pipeline,
            bind_group_layout,
            bind_groups: std::collections::HashMap::new(),
            current: TextureRegistry::MISSING,
            uniform,
            mesh: MeshBuffers::new(device, "Sindri textured cube", &VERTICES, &INDICES),
        }
    }

    /// Returns the bind group for `texture`, creating it on first use.
    fn bind_texture(
        &mut self,
        device: &wgpu::Device,
        registry: &TextureRegistry,
        texture: TextureId,
    ) {
        self.current = texture;
        if self.bind_groups.contains_key(&texture) {
            return;
        }
        let resolved = registry.get(texture);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri textured cube bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
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
        self.bind_groups.insert(texture, bind_group);
    }

    /// Draws the cube into a frame something else has already cleared.
    ///
    /// Both attachments load: a renderer draws one thing, and deciding what the
    /// rest of the frame starts as is not its to make. See
    /// [`encode_clear`](crate::encode_clear).
    pub fn encode(
        &mut self,
        context: DrawContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &DepthTarget,
        model_view_projection: Mat4,
    ) {
        self.bind_texture(context.device, context.textures, context.texture);
        let queue = context.queue;
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&CubeUniform {
                model_view_projection: model_view_projection.to_cols_array_2d(),
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sindri textured cube pass"),
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
                view: depth.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        let bind_group = self
            .bind_groups
            .get(&self.current)
            .expect("the texture is bound before encoding");
        pass.set_bind_group(0, bind_group, &[]);
        self.mesh.draw(&mut pass);
    }

    /// The texture the next encode will draw with.
    pub const fn texture(&self) -> TextureId {
        self.current
    }
}
