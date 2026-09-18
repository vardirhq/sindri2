//! Stackable tile volumes, resolved into their visible baked faces.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use sindri_core::{
    EntityId, SceneComponent, SpriteRef, TileDefinition, TileFace, TileSetDocument, World,
};
use sindri_grid::GridCoord3;
use sindri_render::{SpriteInstance, TextureId, TransparentOrder, UvRect};

use crate::{
    TextureBindings, TileGridComponent, TileGridError, TileSetBindings, TileVolumeComponent,
    TileVolumeIndex, cell_to_local_in,
};

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::sprite::{DrawSpace, SpriteBatches, SpriteDraw};
use super::{SceneExtractError, SceneExtractor, transform_matrix};

/// One volume already resolved into what it draws.
///
/// Everything here is a consequence of the volume, its grid, its transform and
/// the art it draws from -- never of where the camera is. That split is the
/// point: the camera moves every frame and none of this has to be rebuilt when
/// it does. Only the depth a cell sorts at is measured again, from the two
/// numbers each cell keeps for exactly that.
#[derive(Clone, Debug)]
pub(super) struct BakedVolume {
    /// The entity revision, binding generations and tile-set generation this
    /// was built from. Any of them moving on means this is stale.
    revision: u64,
    textures: u64,
    tile_sets: u64,
    layer: i32,
    /// Whether the volume authored any cells at all, which decides whether a
    /// missing world camera is an error. A volume with cells has to be drawn
    /// somewhere; an empty one has nothing to be wrong about.
    authored: bool,
    cells: Vec<BakedCell>,
    /// A solid grid's blocks, in chunks, each grouped by texture.
    ///
    /// Nothing about a face is measured again. A projected volume keeps two
    /// numbers per cell so its depth can be recomputed when the camera moves;
    /// a solid one has no depth to compute, because the depth buffer is doing
    /// it.
    ///
    /// Chunked rather than one list, because a world is bigger than a view of
    /// it. Submitting every face of a hundred and sixty cells square is
    /// seventy-odd thousand instances a frame to show the two per cent of them
    /// the camera frames -- enough CPU work per frame, before the GPU draws
    /// anything, to starve a browser's main thread.
    solid: Vec<BakedChunk>,
}

/// One square of a solid volume, and where it is.
#[derive(Clone, Debug)]
struct BakedChunk {
    /// World-space bounds of every face in it, for testing against a camera.
    min: Vec3,
    max: Vec3,
    faces: Vec<BakedSolidFace>,
}

/// How many columns square one chunk covers.
///
/// Small enough that a chunk is mostly inside or mostly outside the view,
/// large enough that the per-chunk test is not itself the cost. At sixteen, a
/// hundred and sixty square world is a hundred chunks and a framed view
/// touches a handful.
const CHUNK: i32 = 16;

impl BakedChunk {
    /// Whether any of this chunk could be on screen.
    ///
    /// Conservative on purpose: it rejects a chunk only when all eight corners
    /// fall outside the same clip plane, which can keep a chunk that is not
    /// really visible but can never drop one that is. A culling test that is
    /// wrong in the other direction takes bites out of the world as the camera
    /// turns, which is far worse than drawing a little too much.
    fn in_view(&self, view_projection: Mat4) -> bool {
        let corners = [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ];
        let clip = corners.map(|corner| view_projection * corner.extend(1.0));
        // Six planes, each rejected only if every corner is beyond it.
        !(clip.iter().all(|point| point.x < -point.w)
            || clip.iter().all(|point| point.x > point.w)
            || clip.iter().all(|point| point.y < -point.w)
            || clip.iter().all(|point| point.y > point.w)
            || clip.iter().all(|point| point.z < 0.0)
            || clip.iter().all(|point| point.z > point.w))
    }
}

/// One block face of a solid volume.
#[derive(Clone, Debug)]
struct BakedSolidFace {
    texture: TextureId,
    sprite: SpriteInstance,
}

