//! Sprites in the world, and the batching every image draw shares.
//!
//! A world sprite, a UI image, and a tilemap's cells are all one quad with one
//! texture, so they fill one set of batches and are flushed together here. What
//! separates them is the space they are drawn in, which is the first part of a
//! batch's key: a batch is one draw, and two spaces cannot share a draw because
//! they differ in both projection and pipeline.

use std::collections::BTreeMap;

use sindri_core::{EntityId, SpriteRef, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, SpriteDepth,
    SpriteInstance, TextureId, TransparentOrder, UvRect,
};

use crate::screen_ui::UiHierarchy;
use crate::{SpriteAnimationComponent, SpriteAnimations, SpriteComponent, TextureBindings};

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::{SceneExtractError, SceneExtractor, transform_matrix};

/// Which projection and pipeline a batch of images is drawn with.
///
/// Declared screen-first because it is the first half of a batch key, and the
/// order batches come out in is the order their passes are pushed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum DrawSpace {
    Screen,
    World,
}

/// Everything that will become a sprite draw, gathered before any of it is
/// ordered.
///
/// Keyed by what decides a batch — the space, which picks the projection and
/// pipeline; the layer, which overrides distance; and the texture, which is what
/// a draw binds. World sprites, UI images, and tilemaps fill the same map, so
/// they share a batch when they share all three.
pub(super) type SpriteBatches =
    BTreeMap<(DrawSpace, i32, TextureId), Vec<(TransparentOrder, SpriteInstance)>>;

/// What each animated entity shows when nothing has ticked it yet.
///
/// A scene just loaded, an entity in the editor outside play mode, a frame
/// captured before the first tick. Only images that authored no part of their
/// own take it: a sheet drawn whole is every frame at once, which is a picture
/// nobody meant, while an authored part is how a scene picks a rest pose other
/// than the clip's first frame.
pub(super) type RestingSprites = BTreeMap<EntityId, String>;

/// The runtime state a frame is drawn against, resolved once by the caller.
///
/// Separate from [`Drawing`] because it crosses the boundary between extraction
/// and its caller, while `Drawing` only exists inside it.
#[derive(Clone, Copy)]
pub(super) struct Shared<'a> {
    pub(super) textures: &'a TextureBindings,
    pub(super) animations: &'a SpriteAnimations,
    pub(super) effects: Option<&'a crate::Effects2d>,
    pub(super) hierarchy: &'a UiHierarchy,
}

/// What every drawn image needs, other than the world it came from.
///
/// A bundle rather than six more parameters on each pusher: they all take the
/// same set, and a list this long is one where a caller can transpose two
/// references of the same type without the compiler minding.
#[derive(Clone, Copy)]
pub(super) struct Drawing<'a> {
    pub(super) cameras: &'a ResolvedCameras,
    pub(super) textures: &'a TextureBindings,
    pub(super) animations: &'a SpriteAnimations,
    pub(super) resting: &'a RestingSprites,
    /// Where every UI element ends up once its parents have had their say.
    pub(super) hierarchy: &'a UiHierarchy,
}

impl SceneExtractor {
    /// Draws every image the world holds: world sprites, tilemaps, and the UI.
    pub(super) fn push_images(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        shared: Shared<'_>,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let Shared {
            textures,
            animations,
            effects,
            hierarchy,
        } = shared;
        let mut batches: SpriteBatches = BTreeMap::new();
        let resting = self.resting_sprites(world)?;
        let drawing = Drawing {
            cameras,
            textures,
            animations,
            resting: &resting,
            hierarchy,
        };
        self.push_world_sprites(world, drawing, &mut batches)?;
        self.push_ui_images(world, drawing, &mut batches)?;
        self.push_tilemaps(world, cameras, textures, &mut batches)?;
        // Into the same batches as everything else, so a burst merges with what
        // is already on its layer instead of costing a draw call.
        Self::push_effects(effects, cameras, textures, &mut batches)?;
        Self::flush_batches(batches, cameras, frame)
    }

    /// A broken clip falls through to the image's own reference rather than
    /// failing the frame, for the reason a broken clip does not fail loading:
    /// the editor is where it gets fixed, and it has to draw to be fixed there.
    fn resting_sprites(&self, world: &World) -> Result<RestingSprites, SceneExtractError> {
        let mut resting = RestingSprites::new();
        for (entity, animation) in self.components.query::<SpriteAnimationComponent>(world)? {
            if let Some(sprite) = animation.resting_sprite().ok().flatten() {
                resting.insert(entity, sprite.to_owned());
            }
        }
        Ok(resting)
    }

    fn push_world_sprites(
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
            ..
        } = drawing;
        for (entity, sprite) in self.components.query::<SpriteComponent>(world)? {
            let reference = sprite.reference()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
            let model = transform_matrix(transform);
            let order = TransparentOrder::new(
                sprite.layer,
                camera_distance(camera.view, model.w_axis.truncate()),
                entity.index(),
            )?;
            batches
                .entry((
                    DrawSpace::World,
                    sprite.layer,
                    textures.resolve(reference.texture()),
                ))
                .or_default()
                .push((
                    order,
                    SpriteInstance::new(model, sprite.tint).with_uv_rect(drawn_rect(
                        entity, &reference, textures, animations, resting,
                    )),
                ));
        }
        Ok(())
    }

    fn flush_batches(
        batches: SpriteBatches,
        cameras: &ResolvedCameras,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        for ((space, layer, texture), mut sprites) in batches {
            sprites.sort_by_key(|(order, _)| *order);
            let (stage, camera, depth) = match space {
                DrawSpace::Screen => (
                    RenderStage::Overlay,
                    cameras
                        .overlay
                        .expect("every resolved view includes screen-space projection"),
                    SpriteDepth::Ignore,
                ),
                DrawSpace::World => (
                    RenderStage::Transparent2d,
                    cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                    SpriteDepth::Test,
                ),
            };
            frame.push(FramePass::new(
                stage,
                RenderLayer(layer),
                FrameCamera {
                    view_projection: camera.view_projection,
                },
                FrameCommand::SpriteBatch {
                    texture,
                    depth,
                    instances: sprites.into_iter().map(|(_, sprite)| sprite).collect(),
                },
            ));
        }
        Ok(())
    }
}

/// Which part of its sheet one image draws this frame.
///
/// What the animation says, then what a clip would show at rest, then what the
/// image itself names. A sheet drawn whole is every sprite at once, which is
/// why an animated image that names no part of its own falls back to its clip's
/// first frame rather than to everything.
pub(super) fn drawn_rect(
    entity: EntityId,
    reference: &SpriteRef,
    textures: &TextureBindings,
    animations: &SpriteAnimations,
    resting: &RestingSprites,
) -> UvRect {
    match animations.sprite(entity) {
        // A playing clip decides, and if what it names does not resolve the
        // answer is the whole image rather than some other frame: falling back
        // to frame zero would draw a plausible picture of the wrong moment,
        // which is the failure that hides.
        Some(name) => textures
            .sheet_sprite(reference.texture(), name)
            .unwrap_or(UvRect::FULL),
        None => reference
            .sprite()
            .is_none()
            .then(|| {
                resting
                    .get(&entity)
                    .and_then(|name| textures.sheet_sprite(reference.texture(), name))
            })
            .flatten()
            .or_else(|| textures.sprite_rect(reference))
            .unwrap_or(UvRect::FULL),
    }
}
