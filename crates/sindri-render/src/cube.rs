use std::borrow::Cow;

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::{ColoredVertex, DepthTarget, MeshBuffers};

const SHADER: &str = include_str!("cube.wgsl");

const VERTICES: [ColoredVertex; 8] = [
    ColoredVertex {
        position: [-1.0, -1.0, 1.0],
        color: [1.0, 0.2, 0.2],
    },
    ColoredVertex {
        position: [1.0, -1.0, 1.0],
        color: [0.2, 1.0, 0.2],
    },
    ColoredVertex {
        position: [1.0, 1.0, 1.0],
        color: [0.2, 0.4, 1.0],
    },
    ColoredVertex {
        position: [-1.0, 1.0, 1.0],
        color: [1.0, 0.8, 0.2],
    },
    ColoredVertex {
        position: [-1.0, -1.0, -1.0],
        color: [0.8, 0.2, 1.0],
    },
    ColoredVertex {
        position: [1.0, -1.0, -1.0],
        color: [0.2, 1.0, 1.0],
    },
    ColoredVertex {
        position: [1.0, 1.0, -1.0],
        color: [1.0, 0.4, 0.8],
    },
    ColoredVertex {
        position: [-1.0, 1.0, -1.0],
        color: [0.9, 0.9, 0.9],
    },
];

const INDICES: [u16; 36] = [
    0, 1, 2, 2, 3, 0, 1, 5, 6, 6, 2, 1, 5, 4, 7, 7, 6, 5, 4, 0, 3, 3, 7, 4, 3, 2, 6, 6, 7, 3, 4, 5,
    1, 1, 0, 4,
];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CubeUniform {
    model_view_projection: [[f32; 4]; 4],
}

#[derive(Debug)]
pub struct CubeRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    mesh: MeshBuffers,
}

impl CubeRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sindri cube uniform"),
            contents: bytemuck::bytes_of(&CubeUniform {
                model_view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sindri cube bind group layout"),
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
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri cube bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sindri cube shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sindri cube pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sindri cube pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ColoredVertex::layout())],
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
        });
        Self {
            pipeline,
            bind_group,
            uniform,
            mesh: MeshBuffers::new(device, "Sindri cube", &VERTICES, &INDICES),
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
            label: Some("Sindri cube pass"),
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
}
