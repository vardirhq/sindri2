//! `sindri.ui.text`, laid out into the frame.

use std::collections::BTreeMap;

use sindri_core::World;
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, TextError,
    TextInstance, Viewport,
};

/// How tall the overlay is in its own units.
///
/// Two, centred on the origin, whatever the viewport is — the same two a screen
/// element's transform is placed in. A font size is a share of this.
const OVERLAY_HEIGHT: f32 = 2.0;

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
            // Taller than the whole screen is not a size anyone wants; it is a
            // pixel count typed where a share of the screen goes, which is the
            // mistake this unit invites and the one worth naming.
            if text.font_size > OVERLAY_HEIGHT {
                return Err(SceneExtractError::Text(TextError::InvalidFontSize(
                    text.font_size,
                )));
            }
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            // Overlay units into pixels, which is the one conversion between
            // what a scene says and what a renderer draws.
            let metrics = text.pixel_metrics(height);
            let fraction = placement.text_origin(view, transform, text.anchor);
            let position = [fraction[0] * width, fraction[1] * height];
            layers
                .entry(text.layer)
                .or_default()
                .push(TextInstance::new(
                    text.resolved(),
                    text.font,
                    position,
                    metrics[0],
                    metrics[1],
                    text.color,
                    text.anchor.text_align(),
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
