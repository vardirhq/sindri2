//! Renderer-owned world lighting shared by every textured 3D draw.

/// Ambient plus one directional world light.
///
/// Direction is the direction the light travels, from the source toward the
/// world. The default preserves the historical unlit appearance: white ambient
/// at full strength and no directional contribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldLighting {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub directional_direction: [f32; 3],
    pub directional_color: [f32; 3],
    pub directional_intensity: f32,
}

impl Default for WorldLighting {
    fn default() -> Self {
        Self {
            ambient_color: [1.0; 3],
            ambient_intensity: 1.0,
            directional_direction: [0.0, -1.0, 0.0],
            directional_color: [1.0; 3],
            directional_intensity: 0.0,
        }
    }
}
