use std::borrow::Cow;

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::{
    CachedMeshId, DepthTarget, MeshBuffers, ShadowSettings, TextureId, TextureRegistry,
    TexturedMeshCacheStats, TexturedVertex, WorldLighting,
    shadow::{ShadowMap, create_shadow_pipeline},
    textured_mesh_cache::TexturedMeshCache,
};

const SHADER: &str = include_str!("textured_cube.wgsl");

mod geometry;
mod shadow_pass;

use geometry::{INDICES, VERTICES};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CubeUniform {
    model_view_projection: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    ambient: [f32; 4],
    directional_direction: [f32; 4],
    directional_color: [f32; 4],
    light_view_projection: [[f32; 4]; 4],
    shadow: [f32; 4],
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Sindri textured cube bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
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
    batches: Vec<MeshBatch>,
    next: usize,
    mesh: MeshBuffers,
    cached_meshes: TexturedMeshCache,
    lighting: WorldLighting,
    shadow_settings: ShadowSettings,
    shadow_map: ShadowMap,
    shadow_bind_group_layout: wgpu::BindGroupLayout,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_view_projection: Mat4,
    ambient_occlusion_strength: f32,
}

/// GPU state owned by one textured-mesh draw in a submission.
///
/// Queue writes land before the submitted command buffer runs, so sharing a
/// uniform across draws would make every mesh use the final draw's transform.
#[derive(Debug)]
struct MeshBatch {
    uniform: wgpu::Buffer,
    shadow_uniform: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    bind_groups: std::collections::HashMap<TextureId, wgpu::BindGroup>,
}

mod context;
pub(crate) use context::CachedMeshRequest;
pub use context::DrawContext;

