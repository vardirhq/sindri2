//! `sindri.ui.shape`, laid out into the frame.
//!
//! Batched by blend and layer rather than by kind: the kind is per instance and
//! costs a comparison in the shader, while the blend is baked into the pipeline.
//! So a ring, a grid and a hexagon on one layer draw together, and paint and
//! light do not.

use std::collections::BTreeMap;

use sindri_core::{EntityId, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, ShapeBlend,
    ShapeInstance,
};

use crate::screen_ui::UiHierarchy;
use crate::{ShapeComponent, UiShapeComponent, UiShapeKind};

use super::camera::ResolvedCameras;
use super::ui::ui_matrix;
use super::{SceneExtractError, SceneExtractor, transform_matrix};

/// Optional authored vertices carried beside the typed shape fields.
///
/// `ShapeGeometry` deliberately remains the compact common shape schema. Custom
/// polygon points are a bounded extension used only by polygon rendering, so an
/// older scene with no `points` remains byte-for-byte the same and every other
/// kind keeps the slot it already used for corner radius.
fn authored_points(world: &World, entity: EntityId, component: &str) -> Vec<[f32; 2]> {
    let Some(points) = world
        .get(entity)
        .and_then(|data| data.components.get(component))
        .and_then(|payload| payload.get("points"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    points
        .iter()
        .take(sindri_render::MAX_POLYGON_POINTS)
        .filter_map(|point| {
            let values = point.as_array()?;
            let [x, y] = values.as_slice() else {
                return None;
            };
            #[allow(clippy::cast_possible_truncation)]
            Some([x.as_f64()? as f32, y.as_f64()? as f32])
        })
        .collect()
}

fn shape_instance(
    world: &World,
    entity: EntityId,
    component: &str,
    geometry: &crate::ShapeGeometry,
    model: glam::Mat4,
) -> ShapeInstance {
    let instance = geometry.instance(model);
    if geometry.kind != UiShapeKind::Polygon {
        return instance;
    }
    let points = authored_points(world, entity, component);
    instance.with_polygon_points(&points)
}

impl SceneExtractor {
    pub(super) fn push_shapes(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        hierarchy: &UiHierarchy,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let shapes = self.components.query::<UiShapeComponent>(world)?;
        if shapes.is_empty() {
            return Ok(());
        }
        let extent = cameras
            .overlay_extent
            .expect("every resolved view includes screen-space extent");
        let overlay = cameras
            .overlay
            .expect("every resolved view includes screen-space projection");
        let camera = FrameCamera {
            view_projection: overlay.view_projection,
        };

        // Ordered so the frame's passes come out layer by layer, and within a
        // layer with paint before light: light added under paint would be
        // covered by it, which is the one order that makes a glow invisible.
        let mut batches: BTreeMap<(i32, bool), Vec<ShapeInstance>> = BTreeMap::new();
        for (entity, shape) in shapes {
            if !world.is_active(entity) {
                continue;
            }
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let placed = hierarchy.placement_or(entity, shape.anchor);
            let model = ui_matrix(placed, transform, extent);
            batches
                .entry((shape.layer, shape.geometry.blend() == ShapeBlend::Add))
                .or_default()
                .push(shape_instance(
                    world,
                    entity,
                    "sindri.ui.shape",
                    &shape.geometry,
                    model,
                ));
        }

        for ((layer, additive), instances) in batches {
            frame.push(FramePass::new(
                RenderStage::Overlay,
                RenderLayer(layer),
                camera,
                FrameCommand::Shapes {
                    blend: if additive {
                        ShapeBlend::Add
                    } else {
                        ShapeBlend::Over
                    },
                    instances,
                },
            ));
        }
        Ok(())
    }
}

impl SceneExtractor {
    /// `sindri.shape`, drawn in the world rather than on the overlay.
    ///
    /// Through the world camera and against the depth buffer, so a shape is
    /// somewhere in the scene: hidden by what is in front of it, and moving
    /// when the view does. Everything else about it is the overlay shape's
    /// story — the same geometry, the same batching by blend and layer.
    pub(super) fn push_world_shapes(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let shapes = self.components.query::<ShapeComponent>(world)?;
        if shapes.is_empty() {
            return Ok(());
        }
        let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;

        let mut batches: BTreeMap<(i32, bool), Vec<ShapeInstance>> = BTreeMap::new();
        for (entity, shape) in shapes {
            if !world.is_active(entity) {
                continue;
            }
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            batches
                .entry((shape.layer, shape.geometry.blend() == ShapeBlend::Add))
                .or_default()
                .push(shape_instance(
                    world,
                    entity,
                    "sindri.shape",
                    &shape.geometry,
                    transform_matrix(transform),
                ));
        }

        for ((layer, additive), instances) in batches {
            frame.push(FramePass::new(
                RenderStage::Transparent2d,
                RenderLayer(layer),
                FrameCamera {
                    view_projection: camera.view_projection,
                },
                FrameCommand::Shapes {
                    blend: if additive {
                        ShapeBlend::Add
                    } else {
                        ShapeBlend::Over
                    },
                    instances,
                },
            ));
        }
        Ok(())
    }
}
