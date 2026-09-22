/// Renderer-neutral atmosphere applied to opaque world geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogSettings {
    pub enabled: bool,
    pub color: [f32; 3],
    /// World-space distance where fog begins to accumulate.
    pub start: f32,
    /// Distance over which the linear distance term reaches full fog.
    pub distance: f32,
    /// Additional exponential density. Zero leaves only distance fog.
    pub density: f32,
    /// Height below which extra fog accumulates.
    pub height: f32,
    /// Strength of the low-altitude contribution.
    pub height_falloff: f32,
}

impl Default for FogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [0.55, 0.65, 0.75],
            start: 24.0,
            distance: 64.0,
            density: 0.0,
            height: 0.0,
            height_falloff: 0.0,
        }
    }
}
