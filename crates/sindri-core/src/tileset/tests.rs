//! What a tile set means, and what a face does when the art stops short.

use super::*;

#[test]
fn a_valid_tile_set_keeps_semantics_separate_from_faces() {
    let set = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": { "grass": {
            "faces": {
              "top": { "sprite": "blocks.png#grass_top", "size": [1.0, 0.5] },
              "south": { "sprite": "blocks.png#earth_south", "size": [1.0, 0.5], "offset": [0.0, -0.25] }
            }
          } }
        }"#,
    )
    .unwrap();
    let grass = set.tile("grass").unwrap();
    assert!(grass.supports);
    assert!(grass.walkable);
    assert!(grass.occludes);
    assert!(grass.faces.get(TileFace::Top).is_some());
}

/// `solid` was the old name for `walkable`, and a document that used it
/// still means what it meant.
#[test]
fn the_old_solid_field_still_reads_as_walkable() {
    let set = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": {
            "water": {
              "solid": false,
              "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 0.5] } }
            }
          }
        }"#,
    )
    .unwrap();
    let water = set.tile("water").unwrap();
    assert!(!water.walkable, "solid: false meant you cannot stand there");
    assert!(
        water.supports,
        "and said nothing about whether anything rests on it, so the \
         pond holds a boat up rather than being a hole"
    );
}

/// Standing on something rests on it, so the pair cannot disagree.
#[test]
fn a_walkable_tile_that_supports_nothing_is_rejected() {
    let error = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": {
            "ghost": {
              "supports": false,
              "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 0.5] } }
            }
          }
        }"#,
    )
    .unwrap_err();
    assert!(
        matches!(error, TileSetError::WalkableWithoutSupport(ref tile) if tile == "ghost"),
        "{error:?}"
    );
}

#[test]
fn invalid_visual_geometry_is_rejected_at_asset_decode() {
    let error = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": { "grass": { "faces": {
            "top": { "sprite": "blocks.png#grass", "size": [1.0, 0.0] }
          } } }
        }"#,
    )
    .unwrap_err();
    assert!(matches!(error, TileSetError::InvalidSize { .. }));
}

/// Three-sided art on a cube a camera can go round.
#[test]
fn a_face_the_art_never_drew_borrows_the_one_opposite_it() {
    let set = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": { "grass": { "faces": {
            "top": { "sprite": "blocks.png#grass-top", "size": [1.0, 1.0] },
            "south": { "sprite": "blocks.png#grass-south", "size": [1.0, 1.0] },
            "east": { "sprite": "blocks.png#grass-east", "size": [1.0, 1.0] }
          } } }
        }"#,
    )
    .expect("the tile set parses");
    let faces = &set.tile("grass").expect("the tile is there").faces;

    for (face, expected) in [
        (TileFace::Top, "blocks.png#grass-top"),
        (TileFace::South, "blocks.png#grass-south"),
        (TileFace::East, "blocks.png#grass-east"),
        // The three nobody could see, and so nobody drew.
        (TileFace::Bottom, "blocks.png#grass-top"),
        (TileFace::North, "blocks.png#grass-south"),
        (TileFace::West, "blocks.png#grass-east"),
    ] {
        let (_, visual) = faces
            .resolved(face)
            .unwrap_or_else(|| panic!("{face:?} resolves to something"));
        assert_eq!(visual.sprite, expected, "{face:?}");
    }

    // A borrowed face says so, so a caller that wants to mirror it can.
    assert_eq!(
        faces.resolved(TileFace::North).map(|(face, _)| face),
        Some(TileFace::South)
    );
    assert_eq!(
        faces.resolved(TileFace::South).map(|(face, _)| face),
        Some(TileFace::South)
    );
}

/// The fallback runs both ways: art drawn from the other side works too.
#[test]
fn a_tile_drawn_from_the_other_side_resolves_just_as_well() {
    let set = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": { "slab": { "faces": {
            "bottom": { "sprite": "blocks.png#slab-bottom", "size": [1.0, 1.0] },
            "north": { "sprite": "blocks.png#slab-north", "size": [1.0, 1.0] }
          } } }
        }"#,
    )
    .expect("the tile set parses");
    let faces = &set.tile("slab").expect("the tile is there").faces;

    assert_eq!(
        faces
            .resolved(TileFace::Top)
            .map(|(_, v)| v.sprite.as_str()),
        Some("blocks.png#slab-bottom")
    );
    assert_eq!(
        faces
            .resolved(TileFace::South)
            .map(|(_, v)| v.sprite.as_str()),
        Some("blocks.png#slab-north")
    );
    // Nothing to borrow from: east and west are both undrawn.
    assert!(faces.resolved(TileFace::East).is_none());
}
