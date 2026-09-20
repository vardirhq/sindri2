//! `sindri.mesh`: built-in and authored geometry to draw.

use serde::Deserialize;
use sindri_core::SceneComponent;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MeshPrimitive {
    Cube,
    Surface,
}

/// Explicit textured geometry.
///
/// This is deliberately small: it is enough for generated terrain and imported
/// geometry to cross the scene/render boundary without teaching the renderer
/// what a terrain chunk is. Larger asset-backed meshes can replace the inline
/// representation later without changing that boundary.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SurfaceMesh {
    #[serde(default)]
    pub vertices: Vec<[f32; 3]>,
    #[serde(default)]
    pub uvs: Vec<[f32; 2]>,
    #[serde(default)]
    pub indices: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MeshComponent {
    pub primitive: MeshPrimitive,
    pub texture: String,
    #[serde(default)]
    pub layer: i32,
    #[serde(default)]
    pub surface: Option<SurfaceMesh>,
}

impl SceneComponent for MeshComponent {
    const TYPE_NAME: &'static str = "sindri.mesh";
}
