//! `sindri.ui.text`, laid out into the frame.

use std::collections::BTreeMap;

use sindri_core::World;
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, TextInstance,
    Viewport,
};

use crate::UiTextComponent;

use super::camera::{OverlayPlacement, OverlayView, ResolvedCameras};
use super::{SceneExtractError, SceneExtractor};

impl SceneExtractor {
    pub(super) fn push_text(
        &self,
        world: &World,
        viewport: Viewport,
        cameras: &ResolvedCameras,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let texts = self.components.query::<UiTextComponent>(world)?;
        if texts.is_empty() {
            return Ok(());
        }
        // Screen-space projection is viewport-owned. The Option shape remains
        // shared with world-camera resolution, but every resolver constructs
        // this value rather than looking for a scene camera entity.
        let overlay = cameras
            .overlay
            .expect("every resolved view includes screen-space projection");
        let extent = cameras
            .overlay_extent
            .expect("every resolved view includes screen-space extent");
        let placement = OverlayPlacement::new(extent);
        let view = OverlayView {
            view_projection: overlay.view_projection,
            framed_half_height: overlay.framed_half_height,
        };
        let width = f32::from(u16::try_from(viewport.width)?);
        let height = f32::from(u16::try_from(viewport.height)?);
        let mut layers: BTreeMap<i32, Vec<TextInstance>> = BTreeMap::new();

        for (entity, text) in texts {
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let fraction = placement.text_origin(view, transform, text.anchor);
            let position = [fraction[0] * width, fraction[1] * height];
            layers
                .entry(text.layer)
                .or_default()
                .push(TextInstance::new(
                    text.resolved(),
                    text.font,
                    position,
                    text.font_size,
                    text.line_height,
                    text.color,
                )?);
        }

        for (layer, instances) in layers {
            frame.push(FramePass::new(
                RenderStage::Overlay,
                RenderLayer(layer),
                FrameCamera {
                    view_projection: overlay.view_projection,
                },
                FrameCommand::Text { instances },
            ));
        }
        Ok(())
    }
}
