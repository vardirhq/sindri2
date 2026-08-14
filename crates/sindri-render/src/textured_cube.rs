use std::borrow::Cow;

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::{DepthTarget, MeshBuffers, Texture2D, TexturedVertex};

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
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    mesh: MeshBuffers,
    texture: Texture2D,
}

impl TexturedCubeRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let texture = Texture2D::checkerboard(
            device,
            queue,
            "Sindri cube checkerboard",
            64,
            8,
            [[18, 34, 55, 255], [240, 114, 43, 255]],
        )
        .expect("static checkerboard texture dimensions are valid");
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sindri textured cube uniform"),
            contents: bytemuck::bytes_of(&CubeUniform {
                model_view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = create_bind_group_layout(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri textured cube bind group"),
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
        let pipeline = create_pipeline(device, target_format, &bind_group_layout);
        Self {
            pipeline,
            bind_group,
            uniform,
            mesh: MeshBuffers::new(device, "Sindri textured cube", &VERTICES, &INDICES),
            texture,
        }
    }

    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &DepthTarget,
        model_view_projection: Mat4,
    ) {
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
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.018,
                        g: 0.025,
                        b: 0.045,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        self.mesh.draw(&mut pass);
    }

    pub const fn texture(&self) -> &Texture2D {
        &self.texture
    }
}
