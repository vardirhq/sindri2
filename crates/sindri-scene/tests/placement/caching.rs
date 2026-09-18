//! How often the ground under a placement is worked out.

use serde_json::{Value, json};
use sindri_scene::{GridSurfaces, resolve_grid_placements};

use crate::support::{placed, tile_sets, transform_of, world_with};

/// The cost that made a landscape unplayable, pinned where it can be seen.
///
/// Placement runs every frame, and deriving a volume's surfaces walks every
/// occupied cell in it. On a generated world that was about 185ms a frame --
/// more than everything else in a frame put together, enough to starve the
/// browser's main thread so badly that a background track never started. The
/// answer is not to derive faster but to derive once.
#[test]
fn a_grid_that_did_not_change_is_not_derived_again() {
    let (mut world, extractor) = world_with(vec![placed("rock", &json!([1, 1]))], 0.0);
    let sets = tile_sets();
    let mut surfaces = GridSurfaces::default();

    for _ in 0..8 {
        resolve_grid_placements(
            &mut world,
            extractor.components(),
            Some(&sets),
            &mut surfaces,
        )
        .expect("the placement resolves");
    }
    assert_eq!(
        surfaces.derivations(),
        1,
        "eight frames over an unchanged grid derive its surfaces once"
    );
}

/// The other half of the bargain: a cache that never re-derives is a bug that
/// leaves props standing on ground that has moved out from under them.
#[test]
fn changing_the_ground_derives_it_again() {
    let (mut world, extractor) = world_with(vec![placed("rock", &json!([1, 1]))], 0.0);
    let sets = tile_sets();
    let mut surfaces = GridSurfaces::default();
    resolve_grid_placements(
        &mut world,
        extractor.components(),
        Some(&sets),
        &mut surfaces,
    )
    .unwrap();
    let before = transform_of(&world, "rock").position[1];

    let floor = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|value| value.as_str() == "floor")
        })
        .map(|(entity, _)| entity)
        .unwrap();
    world
        .get_mut(floor)
        .and_then(|data| data.components.get_mut("sindri.tile_volume"))
        .and_then(|payload| payload.get_mut("cells"))
        .and_then(Value::as_array_mut)
        .unwrap()
        .push(json!({ "position": [1, 1, 0], "tile": "block" }));

    resolve_grid_placements(
        &mut world,
        extractor.components(),
        Some(&sets),
        &mut surfaces,
    )
    .unwrap();
    assert_eq!(
        surfaces.derivations(),
        2,
        "a volume that changed is derived again"
    );
    assert!(
        transform_of(&world, "rock").position[1] > before,
        "and the prop rides the ground up rather than staying on a cached floor"
    );
}
