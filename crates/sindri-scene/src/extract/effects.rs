//! Drawing the fleck pool, which is not a scan of the world.
//!
//! Every fleck goes through the same batching as a world sprite, so a burst
//! merges with whatever else is on its layer rather than costing a draw call of
//! its own. What it skips is the part the measurement blamed: there is no
//! per-fleck component to look up, no payload to deserialize, and no entity to
//! find — the pool is already the array this needs.

use glam::{Mat4, Vec3};
use sindri_render::{SpriteInstance, TransparentOrder};

use crate::Effects2d;

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::sprite::{DrawSpace, SpriteBatches};
use super::{SceneExtractError, SceneExtractor};
use crate::TextureBindings;

impl SceneExtractor {
    pub(super) fn push_effects(
        effects: Option<&Effects2d>,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
        let Some(effects) = effects.filter(|pool| pool.live() > 0) else {
            return Ok(());
        };
        let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
        for (index, fleck) in effects.flecks().iter().enumerate() {
            let Some(texture) = effects.texture(fleck.texture) else {
                continue;
            };
            let position = Vec3::new(fleck.position[0], fleck.position[1], 0.0);
            let model = Mat4::from_translation(position)
                * Mat4::from_scale(Vec3::new(fleck.size, fleck.size, 1.0));
            // Flecks share a layer and are drawn back to front by distance like
            // any other transparent thing. The index breaks ties, which keeps a
            // frame deterministic without anyone sorting the pool itself — the
            // pool's order is a swap-remove artefact and means nothing.
            let order = TransparentOrder::new(
                fleck.layer,
                camera_distance(camera.view, position),
                u32::try_from(index).unwrap_or(u32::MAX),
            )?;
            batches
                .entry((DrawSpace::World, fleck.layer, textures.resolve(texture)))
                .or_default()
                .push((order, SpriteInstance::new(model, fleck.drawn_tint())));
        }
        Ok(())
    }
}
