//! Which entity a point in a viewport names.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sindri_core::EntityData;
use sindri_scene::SceneExtractor;

use super::*;

fn spawn(world: &mut World, transform: Transform3D, type_name: &str, payload: Value) -> EntityId {
    world.spawn(EntityData {
        transform_3d: Some(transform),
        components: BTreeMap::from([(type_name.to_owned(), payload)]),
        ..EntityData::default()
    })
}

fn sprite(layer: i32) -> Value {
    json!({
        "texture": "procedural:checkerboard",
        "space": "world",
        "layer": layer
    })
}

#[test]
fn a_click_selects_the_sprite_quad_and_misses_outside_it() {
    let extractor = SceneExtractor::new().unwrap();
    let mut world = World::default();
    let sprite = spawn(
        &mut world,
        Transform3D {
            position: [0.0, 0.0, 0.5],
            ..Transform3D::default()
        },
        "sindri.sprite",
        sprite(0),
    );

    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
        Some(sprite)
    );
    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.9, 0.5]).unwrap(),
        None
    );
}

#[test]
fn the_higher_sprite_layer_wins_even_when_it_is_farther_back() {
    let extractor = SceneExtractor::new().unwrap();
    let mut world = World::default();
    let _near = spawn(
        &mut world,
        Transform3D {
            position: [0.0, 0.0, 0.2],
            ..Transform3D::default()
        },
        "sindri.sprite",
        sprite(0),
    );
    let high_layer = spawn(
        &mut world,
        Transform3D {
            position: [0.0, 0.0, 0.8],
            ..Transform3D::default()
        },
        "sindri.sprite",
        sprite(1),
    );

    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
        Some(high_layer)
    );
}

#[test]
fn opaque_geometry_blocks_a_sprite_behind_but_not_one_in_front() {
    let extractor = SceneExtractor::new().unwrap();
    let mut world = World::default();
    let cube = spawn(
        &mut world,
        Transform3D {
            position: [0.0, 0.0, 0.6],
            scale: [0.1, 0.1, 0.1],
            ..Transform3D::default()
        },
        "sindri.mesh",
        json!({
            "primitive": "cube",
            "texture": "procedural:checkerboard"
        }),
    );
    let _behind = spawn(
        &mut world,
        Transform3D {
            position: [0.0, 0.0, 0.8],
            ..Transform3D::default()
        },
        "sindri.sprite",
        sprite(10),
    );

    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
        Some(cube)
    );

    let in_front = spawn(
        &mut world,
        Transform3D {
            position: [0.0, 0.0, 0.2],
            ..Transform3D::default()
        },
        "sindri.sprite",
        sprite(-10),
    );
    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
        Some(in_front)
    );
}

#[test]
fn only_a_filled_tilemap_cell_selects_the_map() {
    let extractor = SceneExtractor::new().unwrap();
    let mut world = World::default();
    let map = spawn(
        &mut world,
        Transform3D {
            position: [-0.5, 0.5, 0.5],
            ..Transform3D::default()
        },
        "sindri.tilemap",
        json!({
            "texture": "textures/tiles.png",
            "palette": ["floor"],
            "columns": 2,
            "rows": 1,
            "tiles": [0, null],
            "space": "world"
        }),
    );

    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
        Some(map)
    );
    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [1.0, 0.5]).unwrap(),
        None
    );
}

