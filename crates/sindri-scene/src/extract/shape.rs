//! `sindri.ui.shape`, laid out into the frame.
//!
//! Batched by blend and layer rather than by kind: the kind is per instance and
//! costs a comparison in the shader, while the blend is baked into the pipeline.
//! So a ring, a grid and a hexagon on one layer draw together, and paint and
//! light do not.

use std::collections::BTreeMap;

use sindri_core::World;
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, ShapeBlend,
    ShapeInstance,
};

use crate::UiShapeComponent;
use crate::screen_ui::UiHierarchy;

use super::camera::ResolvedCameras;
use super::ui::ui_matrix;
use super::{SceneExtractError, SceneExtractor};

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
                .entry((shape.layer, shape.blend() == ShapeBlend::Add))
                .or_default()
                .push(shape.instance(model));
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
