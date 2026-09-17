//! Stackable cells expanded into only their visible baked faces.

use sindri_core::{SpriteSheetDocument, TileSetDocument};
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings};

use crate::support::{VIEWPORT, document, scene, world_from};

fn tile_sets() -> TileSetBindings {
    bind(
        r#"{
          "format_version": 1,
          "tiles": { "grass": { "faces": {
            "top": {
              "sprite": "blocks.png#0", "size": [1.0, 0.5]
            },
            "south": {
              "sprite": "blocks.png#1", "size": [1.0, 0.5],
              "offset": [0.0, -0.25]
            }
          } } }
        }"#,
    )
}

/// The same two tiles, plus a half-height one. A slab is its own tile with its
/// own baked faces, the way a slab in a voxel game is its own block.
fn tile_sets_with_a_slab() -> TileSetBindings {
    bind(
        r#"{
          "format_version": 1,
          "tiles": {
            "grass": { "faces": {
              "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5] },
              "south": { "sprite": "blocks.png#1", "size": [1.0, 0.5], "offset": [0.0, -0.25] }
            } },
            "grass-slab": { "height": 0.5, "faces": {
              "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5], "offset": [0.0, -0.25] },
              "south": { "sprite": "blocks.png#1", "size": [1.0, 0.25], "offset": [0.0, -0.375] }
            } }
          }
        }"#,
    )
}

/// Top, south and east, so an orthogonal volume has an east face to drop.
fn tile_sets_with_three_faces() -> TileSetBindings {
    bind(
        r#"{
          "format_version": 1,
          "tiles": { "grass": { "faces": {
            "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5] },
            "south": { "sprite": "blocks.png#1", "size": [1.0, 0.5], "offset": [0.0, -0.25] },
            "east": { "sprite": "blocks.png#1", "size": [1.0, 0.5], "offset": [0.25, -0.25] }
          } } }
        }"#,
    )
}

fn bind(json: &str) -> TileSetBindings {
    let document = TileSetDocument::from_json(json).unwrap();
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("blocks.png", TextureId::new(9));
    bindings
        .bind_sheet("blocks.png", &SpriteSheetDocument::from_grid(2, 1))
        .unwrap();
    bindings
}

/// The world holding one volume of these cells in this projection.
///
/// Framed by the camera a tile game actually uses: orthographic, square on to
/// the plane the cells are laid out in. The shared test camera is a perspective
/// one at an arbitrary angle, which is fine for asking whether something was
/// drawn and useless for asking in what order — it puts a depth of its own on
/// every cell and decides the answer before the volume's own ordering is
/// consulted.
fn volume_scene(cells: &str, projection: &str) -> sindri_core::World {
    world_from(&document(&format!(
        r#"
        {{ "id": "main-camera", "transform_3d": {{ "position": [0.0, 0.0, 10.0] }},
           "components": {{ "sindri.camera": {{
             "projection": "orthographic", "vertical_size": 8.0,
             "near": 0.1, "far": 100.0 }} }} }},
        {{ "id": "blocks", "transform_3d": {{}}, "components": {{
          "sindri.tile_grid": {{
            "columns": 3, "rows": 3, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "{projection}"
          }},
          "sindri.tile_volume": {{
            "tileset": "world.tileset.json",
            "cells": [{cells}]
          }}
        }} }}"#
    )))
}

/// Every sprite instance the volume draws, in the order it draws them.
fn instances(
    cells: &str,
    projection: &str,
    tile_sets: &TileSetBindings,
) -> Vec<sindri_render::SpriteInstance> {
    SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &volume_scene(cells, projection),
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(tile_sets),
        )
        .expect("the volume extracts")
        .passes()
        .iter()
        .flat_map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// How many sprite instances a volume of these cells draws.
fn drawn(cells: &str, tile_sets: &TileSetBindings) -> usize {
    instances(cells, "isometric", tile_sets).len()
}

