//! The UI family, laid out against the viewport rather than in the world.
//!
//! Everything here shares one rule: an element's anchor picks a point on the
//! viewport, and the entity's transform is an offset from that point. No
//! authored camera is consulted, so moving, removing, or replacing a gameplay
//! camera cannot move a HUD or take it away.

use glam::{Mat4, Vec2};
use sindri_core::{Transform3D, World};
use sindri_render::{SpriteInstance, TransparentOrder};

use crate::UiImageComponent;
use crate::screen_ui::UiPlaced;

use super::camera::view::{OverlayExtent, camera_distance};
use super::sprite::{DrawSpace, Drawing, SpriteBatches, drawn_rect};
use super::{SceneExtractError, SceneExtractor};

/// Where an already-placed element lands on this viewport.
///
/// `placed` is the element's offset and turn with its ancestors folded in —
/// [`UiHierarchy`] resolves that once for everything, so a label on a card and
/// the card agree about where the card is. What remains here is the part that
/// depends on the viewport: which point the anchor names, and the element's own
/// size.
///
/// Only X and Y reach the overlay: a UI element is positioned against the
/// viewport extent, and its Z orders it rather than placing it, so the matrix is
/// flat. A world sprite has no such split — it goes through `transform_matrix`,
/// the same one a mesh does, because it is in the same world a mesh is.
///
/// The size is the element's own and is never inherited, because `scale` here is
/// a size in overlay units rather than a multiplier on a coordinate space. See
/// [`UiHierarchy`] for why that is the whole difference between this and a
/// `RectTransform`.
pub(super) fn ui_matrix(placed: UiPlaced, transform: Transform3D, extent: OverlayExtent) -> Mat4 {
    let unit = Vec2::from_array(placed.anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + placed.offset;
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_quat(placed.rotation)
        * Mat4::from_scale(Vec2::from_array(transform.scale_2d()).extend(1.0))
}

impl SceneExtractor {
    pub(super) fn push_ui_images(
        &self,
        world: &World,
        drawing: Drawing<'_>,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
        let Drawing {
            cameras,
            textures,
            animations,
            resting,
            hierarchy,
        } = drawing;
        let images = self.components.query::<UiImageComponent>(world)?;
        if images.is_empty() {
            return Ok(());
        }
        let extent = cameras
            .overlay_extent
            .expect("every resolved view includes screen-space extent");
        let camera = cameras
            .overlay
            .expect("every resolved view includes screen-space projection");
        for (entity, image) in images {
            // A bar filled to nothing draws nothing. Skipping here rather than
            // pushing a zero-area quad keeps the empty case out of the renderer
            // entirely, which is where a degenerate rect would be a problem.
            let (fill_offset, fill_scale) = image.fill.sub_rect();
            let Some(uv) = drawn_rect(entity, &image.reference()?, textures, animations, resting)
                .part(fill_offset, fill_scale)
            else {
                continue;
            };
            let reference = image.reference()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            // The fill shrinks the quad within the element's authored rect, so
            // the drawn part keeps the edge it fills from instead of the bar
            // closing towards its middle.
            let placed = hierarchy.placement_or(entity, image.anchor);
            let model = ui_matrix(placed, transform, extent)
                * Mat4::from_translation(glam::Vec3::new(fill_offset[0], fill_offset[1], 0.0))
                * Mat4::from_scale(glam::Vec3::new(fill_scale[0], fill_scale[1], 1.0));
            // Drawn flat against the overlay, but the Z still says how far back
            // in the stack it sits, so the distance is measured against the
            // authored Z rather than the flattened one.
            let position = model.w_axis.truncate().with_z(transform.position[2]);
            let order = TransparentOrder::new(
                image.layer,
                camera_distance(camera.view, position),
                entity.index(),
            )?;
            batches
                .entry((
                    DrawSpace::Screen,
                    image.layer,
                    textures.resolve(reference.texture()),
                ))
                .or_default()
                .push((
                    order,
                    SpriteInstance::new(model, image.tint).with_uv_rect(uv),
                ));
        }
        Ok(())
    }
}
