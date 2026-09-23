use glam::{Mat4, Vec3};

use crate::{FogSettings, MeshSurface, ShadowSettings, WorldLighting};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct CubeUniform {
    pub model_view_projection: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub ambient: [f32; 4],
    pub directional_direction: [f32; 4],
    pub directional_color: [f32; 4],
    pub light_view_projection: [[f32; 4]; 4],
    pub shadow: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_params: [f32; 4],
    pub camera_position: [f32; 4],
    /// `uv_offset.xy`, glow, alpha cutoff.
    pub surface: [f32; 4],
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn cube_uniform(
    model: Mat4,
    model_view_projection: Mat4,
    lighting: WorldLighting,
    light_view_projection: Mat4,
    shadows: ShadowSettings,
    ambient_occlusion_strength: f32,
    fog: FogSettings,
    camera_position: Vec3,
    surface: MeshSurface,
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
        fog_color: [
            fog.color[0],
            fog.color[1],
            fog.color[2],
            if fog.enabled { 1.0 } else { 0.0 },
        ],
        fog_params: [fog.start, fog.distance, fog.density, fog.height],
        camera_position: [
            camera_position.x,
            camera_position.y,
            camera_position.z,
            fog.height_falloff,
        ],
        surface: [
            surface.uv_offset[0],
            surface.uv_offset[1],
            surface.glow,
            surface.alpha_cutoff,
        ],
    }
}