/// One cell's faces, and the two numbers its depth is measured from.
#[derive(Clone, Debug)]
struct BakedCell {
    /// Where the cell's column meets the ground, in world space.
    ground: Vec3,
    /// The Z the whole column sorts at.
    depth_z: f32,
    faces: Vec<BakedFace>,
}

/// One face, ready to submit.
#[derive(Clone, Debug)]
struct BakedFace {
    model: Mat4,
    texture: TextureId,
    rect: UvRect,
    /// Tie-break among draws the camera puts at the same depth.
    order: u32,
}

impl SceneExtractor {
    pub(super) fn push_tile_volumes(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        tile_sets: Option<&TileSetBindings>,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
        let mut baked = self.baked_volumes.borrow_mut();
        // A volume that has been despawned should not be kept alive by the
        // cache that once drew it.
        baked.retain(|entity, _| world.contains(*entity));
        let textures_generation = textures.generation();
        let tile_sets_generation = tile_sets.map_or(0, TileSetBindings::generation);
        for entity in volume_entities(world) {
            // Re-asked every frame rather than baked, because it is not a
            // property of the volume: switching off an ancestor switches off
            // everything under it, and that ancestor's change is not this
            // entity's.
            if !world.is_active(entity) {
                continue;
            }
            let revision = world.revision(entity).unwrap_or_default();
            let fresh = baked.get(&entity).is_some_and(|volume| {
                volume.revision == revision
                    && volume.textures == textures_generation
                    && volume.tile_sets == tile_sets_generation
            });
            if !fresh {
                let volume = self.bake_tile_volume(world, entity, textures, tile_sets)?;
                baked.insert(
                    entity,
                    BakedVolume {
                        revision,
                        textures: textures_generation,
                        tile_sets: tile_sets_generation,
                        ..volume
                    },
                );
            }
            let volume = &baked[&entity];
            if !volume.authored {
                continue;
            }
            if !volume.solid.is_empty() {
                let framed = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
                for chunk in &volume.solid {
                    if !chunk.in_view(framed.view_projection) {
                        continue;
                    }
                    for face in &chunk.faces {
                        batches.push(SpriteDraw {
                            space: DrawSpace::Solid,
                            texture: face.texture,
                            // Every solid draw shares one order, so the stable
                            // sort before batching leaves them in the order
                            // they were baked -- grouped by texture within a
                            // chunk, which is a draw call per material per
                            // chunk rather than one per cell.
                            order: TransparentOrder::new(volume.layer, 0.0, 0)?,
                            sprite: face.sprite,
                        });
                    }
                }
                continue;
            }
            let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
            for cell in &volume.cells {
                let depth = camera_distance(camera.view, cell.ground.with_z(cell.depth_z));
                for face in &cell.faces {
                    let order = TransparentOrder::new(volume.layer, depth, face.order)?;
                    batches.push(SpriteDraw {
                        space: DrawSpace::World,
                        texture: face.texture,
                        order,
                        sprite: SpriteInstance::new(face.model, [1.0; 4]).with_uv_rect(face.rect),
                    });
                }
            }
        }
        Ok(())
    }

    /// Resolves one volume into its faces, from scratch.
    fn bake_tile_volume(
        &self,
        world: &World,
        entity: EntityId,
        textures: &TextureBindings,
        tile_sets: Option<&TileSetBindings>,
    ) -> Result<BakedVolume, SceneExtractError> {
        let empty = BakedVolume {
            revision: 0,
            textures: 0,
            tile_sets: 0,
            layer: 0,
            authored: false,
            cells: Vec::new(),
            solid: Vec::new(),
        };
        let Some(volume) = self.components.get::<TileVolumeComponent>(world, entity)? else {
            return Ok(empty);
        };
        // A freshly added volume is intentionally empty and has no tile-set
        // reference yet. There is nothing to render, so requiring a grid or
        // asset before the author has had a chance to choose either would
        // turn Add Component into an immediate scene error.
        if volume.cells.is_empty() && volume.tileset.is_empty() {
            return Ok(empty);
        }
        let grid = self
            .components
            .get::<TileGridComponent>(world, entity)?
            .ok_or(SceneExtractError::MissingTileGrid)?;
        grid.validate()?;
        let occupied = volume.index(&grid)?;
        let tile_set = tile_sets
            .and_then(|bindings| bindings.get(&volume.tileset))
            .ok_or_else(|| SceneExtractError::UnboundTileSet(volume.tileset.clone()))?;

        let transform = world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .unwrap_or_default();
        if let Some(cell_size) = grid.solid_cell() {
            return Ok(BakedVolume {
                authored: true,
                solid: solid_faces(
                    &volume,
                    tile_set,
                    textures,
                    cell_size,
                    transform_matrix(transform),
                )?,
                ..empty
            });
        }
        bake_projected_volume(
            &volume, &grid, tile_set, textures, transform, &occupied, empty,
        )
    }
}

