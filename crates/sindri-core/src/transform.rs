use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform2D {
    pub position: [f32; 2],
    pub rotation_radians: f32,
    pub scale: [f32; 2],
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            rotation_radians: 0.0,
            scale: [1.0, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform3D {
    pub position: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