impl TexturedCubeRenderer {
    /// Textures come from the frame's [`TextureRegistry`] rather than being
    /// baked in, so one renderer draws every mesh in a scene.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = create_bind_group_layout(device);
        let pipeline = create_pipeline(device, target_format, &bind_group_layout);
        let shadow_map = ShadowMap::new(device, ShadowSettings::default().map_size);
        let (shadow_bind_group_layout, shadow_pipeline) = create_shadow_pipeline(device);
        Self {
            pipeline,
            bind_group_layout,
            batches: Vec::new(),
            next: 0,
            mesh: MeshBuffers::new(device, "Sindri textured cube", &VERTICES, &INDICES),
            cached_meshes: TexturedMeshCache::default(),
            lighting: WorldLighting::default(),
            shadow_settings: ShadowSettings::default(),
            shadow_map,
            shadow_bind_group_layout,
            shadow_pipeline,
            shadow_view_projection: Mat4::IDENTITY,
            ambient_occlusion_strength: 0.0,
        }
    }

    /// Sets the world lighting used by subsequent textured 3D draws.
    pub fn set_lighting(&mut self, lighting: WorldLighting) {
        self.lighting = lighting;
    }

    pub fn set_shadows(&mut self, device: &wgpu::Device, settings: ShadowSettings) {
        self.shadow_settings = settings;
        if self.shadow_map.resize(device, settings.map_size) {
            for batch in &mut self.batches {
                batch.bind_groups.clear();
            }
        }
    }

    /// Sets how strongly mesh-authored ambient occlusion darkens world geometry.
    pub fn set_ambient_occlusion(&mut self, strength: f32) {
        self.ambient_occlusion_strength = strength.clamp(0.0, 1.0);
    }

    pub(crate) const fn shadows_enabled(&self) -> bool {
        self.shadow_settings.enabled && self.lighting.directional_intensity > 0.0
    }

    /// Makes the reusable draw slots available for a new GPU submission.
    pub fn begin_submission(&mut self) {
        self.next = 0;
    }

    fn reserve(&mut self, device: &wgpu::Device) -> usize {
        let slot = self.next;
        self.next += 1;
        if slot == self.batches.len() {
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sindri textured mesh uniform"),
                contents: bytemuck::bytes_of(&cube_uniform(
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                    WorldLighting::default(),
                    Mat4::IDENTITY,
                    ShadowSettings::default(),
                    0.0,
                )),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let shadow_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sindri shadow mesh uniform"),
                contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Sindri shadow mesh bind group"),
                layout: &self.shadow_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_uniform.as_entire_binding(),
                }],
            });
            self.batches.push(MeshBatch {
                uniform,
                shadow_uniform,
                shadow_bind_group,
                bind_groups: std::collections::HashMap::new(),
            });
        }
        slot
    }

    /// Returns the bind group for `texture`, creating it on first use.
    fn bind_texture(
        &mut self,
        device: &wgpu::Device,
        registry: &TextureRegistry,
        slot: usize,
        texture: TextureId,
    ) {
        let batch = &mut self.batches[slot];
        if batch.bind_groups.contains_key(&texture) {
            return;
        }
        let resolved = registry.get(texture);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sindri textured cube bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: batch.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(resolved.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(resolved.sampler()),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(self.shadow_map.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(self.shadow_map.sampler()),
                },
            ],
        });
        batch.bind_groups.insert(texture, bind_group);
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
        let slot = self.reserve(context.device);
        self.bind_texture(context.device, context.textures, slot, context.texture);
        let batch = &self.batches[slot];
        let bind_group = batch
            .bind_groups
            .get(&context.texture)
            .expect("the texture is bound before encoding");
        encode_mesh_buffers(
            context.queue,
            encoder,
            (target, depth),
            Mat4::IDENTITY,
            model_view_projection,
            self.lighting,
            self.shadow_view_projection,
            self.shadow_settings,
            self.ambient_occlusion_strength,
            (&batch.uniform, &self.pipeline, bind_group),
            &self.mesh,
            "Sindri textured cube pass",
        );
    }

    /// Draws a cube with a separate model transform so lighting stays in world space.
    pub fn encode_world(
        &mut self,
        context: DrawContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &DepthTarget,
        model: Mat4,
        view_projection: Mat4,
    ) {
        let slot = self.reserve(context.device);
        self.bind_texture(context.device, context.textures, slot, context.texture);
        let batch = &self.batches[slot];
        let bind_group = batch
            .bind_groups
            .get(&context.texture)
            .expect("the texture is bound before encoding");
        encode_mesh_buffers(
            context.queue,
            encoder,
            (target, depth),
            model,
            view_projection * model,
            self.lighting,
            self.shadow_view_projection,
            self.shadow_settings,
            self.ambient_occlusion_strength,
            (&batch.uniform, &self.pipeline, bind_group),
            &self.mesh,
            "Sindri textured cube pass",
        );
    }

    /// Draws caller-provided textured triangles through the same opaque 3D
    /// pipeline as cubes.
    ///
    /// This remains the transient path for authored surface meshes. Generated
    /// terrain should use [`Self::encode_cached_mesh`].
    pub fn encode_mesh(
        &mut self,
        context: DrawContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: (&wgpu::TextureView, &DepthTarget),
        model_view_projection: Mat4,
        vertices: &[TexturedVertex],
        indices: &[u16],
    ) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        let mesh = MeshBuffers::new(context.device, "Sindri textured surface", vertices, indices);
        let slot = self.reserve(context.device);
        self.bind_texture(context.device, context.textures, slot, context.texture);
        let batch = &self.batches[slot];
        let bind_group = batch
            .bind_groups
            .get(&context.texture)
            .expect("the texture is bound before encoding");
        encode_mesh_buffers(
            context.queue,
            encoder,
            target,
            Mat4::IDENTITY,
            model_view_projection,
            self.lighting,
            self.shadow_view_projection,
            self.shadow_settings,
            self.ambient_occlusion_strength,
            (&batch.uniform, &self.pipeline, bind_group),
            &mesh,
            "Sindri textured surface pass",
        );
    }

    /// Draws caller-provided textured triangles with world-space lighting.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_mesh_world(
        &mut self,
        context: DrawContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: (&wgpu::TextureView, &DepthTarget),
        model: Mat4,
        view_projection: Mat4,
        vertices: &[TexturedVertex],
        indices: &[u16],
    ) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        let mesh = MeshBuffers::new(context.device, "Sindri textured surface", vertices, indices);
        let slot = self.reserve(context.device);
        self.bind_texture(context.device, context.textures, slot, context.texture);
        let batch = &self.batches[slot];
        let bind_group = batch
            .bind_groups
            .get(&context.texture)
            .expect("the texture is bound before encoding");
        encode_mesh_buffers(
            context.queue,
            encoder,
            target,
            model,
            view_projection * model,
            self.lighting,
            self.shadow_view_projection,
            self.shadow_settings,
            self.ambient_occlusion_strength,
            (&batch.uniform, &self.pipeline, bind_group),
            &mesh,
            "Sindri textured surface pass",
        );
    }

    /// Draws a persistent textured mesh, replacing its GPU buffers only when a
    /// newer revision supplies geometry.
    pub(crate) fn encode_cached_mesh(
        &mut self,
        context: DrawContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: (&wgpu::TextureView, &DepthTarget),
        model: Mat4,
        view_projection: Mat4,
        request: CachedMeshRequest<'_>,
    ) {
        let slot = self.reserve(context.device);
        self.bind_texture(context.device, context.textures, slot, context.texture);
        let Some(mesh) = self.cached_meshes.resolve(
            context.device,
            request.id,
            request.revision,
            request.replacement,
        ) else {
            return;
        };
        let batch = &self.batches[slot];
        let bind_group = batch
            .bind_groups
            .get(&context.texture)
            .expect("the texture is bound before encoding");
        encode_mesh_buffers(
            context.queue,
            encoder,
            target,
            model,
            view_projection * model,
            self.lighting,
            self.shadow_view_projection,
            self.shadow_settings,
            self.ambient_occlusion_strength,
            (&batch.uniform, &self.pipeline, bind_group),
            mesh,
            "Sindri cached textured mesh pass",
        );
    }

    /// Releases one persistent mesh, normally from a residency `left` delta.
    pub fn release_cached_mesh(&mut self, cache: CachedMeshId) -> bool {
        self.cached_meshes.release(cache)
    }

    /// Cumulative cache counters and the current number of resident entries.
    pub fn cached_mesh_stats(&self) -> TexturedMeshCacheStats {
        self.cached_meshes.stats()
    }

    /// High-water mark of textured mesh draws in one submission.
    #[must_use]
    pub fn batch_slots(&self) -> usize {
        self.batches.len()
    }
}