/// Every sprite a tile set can name, parsed and resolved once.
///
/// There are tens of these and thousands of faces drawn from them, and the
/// string does not say anything different the six hundredth time it is read.
/// Every look a tile has, not just its first, or a variant with a broken sprite
/// would be a hole appearing in whichever cells happened to choose it.
fn resolved_sprites<'a>(
    tile_set: &'a TileSetDocument,
    textures: &TextureBindings,
) -> Result<BTreeMap<&'a str, (TextureId, UvRect)>, SceneExtractError> {
    let mut resolved: BTreeMap<&str, (TextureId, UvRect)> = BTreeMap::new();
    for definition in tile_set.tiles.values() {
        for faces in definition.all_faces() {
            for (_, visual) in faces.iter() {
                if resolved.contains_key(visual.sprite.as_str()) {
                    continue;
                }
                let reference = SpriteRef::parse(&visual.sprite)?;
                resolved.insert(visual.sprite.as_str(), textures.resolve_sprite(&reference));
            }
        }
    }
    Ok(resolved)
}

/// Every entity carrying a tile volume, without decoding any of them.
///
/// The point of not using `query` here: decoding a volume is proportional to
/// its cells, and on a cache hit the answer is already known. A farm-sized
/// island cost most of a millisecond a frame just to turn its six hundred
/// cells of JSON back into structs and throw them away.
fn volume_entities(world: &World) -> Vec<EntityId> {
    world
        .entities()
        .filter(|(_, data)| data.components.contains_key(TileVolumeComponent::TYPE_NAME))
        .map(|(entity, _)| entity)
        .collect()
}

/// Whether a neighbouring cell hides this face completely.
///
/// "Completely" is the word doing the work once tiles have heights. A face is
/// only dropped when nothing of it could be seen: a neighbour shorter than the
/// face it abuts leaves the rest of that face exposed, which is exactly what
/// makes a slab beside a block read as a slab rather than as a block.
pub(crate) fn face_is_occluded(
    occupied: &TileVolumeIndex<'_>,
    tileset: &str,
    tile_set: &TileSetDocument,
    coord: GridCoord3,
    face: TileFace,
    definition: &TileDefinition,
) -> Result<bool, SceneExtractError> {
    let [x, y, z] = face.neighbour_offset();
    let Some(neighbour) = coord.checked_offset(x, y, z) else {
        return Ok(false);
    };
    let Some(tile) = occupied.tile(neighbour) else {
        return Ok(false);
    };
    let neighbour = tile_set
        .tile(tile)
        .ok_or_else(|| SceneExtractError::UnknownTile {
            tile_set: tileset.to_owned(),
            tile: tile.to_owned(),
        })?;
    Ok(match face {
        // The cell above starts at this cell's ceiling, so it can only cover a
        // top face that reaches the ceiling. A slab's top sits below it, with
        // the gap an isometric camera looks into.
        TileFace::Top => neighbour.occludes && definition.fills_cell(),
        // Symmetrically, the cell below only reaches this floor when it is
        // full: a slab underneath leaves this cell's underside exposed.
        TileFace::Bottom => neighbour.occludes && neighbour.fills_cell(),
        TileFace::North | TileFace::West | TileFace::East | TileFace::South => {
            neighbour.hides_side_of(definition.height)
        }
    })
}

