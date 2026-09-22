//! Directional shadow-map state shared by textured world geometry.

use glam::{Mat4, Vec3, Vec4};

use crate::WorldLighting;

const MIN_MAP_SIZE: u32 = 256;
const MAX_MAP_SIZE: u32 = 2048;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowSettings {
    pub enabled: bool,
    pub distance: f32,
    pub map_size: u32,
    pub bias: f32,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            distance: 48.0,
            map_size: 1024,
            bias: 0.002,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ShadowMap {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    size: u32,
}

impl ShadowMap {
    pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub(crate) fn new(device: &wgpu::Device, size: u32) -> Self {
        let size = size.clamp(MIN_MAP_SIZE, MAX_MAP_SIZE);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sindri directional shadow map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sindri directional shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        Self {
            _texture: texture,
            view,
            sampler,
            size,
        }
    }

    pub(crate) fn resize(&mut self, device: &wgpu::Device, size: u32) -> bool {
        let size = size.clamp(MIN_MAP_SIZE, MAX_MAP_SIZE);
        if size == self.size {
            return false;
        }
        *self = Self::new(device, size);
        true
    }

    pub(crate) const fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
    pub(crate) const fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

pub(crate) fn light_view_projection(
    camera_view_projection: Mat4,
    lighting: WorldLighting,
    settings: ShadowSettings,
) -> Mat4 {
    let inverse = camera_view_projection.inverse();
    let center = inverse * Vec4::new(0.0, 0.0, 0.5, 1.0);
    let center = if center.w.abs() > f32::EPSILON {
        center.truncate() / center.w
    } else {
        Vec3::ZERO
    };
    let direction = Vec3::from_array(lighting.directional_direction).normalize_or_zero();
    let direction = if direction.length_squared() > f32::EPSILON {
        direction
    } else {
        Vec3::NEG_Y
    };
    let distance = settings.distance.max(1.0);
    let eye = center - direction * distance;
    let up = if direction.dot(Vec3::Y).abs() > 0.98 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = glam::camera::rh::view::look_at_mat4(eye, center, up);
    let projection = glam::camera::rh::proj::directx::orthographic(
        -distance,
        distance,
        -distance,
        distance,
        0.1,
        distance * 2.5,
    );
    projection * view
}

const SHADOW_SHADER: &str = r"
struct ShadowUniform { light_model_view_projection: mat4x4<f32>, }
@group(0) @binding(0) var<uniform> shadow: ShadowUniform;
struct VertexInput { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32>, }
@vertex fn vs_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow.light_model_view_projection * vec4<f32>(input.position, 1.0);
}
"#;

pub(crate) fn create_shadow_pipeline(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri shadow bind group layout"),
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
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Sindri shadow shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SHADOW_SHADER)),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Sindri shadow pipeline layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Sindri shadow pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(crate::TexturedVertex::layout())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: ShadowMap::FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    (bind_group_layout, pipeline)
}
