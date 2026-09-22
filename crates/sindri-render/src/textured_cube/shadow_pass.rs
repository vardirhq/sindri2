use glam::Mat4;

use super::{MeshBuffers, TexturedCubeRenderer, TexturedVertex};
use crate::shadow::light_view_projection;

impl TexturedCubeRenderer {
    pub(crate) fn begin_shadow_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        camera_view_projection: Mat4,
    ) {
        self.shadow_view_projection =
            light_view_projection(camera_view_projection, self.lighting, self.shadow_settings);
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sindri directional shadow clear"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.shadow_map.view(),
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
    }

    pub(crate) fn encode_shadow_cube(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        model: Mat4,
    ) {
        let slot = self.reserve(device);
        encode_shadow_mesh(
            queue,
            encoder,
            self.shadow_map.view(),
            self.shadow_view_projection * model,
            &self.batches[slot].shadow_uniform,
            &self.batches[slot].shadow_bind_group,
            &self.shadow_pipeline,
            &self.mesh,
        );
    }

    pub(crate) fn encode_shadow_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        model: Mat4,
        vertices: &[TexturedVertex],
        indices: &[u16],
    ) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        let mesh = MeshBuffers::new(device, "Sindri shadow transient mesh", vertices, indices);
        let slot = self.reserve(device);
        encode_shadow_mesh(
            queue,
            encoder,
            self.shadow_map.view(),
            self.shadow_view_projection * model,
            &self.batches[slot].shadow_uniform,
            &self.batches[slot].shadow_bind_group,
            &self.shadow_pipeline,
            &mesh,
        );
    }

    pub(crate) fn encode_shadow_cached_mesh(
        &mut self,
        context: super::DrawContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        model: Mat4,
        request: super::CachedMeshRequest<'_>,
    ) {
        let slot = self.reserve(context.device);
        let Some(mesh) = self.cached_meshes.resolve(
            context.device,
            request.id,
            request.revision,
            request.replacement,
        ) else {
            return;
        };
        encode_shadow_mesh(
            context.queue,
            encoder,
            self.shadow_map.view(),
            self.shadow_view_projection * model,
            &self.batches[slot].shadow_uniform,
            &self.batches[slot].shadow_bind_group,
            &self.shadow_pipeline,
            mesh,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_shadow_mesh(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    shadow_view: &wgpu::TextureView,
    light_model_view_projection: Mat4,
    uniform: &wgpu::Buffer,
    bind_group: &wgpu::BindGroup,
    pipeline: &wgpu::RenderPipeline,
    mesh: &MeshBuffers,
) {
    queue.write_buffer(
        uniform,
        0,
        bytemuck::cast_slice(&light_model_view_projection.to_cols_array()),
    );
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Sindri directional shadow pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: shadow_view,
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