fn cube_uniform(
    model: Mat4,
    model_view_projection: Mat4,
    lighting: WorldLighting,
    light_view_projection: Mat4,
    shadows: ShadowSettings,
    ambient_occlusion_strength: f32,
) -> CubeUniform {
    CubeUniform {
        model_view_projection: model_view_projection.to_cols_array_2d(),
        model: model.to_cols_array_2d(),
        ambient: [
            lighting.ambient_color[0],
            lighting.ambient_color[1],
            lighting.ambient_color[2],
            lighting.ambient_intensity,
        ],
        directional_direction: [
            lighting.directional_direction[0],
            lighting.directional_direction[1],
            lighting.directional_direction[2],
            lighting.directional_intensity,
        ],
        directional_color: [
            lighting.directional_color[0],
            lighting.directional_color[1],
            lighting.directional_color[2],
            0.0,
        ],
        light_view_projection: light_view_projection.to_cols_array_2d(),
        shadow: [
            shadows.bias,
            if shadows.enabled && lighting.directional_intensity > 0.0 {
                1.0
            } else {
                0.0
            },
            ambient_occlusion_strength,
            0.0,
        ],
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_mesh_buffers(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: (&wgpu::TextureView, &DepthTarget),
    model: Mat4,
    model_view_projection: Mat4,
    lighting: WorldLighting,
    light_view_projection: Mat4,
    shadows: ShadowSettings,
    ambient_occlusion_strength: f32,
    state: (&wgpu::Buffer, &wgpu::RenderPipeline, &wgpu::BindGroup),
    mesh: &MeshBuffers,
    label: &str,
) {
    let (uniform, pipeline, bind_group) = state;
    queue.write_buffer(
        uniform,
        0,
        bytemuck::bytes_of(&cube_uniform(
            model,
            model_view_projection,
            lighting,
            light_view_projection,
            shadows,
            ambient_occlusion_strength,
        )),
    );
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target.0,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: target.1.view(),
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
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    mesh.draw(&mut pass);
}
