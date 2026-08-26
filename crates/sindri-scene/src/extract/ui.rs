//! The UI family, laid out against the viewport rather than in the world.
//!
//! Everything here shares one rule: an element's anchor picks a point on the
//! viewport, and the entity's transform is an offset from that point. No
//! authored camera is consulted, so moving, removing, or replacing a gameplay
//! camera cannot move a HUD or take it away.

use glam::{Mat4, Vec2};
use sindri_core::{Transform3D, World};
use sindri_render::{SpriteInstance, TransparentOrder};

use crate::{SpriteAnimations, TextureBindings, UiAnchor, UiImageComponent};

use super::camera::ResolvedCameras;
use super::camera::view::{OverlayExtent, camera_distance, safe_rotation};
use super::sprite::{DrawSpace, RestingSprites, SpriteBatches, drawn_rect};
use super::{SceneExtractError, SceneExtractor};

/// Where an anchored element lands, given the one transform.
///
/// Only X and Y of the transform reach the overlay: a UI element is positioned
/// against the viewport extent, and its Z orders it rather than placing it, so
/// the matrix is flat. A world sprite has no such split — it goes through
/// `transform_matrix`, the same one a mesh does, because it is in the same
/// world a mesh is.
///
/// The whole rotation still reaches the overlay: a quad turned about X or Y
/// foreshortens under the orthographic projection, which is a card flip rather
/// than a mistake. Only the position and the scale are read two-dimensionally,
/// and they are read through the transform's own 2D accessors so that this
/// agrees with every other piece of code that means "in the plane".
pub(super) fn ui_matrix(transform: Transform3D, anchor: UiAnchor, extent: OverlayExtent) -> Mat4 {
    let unit = Vec2::from_array(anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + Vec2::from_array(transform.position_2d());
    let rotation = safe_rotation(transform);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_quat(rotation)
        * Mat4::from_scale(Vec2::from_array(transform.scale_2d()).extend(1.0))
}

impl SceneExtractor {
    pub(super) fn push_ui_images(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
        resting: &RestingSprites,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
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
            let reference = image.reference()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let model = ui_matrix(transform, image.anchor, extent);
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
                    SpriteInstance::new(model, image.tint).with_uv_rect(drawn_rect(
                        entity, &reference, textures, animations, resting,
                    )),
                ));
        }
        Ok(())
    }
}