/// A solid volume's blocks, grouped by the texture each draws from.
///
/// Grouped here rather than sorted later because the sort before batching is
/// stable and keyed on space and order, which every solid face shares: coming
/// out of here in texture order is what makes an island a handful of draw
/// calls instead of one per cell.
fn solid_faces(
    volume: &TileVolumeComponent,
    tile_set: &TileSetDocument,
    textures: &TextureBindings,
    cell_size: [f32; 3],
    model: Mat4,
) -> Result<Vec<BakedChunk>, SceneExtractError> {
    let faces = crate::voxel::cube_faces(volume, tile_set, cell_size).map_err(|error| {
        let crate::voxel::VoxelError::UnknownTile { tile, .. } = error;
        SceneExtractError::UnknownTile {
            tile_set: volume.tileset.clone(),
            tile,
        }
    })?;
    let resolved = resolved_sprites(tile_set, textures)?;
    // Keyed by chunk first and texture second, so a chunk comes out as a run
    // per material rather than a draw call per face.
    let mut grouped: BTreeMap<(i32, i32), BTreeMap<TextureId, Vec<BakedSolidFace>>> =
        BTreeMap::new();
    let mut bounds: BTreeMap<(i32, i32), (Vec3, Vec3)> = BTreeMap::new();
    // A face's centre is a point; the quad around it reaches half a cell out
    // in the widest direction, so every bound is grown by that much before it
    // is used.
    let reach = cell_size[0].max(cell_size[1]).max(cell_size[2]) * 0.5;
    for face in faces {
        let Some(&(texture, rect)) = resolved.get(face.sprite.as_str()) else {
            continue;
        };
        // The face's own light, as a tint on otherwise unshaded art. A block
        // lit in its texture would keep its bright side pointing one way while
        // the camera walked round to the other.
        let shade = [face.shade, face.shade, face.shade, 1.0];
        let placed = model * face.model;
        let centre = placed.w_axis.truncate();
        let key = (face.cell.x.div_euclid(CHUNK), face.cell.y.div_euclid(CHUNK));
        let entry = bounds.entry(key).or_insert((centre, centre));
        entry.0 = entry.0.min(centre);
        entry.1 = entry.1.max(centre);
        grouped
            .entry(key)
            .or_default()
            .entry(texture)
            .or_default()
            .push(BakedSolidFace {
                texture,
                sprite: SpriteInstance::new(placed, shade)
                    .with_uv_rect(rect)
                    .with_corner_shade(face.corners),
            });
    }
    Ok(grouped
        .into_iter()
        .map(|(key, by_texture)| {
            let (min, max) = bounds[&key];
            BakedChunk {
                min: min - Vec3::splat(reach),
                max: max + Vec3::splat(reach),
                faces: by_texture.into_values().flatten().collect(),
            }
        })
        .collect())
}