#[test]
fn stacking_culls_the_shared_face_and_keeps_exposed_faces() {
    let world = world_from(&scene(
        r#",
        { "id": "blocks", "transform_3d": {}, "components": {
          "sindri.tile_grid": {
            "columns": 2, "rows": 2, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          },
          "sindri.tile_volume": {
            "tileset": "world.tileset.json",
            "cells": [
              { "position": [0, 0, 0], "tile": "grass" },
              { "position": [0, 0, 1], "tile": "grass" }
            ]
          }
        } }"#,
    ));
    let tile_sets = tile_sets();
    let frame = SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(&tile_sets),
        )
        .expect("the volume extracts");

    let instances = frame
        .passes()
        .iter()
        .map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances.len(),
            _ => 0,
        })
        .sum::<usize>();
    assert_eq!(
        instances, 3,
        "the lower top is hidden; two south faces and the upper top remain"
    );
}

#[test]
fn a_volume_requires_its_reusable_tile_set_binding() {
    let world = world_from(&scene(
        r#",
        { "id": "blocks", "transform_3d": {}, "components": {
          "sindri.tile_grid": { "columns": 1, "rows": 1 },
          "sindri.tile_volume": {
            "tileset": "missing.tileset.json",
            "cells": [{ "position": [0, 0, 0], "tile": "grass" }]
          }
        } }"#,
    ));
    let error = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect_err("an unbound tile set is not guessed");
    assert!(error.to_string().contains("missing.tileset.json"));
}

#[test]
fn a_full_block_above_a_slab_leaves_the_slabs_top_showing() {
    // The slab's top sits half a cell below the block's floor, so the gap
    // between them is something an isometric camera looks into. Culling that
    // top face is what would make a slab indistinguishable from a block.
    let stacked = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass-slab" },
           { "position": [0, 0, 1], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    assert_eq!(
        stacked, 4,
        "both tops and both south faces should survive: {stacked}"
    );

    // A full block in the same place hides the top beneath it, which is the
    // behaviour heights must not have broken.
    let solid = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass" },
           { "position": [0, 0, 1], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    assert_eq!(solid, 3, "the lower top is hidden by a full block: {solid}");
}

#[test]
fn a_slab_does_not_hide_the_side_of_a_block_beside_it() {
    // The slab covers the bottom half of its neighbour's south face and leaves
    // the top half exposed, so the face has to be drawn.
    let slab_neighbour = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass" },
           { "position": [0, 1, 0], "tile": "grass-slab" }"#,
        &tile_sets_with_a_slab(),
    );
    let block_neighbour = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass" },
           { "position": [0, 1, 0], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    assert!(
        slab_neighbour > block_neighbour,
        "a short neighbour should hide less than a full one: {slab_neighbour} vs {block_neighbour}"
    );
}

#[test]
fn a_block_beside_a_slab_still_hides_the_slabs_side() {
    // The other direction: the neighbour is taller than the face it abuts, so
    // nothing of that face could be seen and it is dropped.
    let hidden = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass-slab" },
           { "position": [0, 1, 0], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    let exposed = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass-slab" }"#,
        &tile_sets_with_a_slab(),
    );
    assert_eq!(
        hidden,
        exposed + 1,
        "the block brings two faces of its own and takes the slab's south face          away, so the pair draws one more than the slab alone: {hidden} vs {exposed}"
    );
}

#[test]
fn a_height_outside_one_cell_is_refused_when_the_asset_decodes() {
    let error = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": { "tall": { "height": 1.5, "faces": {
            "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5] }
          } } }
        }"#,
    )
    .expect_err("a tile taller than its cell is not a tile");
    assert!(
        error.to_string().contains("1.5"),
        "the failure should quote the height written: {error}"
    );
}

#[test]
fn an_orthogonal_volume_leaves_out_the_side_it_cannot_see() {
    // The same tile set in both projections. An orthogonal view looks straight
    // down the rows, so a cell's east face is edge-on and has no width to draw;
    // an isometric view is turned between the axes and shows it.
    let set = tile_sets_with_three_faces();
    let cells = r#"{ "position": [1, 1, 0], "tile": "grass" }"#;
    assert_eq!(
        instances(cells, "isometric", &set).len(),
        3,
        "an isometric block shows its top and both turned sides"
    );
    assert_eq!(
        instances(cells, "orthogonal", &set).len(),
        2,
        "an orthogonal block shows its top and the side facing the viewer"
    );
}

