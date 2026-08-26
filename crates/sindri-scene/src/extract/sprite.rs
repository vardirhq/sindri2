//! Sprites: which texture, which frame, and where in the world or on
//! the screen.

use std::collections::BTreeMap;

use glam::{Mat4, Vec2};
use sindri_core::{EntityId, Transform3D, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage, SpriteDepth,
    SpriteInstance, TextureId, TransparentOrder, UvRect,
};

use crate::{
    SpriteAnchor, SpriteAnimationComponent, SpriteAnimations, SpriteComponent, SpriteSpace,
    TextureBindings,
};

use super::camera::ResolvedCameras;
use super::camera::view::{OverlayExtent, camera_distance, safe_rotation};
use super::{SceneExtractError, SceneExtractor, transform_matrix};

/// Where a screen-anchored sprite lands, given the one transform.
///
/// Only X and Y of the transform reach the overlay: a screen-space sprite is
/// positioned against the viewport extent, and its Z orders it rather than
/// placing it, so the matrix is flat. A world-space sprite has no such split —
/// it goes through `transform_matrix`, the same one a mesh does, because it is
/// in the same world a mesh is.
///
/// The whole rotation still reaches the overlay: a quad turned about X or Y
/// foreshortens under the orthographic projection, which is a card flip rather
/// than a mistake. Only the position and the scale are read two-dimensionally,
/// and they are read through the transform's own 2D accessors so that this
/// agrees with every other piece of code that means "in the plane".
pub(super) fn screen_sprite_matrix(
    transform: Transform3D,
    anchor: SpriteAnchor,
    extent: OverlayExtent,
) -> Mat4 {
    let unit = Vec2::from_array(anchor.unit_offset());
    let origin = extent.center + unit * extent.half_extent;
    let position = origin + Vec2::from_array(transform.position_2d());
    let rotation = safe_rotation(transform);
    Mat4::from_translation(position.extend(0.0))
        * Mat4::from_quat(rotation)
        * Mat4::from_scale(Vec2::from_array(transform.scale_2d()).extend(1.0))
}

/// Everything that will become a sprite draw, gathered before any of it is
/// ordered.
///
/// Keyed by what decides a batch — the space, which picks the projection and
/// pipeline; the layer, which overrides distance; and the texture, which is what
/// a draw binds. Tilemaps and loose sprites fill the same map, so they share a
/// batch when they share all three.
pub(super) type SpriteBatches =
    BTreeMap<(SpriteSpace, i32, TextureId), Vec<(TransparentOrder, SpriteInstance)>>;

impl SceneExtractor {
    pub(super) fn push_sprites(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        animations: &SpriteAnimations,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        // Sprites batch per space, layer, and texture, back to front within a
        // batch, with a stable tie-break. A batch is one draw, so instances
        // share one only when they share the texture it binds — and the space,
        // which decides both the projection and the pipeline.
        let mut batches: SpriteBatches = BTreeMap::new();
        // What an animated sprite shows when `animations` has not reached it —
        // a scene just loaded, an entity in the editor outside play mode, a
        // frame captured before the first tick. Only sprites that authored no
        // rect of their own take it: a sheet drawn whole is every frame at
        // once, which is a picture nobody meant, while an authored rect is how
        // a scene picks a rest pose other than the clip's first frame.
        //
        // A broken clip falls through to the sprite's rect rather than failing
        // the frame, for the reason a broken clip does not fail loading: the
        // editor is where it gets fixed, and it has to draw to be fixed there.
        let mut resting: BTreeMap<EntityId, String> = BTreeMap::new();
        for (entity, animation) in self.components.query::<SpriteAnimationComponent>(world)? {
            if let Some(sprite) = animation.resting_sprite().ok().flatten() {
                resting.insert(entity, sprite.to_owned());
            }
        }
        for (entity, sprite) in self.components.query::<SpriteComponent>(world)? {
            let reference = sprite.reference()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let (model, camera) = match sprite.screen_anchor() {
                Some(anchor) => {
                    let extent = cameras
                        .overlay_extent
                        .expect("every resolved view includes screen-space extent");
                    (
                        screen_sprite_matrix(transform, anchor, extent),
                        cameras
                            .overlay
                            .expect("every resolved view includes screen-space projection"),
                    )
                }
                None => (
                    transform_matrix(transform),
                    cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                ),
            };
            // A screen sprite is drawn flat against the overlay, but its Z
            // still says how far back in the stack it sits, so the distance is
            // measured against the authored Z rather than the flattened one. A
            // world sprite's two Zs are the same number.
            let position = model.w_axis.truncate().with_z(transform.position[2]);
            let order = TransparentOrder::new(
                sprite.layer,
                camera_distance(camera.view, position),
                entity.index(),
            )?;
            batches
                .entry((
                    sprite.space,
                    sprite.layer,
                    textures.resolve(reference.texture()),
                ))
                .or_default()
                .push((
                    order,
                    SpriteInstance::new(model, sprite.tint).with_uv_rect(
                        // What the animation says, then what a clip would show
                        // at rest, then what the sprite itself names. A sheet
                        // drawn whole is every sprite at once, which is why an
                        // animated sprite that names no part of its own falls
                        // back to its clip's first frame rather than to
                        // everything.
                        match animations.sprite(entity) {
                            // A playing clip decides, and if what it names does
                            // not resolve the answer is the whole image rather
                            // than some other frame: falling back to frame zero
                            // would draw a plausible picture of the wrong
                            // moment, which is the failure that hides.
                            Some(name) => textures
                                .sheet_sprite(reference.texture(), name)
                                .unwrap_or(UvRect::FULL),
                            // Nothing has ticked it yet. A sprite that names no
                            // part of its own shows its clip's first frame,
                            // because a sheet drawn whole is every frame at once
                            // and that is a picture nobody meant.
                            None => reference
                                .sprite()
                                .is_none()
                                .then(|| {
                                    resting.get(&entity).and_then(|name| {
                                        textures.sheet_sprite(reference.texture(), name)
                                    })
                                })
                                .flatten()
                                .or_else(|| textures.sprite_rect(&reference))
                                .unwrap_or(UvRect::FULL),
                        },
                    ),
                ));
        }

        self.push_tilemaps(world, cameras, textures, &mut batches)?;

        for ((space, layer, texture), mut sprites) in batches {
            sprites.sort_by_key(|(order, _)| *order);
            let (stage, camera, depth) = match space {
                SpriteSpace::Screen => (
                    RenderStage::Overlay,
                    cameras
                        .overlay
                        .expect("every resolved view includes screen-space projection"),
                    SpriteDepth::Ignore,
                ),
                SpriteSpace::World => (
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