/// The other kind of volume: quads arranged for one fixed viewpoint, in an
/// order this has to work out because nothing else will.
#[allow(clippy::too_many_arguments)]
fn bake_projected_volume(
    volume: &TileVolumeComponent,
    grid: &TileGridComponent,
    tile_set: &TileSetDocument,
    textures: &TextureBindings,
    transform: sindri_core::Transform3D,
    occupied: &TileVolumeIndex<'_>,
    empty: BakedVolume,
) -> Result<BakedVolume, SceneExtractError> {
    // Once per volume, not once per face. Building this from a transform
    // costs a quaternion and three composes, which was being paid about two
    // thousand times a frame on a farm-sized island.
    let model = transform_matrix(transform);
    // Built once. `cell_to_local` builds and validates one of these per
    // call, which was two calls a cell: thirteen hundred projections
    // constructed to be told the same arithmetic.
    let space = grid.volume_space().map_err(TileGridError::from)?;
    let resolved = resolved_sprites(tile_set, textures)?;
    let mut cells = volume.cells.iter().collect::<Vec<_>>();
    cells.sort_by_key(|cell| grid.depth_key(cell.coord()));

    let mut baked = Vec::with_capacity(cells.len());
    for (cell_index, cell) in cells.into_iter().enumerate() {
        let coord = cell.coord();
        let definition =
            tile_set
                .tile(&cell.tile)
                .ok_or_else(|| SceneExtractError::UnknownTile {
                    tile_set: volume.tileset.clone(),
                    tile: cell.tile.clone(),
                })?;
        let [cell_x, cell_y] = cell_to_local_in(&space, coord)
            .expect("a validated grid projects finite integer cells");
        // Depth is a property of the *cell*, taken from where its column
        // meets the ground, and every face of it shares that one value. Two
        // things follow, and both were wrong while each face measured its
        // own drawn position.
        //
        // Raising a block moves it up the screen, which is not moving it
        // toward the viewer: a stack has to keep the depth of the column it
        // stands in, or a tower walks in front of everything south of it as
        // it grows.
        //
        // And a block's own faces must not sort against each other. Their
        // offsets differ by a fraction of a cell, which was enough to
        // interleave a top with the side of the block beside it. Which face
        // of a cell is drawn first is decided by the face order, not by
        // arithmetic on where its art happens to sit.
        let [ground_x, ground_y] = cell_to_local_in(&space, GridCoord3::new(coord.x, coord.y, 0))
            .expect("a validated grid projects finite integer cells");
        let ground = model * Mat4::from_translation(Vec3::new(ground_x, ground_y, 0.0));
        // The cell's own Z, by the same rule anything standing on this grid
        // takes: depth is a consequence of position, so a block and a prop
        // are finally measured on one axis instead of two. Zero
        // `depth_step` leaves every cell at the volume's own Z, which is
        // what a backdrop wants and what every scene written before this
        // did.
        // Half a step back, because a cell *is* the ground and anything
        // placed on it rests on top: they share a column, so without the
        // bias they tie and submission order decides whether a shrine
        // stands on its flagstone or under it. This is what the
        // hand-written even-and-odd layers encoded before depth was
        // derived.
        let cell_z = transform.position[2] + grid.depth_z(grid.face_depth(coord));
        let visible = grid.projection.visible_faces();
        // Which look this cell has, decided once for the whole cell: a
        // block whose top came from one variant and whose side came from
        // another is not a block.
        let faces =
            definition.faces_at(volume.variant_seed, [coord.x, coord.y, coord.z], &cell.tile);
        let mut baked_faces = Vec::new();
        for (face_index, (face, visual)) in faces.iter().enumerate() {
            // A face the projection turns away from is not culled by a
            // neighbour; there is simply no view of it to draw. An
            // isometric side in an orthogonal volume would otherwise paint
            // itself flat across the block.
            if !visible.contains(&face) {
                continue;
            }
            if face_is_occluded(occupied, &volume.tileset, tile_set, coord, face, definition)? {
                continue;
            }
            let (texture, rect) = *resolved
                .get(visual.sprite.as_str())
                .expect("every face of a bound tile set was resolved above");
            let local = Mat4::from_translation(Vec3::new(
                cell_x + visual.offset[0],
                cell_y + visual.offset[1],
                0.0,
            )) * Mat4::from_scale(Vec3::new(visual.size[0], visual.size[1], 1.0));
            // Cells were sorted back to front above, so the submission
            // index carries that order and the face index orders the faces
            // inside one cell. It is what decides between draws the camera
            // puts at the same depth -- the levels of one column, and
            // anything a projection lays out along the view.
            let stable = cell_index.saturating_mul(6).saturating_add(face_index);
            baked_faces.push(BakedFace {
                model: model * local,
                texture,
                rect,
                order: u32::try_from(stable).unwrap_or(u32::MAX),
            });
        }
        if baked_faces.is_empty() {
            continue;
        }
        baked.push(BakedCell {
            ground: ground.w_axis.truncate(),
            depth_z: cell_z,
            faces: baked_faces,
        });
    }
    Ok(BakedVolume {
        layer: volume.layer,
        authored: true,
        cells: baked,
        ..empty
    })
}