#[test]
fn depth_order_follows_the_projection_rather_than_the_diagonal() {
    // Two cells the two projections disagree about. Along the diagonal, the
    // cell two columns east is nearer; straight down the rows, it is the cell
    // one row south that is nearer, and east is neither nearer nor further.
    let set = tile_sets();
    let cells = r#"{ "position": [2, 0, 0], "tile": "grass" },
                    { "position": [0, 1, 0], "tile": "grass" }"#;
    let first_x = |projection: &str| {
        instances(cells, projection, &set)
            .first()
            .expect("the volume draws")
            .model()
            .w_axis
            .x
    };

    // Isometric: (0, 1) has depth 1 and (2, 0) has depth 2, so the eastern cell
    // is drawn last and its faces are the ones in front.
    assert!(
        first_x("isometric") < first_x("orthogonal"),
        "the two projections should not agree about which cell is behind"
    );
}

#[test]
fn raising_a_block_does_not_move_it_toward_the_viewer() {
    // A stack grows up the screen, which is not the same as growing toward the
    // camera. The block raised a level at the back row still has to be drawn
    // before -- behind -- the block on the ground one row south of it, or a
    // tower walks out in front of everything south of it as it is built.
    let set = tile_sets();
    let rows = instances(
        r#"{ "position": [0, 0, 1], "tile": "grass" },
           { "position": [0, 1, 0], "tile": "grass" }"#,
        "orthogonal",
        &set,
    )
    .iter()
    .map(|instance| instance.model().w_axis.y)
    .collect::<Vec<_>>();

    let (back, front) = rows.split_at(2);
    assert!(
        back.iter().copied().fold(f32::MAX, f32::min)
            > front.iter().copied().fold(f32::MIN, f32::max),
        "every face of the back row should be drawn before any face of the row \
         in front of it: {rows:?}"
    );
}

#[test]
fn the_faces_of_one_cell_do_not_sort_against_each_other() {
    // Framed by the shared perspective camera on purpose: it puts a different
    // depth on every point in the scene, which is what used to take a block
    // apart. A cell's faces sit a fraction of a cell from one another and each
    // measured its own drawn position, so a block's east face was depth-sorted
    // against its own top and drawn after it.
    //
    // They are one block. Every face of a cell takes the depth of the column it
    // stands in, and the tile set's face order decides between them — east
    // before south before top, as `TileFaces::iter` lists them.
    let set = tile_sets_with_three_faces();
    let world = world_from(&scene(
        r#",
        { "id": "blocks", "transform_3d": {}, "components": {
          "sindri.tile_grid": {
            "columns": 4, "rows": 4, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          },
          "sindri.tile_volume": { "tileset": "world.tileset.json", "cells": [
            { "position": [0, 0, 0], "tile": "grass" },
            { "position": [0, 0, 1], "tile": "grass" },
            { "position": [1, 1, 0], "tile": "grass" }
          ] }
        } }"#,
    ));
    let xs = SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(&set),
        )
        .expect("the volume extracts")
        .passes()
        .iter()
        .flat_map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances.clone(),
            _ => Vec::new(),
        })
        .map(|instance| instance.model().w_axis.x)
        .collect::<Vec<_>>();

    // Only the east face carries an X offset, so it marks where each cell's run
    // of faces begins: three faces, then the stacked cell's two — its top is
    // under the block above — then three again.
    assert_eq!(
        xs.len(),
        8,
        "three cells, one of them with its top covered: {xs:?}"
    );
    let east_first = [0, 3, 5]
        .into_iter()
        .all(|start| (xs[start] - 0.25).abs() < 1.0e-5);
    assert!(
        east_first,
        "each cell's faces should begin at its east face rather than be \
         reordered by where their art sits: {xs:?}"
    );
}