/// A UI element anchored away from the centre is picked where it is drawn,
/// not where its transform points in the world.
///
/// The whole reason UI needs a pass of its own: an anchor picks a point on
/// the viewport and the transform is an offset from it, so a world ray
/// through a world camera passes nowhere near the element. Twelve of
/// Gather's twenty-two entities are UI, and none of them could be clicked
/// in the view that draws them.
#[test]
fn an_anchored_ui_image_is_picked_where_it_is_drawn() {
    let extractor = SceneExtractor::new().unwrap();
    let (overlay, placement) = sindri_scene::overlay_for_viewport(1.0).unwrap();
    let mut world = World::default();
    // Top centre, nudged down by a fifth of the overlay's half-height.
    let banner = spawn(
        &mut world,
        Transform3D {
            position: [0.0, -0.2, 0.0],
            ..Transform3D::default()
        },
        "sindri.ui.image",
        json!({ "texture": "procedural:checkerboard", "anchor": "top" }),
    );

    let pick = |point| pick_ui(&world, extractor.components(), overlay, &placement, point).unwrap();
    // Near the top of the viewport, which is where an element anchored
    // there is drawn — and where its transform, read as a world position,
    // says nothing at all.
    assert_eq!(pick([0.5, 0.1]), Some(banner));
    assert_eq!(pick([0.5, 0.5]), None, "the centre is not the top");
    assert_eq!(pick([0.05, 0.1]), None, "and neither is the left edge");
}

/// Two overlapping elements: the higher layer is the one picked, matching
/// the order the renderer stacks them in.
#[test]
fn the_higher_ui_layer_wins_where_two_overlap() {
    let extractor = SceneExtractor::new().unwrap();
    let (overlay, placement) = sindri_scene::overlay_for_viewport(1.0).unwrap();
    let mut world = World::default();
    let ui_image = |layer: i32| json!({ "texture": "procedural:checkerboard", "anchor": "center", "layer": layer });
    spawn(
        &mut world,
        Transform3D::default(),
        "sindri.ui.image",
        ui_image(0),
    );
    let pip = spawn(
        &mut world,
        Transform3D::default(),
        "sindri.ui.image",
        ui_image(3),
    );

    assert_eq!(
        pick_ui(
            &world,
            extractor.components(),
            overlay,
            &placement,
            [0.5, 0.5]
        )
        .unwrap(),
        Some(pip)
    );
}

/// UI text is not picked, deliberately: what a string covers is decided by
/// glyph layout inside the text renderer, and a guessed box for it would
/// select the wrong thing near its edges.
#[test]
fn ui_text_is_left_to_the_hierarchy() {
    let extractor = SceneExtractor::new().unwrap();
    let (overlay, placement) = sindri_scene::overlay_for_viewport(1.0).unwrap();
    let mut world = World::default();
    spawn(
        &mut world,
        Transform3D::default(),
        "sindri.ui.text",
        json!({ "text": "GATHER", "font": "fonts/Inter.ttf", "anchor": "center" }),
    );

    assert_eq!(
        pick_ui(
            &world,
            extractor.components(),
            overlay,
            &placement,
            [0.5, 0.5]
        )
        .unwrap(),
        None
    );
}

/// A fully transparent element is not clickable.
///
/// Gather's win banner is `tint` alpha zero, a third of the viewport wide,
/// sitting in the middle of the scene until the game says otherwise. Picked,
/// it swallowed every click in the centre of the Scene view and selected an
/// element nobody can see — confirmed in the running editor, where clicking
/// the player selected the banner instead.
#[test]
fn an_invisible_element_is_not_clicked() {
    let extractor = SceneExtractor::new().unwrap();
    let (overlay, placement) = sindri_scene::overlay_for_viewport(1.0).unwrap();
    let mut world = World::default();
    spawn(
        &mut world,
        Transform3D::default(),
        "sindri.ui.image",
        json!({
            "texture": "procedural:checkerboard",
            "anchor": "center",
            "tint": [1.0, 1.0, 1.0, 0.0],
            "layer": 120
        }),
    );

    assert_eq!(
        pick_ui(
            &world,
            extractor.components(),
            overlay,
            &placement,
            [0.5, 0.5]
        )
        .unwrap(),
        None,
        "an element drawn as nothing is not an element to click"
    );

    // The same rule in the world, where a transparent sprite would hide
    // whatever is behind it from the pointer just as completely.
    let mut world = World::default();
    spawn(
        &mut world,
        Transform3D::default(),
        "sindri.sprite",
        json!({ "texture": "procedural:checkerboard", "tint": [1.0, 1.0, 1.0, 0.0] }),
    );
    assert_eq!(
        pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
        None
    );
}
