//! Mesh components, and the pass that draws them.

use sindri_core::{SpriteRef, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, TexturedVertex,
};

use crate::{MeshComponent, MeshPrimitive, TextureBindings};

use super::camera::ResolvedCameras;
use super::{SceneExtractError, SceneExtractor, transform_matrix};

impl SceneExtractor {
    pub(super) fn push_meshes(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        for (entity, mesh) in self.components.query::<MeshComponent>(world)? {
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let model = transform_matrix(transform);
            let command = match mesh.primitive {
                MeshPrimitive::Cube => FrameCommand::TexturedCube {
                    model,
                    texture: textures.resolve(&mesh.texture),
                },
                MeshPrimitive::Surface => {
                    let Some(surface) = mesh.surface.as_ref() else {
                        continue;
                    };
                    if surface.vertices.len() != surface.uvs.len()
                        || surface
                            .indices
                            .iter()
                            .any(|index| usize::from(*index) >= surface.vertices.len())
                    {
                        continue;
                    }
                    let reference = SpriteRef::parse(&mesh.texture)?;
                    let (texture, rect) = textures.resolve_sprite(&reference);
                    let vertices = surface
                        .vertices
                        .iter()
                        .zip(&surface.uvs)
                        .map(|(position, uv)| {
                            TexturedVertex::new(
                                *position,
                                [
                                    rect.width().mul_add(uv[0], rect.x()),
                                    rect.height().mul_add(uv[1], rect.y()),
                                ],
                            )
                        })
                        .collect();
                    FrameCommand::TexturedMesh {
                        model,
                        texture,
                        vertices,
                        indices: surface.indices.clone(),
                    }
                }
            };
            frame.push(FramePass::new(
                RenderStage::Opaque3d,
                RenderLayer(mesh.layer),
                FrameCamera {
                    view_projection: cameras
                        .world
                        .ok_or(SceneExtractError::MissingWorldCamera)?
                        .view_projection,
                    position: cameras
                        .world
                        .ok_or(SceneExtractError::MissingWorldCamera)?
                        .view
                        .inverse()
                        .transform_point3(glam::Vec3::ZERO),
                },
                command,
            ));
        }
        Ok(())
    }
}
