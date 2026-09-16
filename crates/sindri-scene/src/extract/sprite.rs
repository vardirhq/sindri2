//! Sprites in the world, and the batching every image draw shares.
//!
//! A world sprite, a UI image, and a tilemap's cells are all one quad with one
//! texture, so they fill one ordered queue and are flushed together here. What
//! separates them is the space they are drawn in: two spaces cannot share a
//! draw because they differ in both projection and pipeline.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use sindri_core::{EntityId, SpriteRef, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, SpriteDepth,
    SpriteInstance, TextureId, TransparentOrder, UvRect,
};

use crate::TileSetBindings;
use crate::screen_ui::UiHierarchy;
use crate::{SpriteAnimationComponent, SpriteAnimations, SpriteComponent, TextureBindings};

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::{SceneExtractError, SceneExtractor, transform_matrix};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum DrawSpace {
    Screen,
    World,
}

pub(super) struct SpriteDraw {
    pub(super) space: DrawSpace,
    pub(super) texture: TextureId,
    pub(super) order: TransparentOrder,
    pub(super) sprite: SpriteInstance,
}

pub(super) type SpriteBatches = Vec<SpriteDraw>;
pub(super) type RestingSprites = BTreeMap<EntityId, String>;

#[derive(Clone, Copy)]
pub(super) struct Shared<'a> {
    pub(super) textures: &'a TextureBindings,
    pub(super) animations: &'a SpriteAnimations,
    pub(super) effects: Option<&'a crate::Effects2d>,
    pub(super) hierarchy: &'a UiHierarchy,
    pub(super) tile_sets: Option<&'a TileSetBindings>,
}

#[derive(Clone, Copy)]
pub(super) struct Drawing<'a> {
    pub(super) cameras: &'a ResolvedCameras,
    pub(super) textures: &'a TextureBindings,
    pub(super) animations: &'a SpriteAnimations,
    pub(super) resting: &'a RestingSprites,
    pub(super) hierarchy: &'a UiHierarchy,
}

impl SceneExtractor {
    pub(super) fn push_images(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        shared: Shared<'_>,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let Shared { textures, animations, effects, hierarchy, tile_sets } = shared;
        let mut batches = SpriteBatches::new();
        let resting = self.resting_sprites(world)?;
        let drawing = Drawing { cameras, textures, animations, resting: &resting, hierarchy };
        self.push_world_sprites(world, drawing, &mut batches)?;
        self.push_ui_images(world, drawing, &mut batches)?;
        self.push_tilemaps(world, cameras, textures, &mut batches)?;
        self.push_tile_volumes(world, cameras, textures, tile_sets, &mut batches)?;
        Self::push_effects(effects, cameras, textures, &mut batches)?;
        Self::flush_batches(batches, cameras, frame)
    }

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
        let Drawing { cameras, textures, animations, resting, .. } = drawing;
        for (entity, sprite) in self.components.query::<SpriteComponent>(world)? {
            let reference = sprite.reference()?;
            let transform = world.get(entity).and_then(|data| data.transform_3d).unwrap_or_default();
            let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
            let [offset_x, offset_y] = textures.sprite_anchor(&reference).offset();
            let model = transform_matrix(transform)
                * Mat4::from_translation(Vec3::new(offset_x, offset_y, 0.0));
            let order = TransparentOrder::new(
                sprite.layer,
                camera_distance(camera.view, model.w_axis.truncate()),
                entity.index(),
            )?;
            batches.push(SpriteDraw {
                space: DrawSpace::World,
                texture: textures.resolve(reference.texture()),
                order,
                sprite: SpriteInstance::new(model, sprite.tint)
                    .with_color_transform(
                        sprite.color_transform.multiply,
                        sprite.color_transform.offset,
                    )
                    .with_uv_rect(drawn_rect(entity, &reference, textures, animations, resting)),
            });
        }
        Ok(())
    }

    fn flush_batches(
        mut batches: SpriteBatches,
        cameras: &ResolvedCameras,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        batches.sort_by_key(|draw| (draw.space, draw.order));
        let mut draws = batches.into_iter().peekable();
        while let Some(first) = draws.next() {
            let space = first.space;
            let layer = first.order.layer();
            let texture = first.texture;
            let mut instances = vec![first.sprite];
            while draws.peek().is_some_and(|draw| {
                draw.space == space && draw.order.layer() == layer && draw.texture == texture
            }) {
                instances.push(draws.next().expect("the adjacent sprite was just inspected").sprite);
            }
            let (stage, camera, depth) = match space {
                DrawSpace::Screen => (
                    RenderStage::Overlay,
                    cameras.overlay.expect("every resolved view includes screen-space projection"),
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
                FrameCamera { view_projection: camera.view_projection },
                FrameCommand::SpriteBatch { texture, depth, instances },
            ));
        }
        Ok(())
    }
}

pub(super) fn drawn_rect(
    entity: EntityId,
    reference: &SpriteRef,
    textures: &TextureBindings,
    animations: &SpriteAnimations,
    resting: &RestingSprites,
) -> UvRect {
    match animations.sprite(entity) {
        Some(name) => textures.sheet_sprite(reference.texture(), name).unwrap_or(UvRect::FULL),
        None => reference
            .sprite()
            .is_none()
            .then(|| resting.get(&entity).and_then(|name| textures.sheet_sprite(reference.texture(), name)))
            .flatten()
            .or_else(|| textures.sprite_rect(reference))
            .unwrap_or(UvRect::FULL),
    }
}
