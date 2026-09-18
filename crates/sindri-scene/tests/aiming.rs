//! What the pointer is on, through the components a scene authors.

use glam::Mat4;
use serde_json::json;
use sindri_core::{
    EntityData, SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, TileFace,
    Transform3D, World,
};
use sindri_scene::{SceneExtractor, voxel};

fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("a valid scene ID")
}

/// Straight down at the origin, so which block is nearest is not a matter of
/// opinion and the face met is the top.
fn overhead() -> Mat4 {
    glam::camera::rh::proj::directx::orthographic(-4.0, 4.0, -4.0, 4.0, 0.1, 20.0)
        * glam::camera::rh::view::look_at_mat4(
            glam::Vec3::new(0.0, 10.0, 0.0),
            glam::Vec3::ZERO,
            // Looking along Y, so "up" on screen has to be another axis.
            glam::Vec3::NEG_Z,
        )
}

/// A one-block tower at the origin, two cells tall, on a grid of boxes.
fn world_with(space: &str) -> (World, SceneExtractor) {
    let mut floor = SceneEntity::new(id("floor"));
    floor.transform_3d = Some(Transform3D::default());
    floor.components.insert(
        "sindri.tile_grid".to_owned(),
        json!({
            "columns": 4, "rows": 4, "cell_size": [1.0, 1.0],
            "cell_height": 1.0, "projection": "isometric", "space": space
        }),
    );
    floor.components.insert(
        "sindri.tile_volume".to_owned(),
        json!({
            "tileset": "world.tileset.json",
            "cells": [
                { "position": [0, 0, 0], "tile": "block" },
                { "position": [0, 0, 1], "tile": "block" }
            ]
        }),
    );
    let document = SceneDocument {
        format_version: SCENE_FORMAT_VERSION,
        metadata: sindri_core::SceneMetadata::default(),
        entities: vec![floor],
    };
    let extractor = SceneExtractor::new().expect("the schemas register");
    let mut world = World::default();
    sindri_core::LoadedScenes::new()
        .enter_keeping_identities(&mut world, "test", &document)
        .expect("the scene loads");
    (world, extractor)
}

#[test]
fn pointing_at_a_tower_finds_its_top_block_and_the_space_above_it() {
    // The two halves of a click, from one ray. Looking straight down a
    // two-block tower, the block under the pointer is the upper one -- not the
    // lower one the ray also passes through afterwards -- and the cell a new
    // block would take is the empty one above it.
    let (world, extractor) = world_with("solid");
    let aim = voxel::aim_at(&world, extractor.components(), overhead(), [0.5, 0.5])
        .expect("the pointer is on the tower");
    assert_eq!(
        (aim.cell.x, aim.cell.y, aim.cell.z),
        (0, 0, 1),
        "the nearest block along the ray is the one you are looking at"
    );
    assert_eq!(aim.face, TileFace::Top, "looking down meets the top");
    assert_eq!(
        (aim.against.x, aim.against.y, aim.against.z),
        (0, 0, 2),
        "a block attached to the top goes above it"
    );
}

#[test]
fn pointing_past_the_tower_finds_nothing() {
    // Not an error: pointing at the sky is an ordinary thing to do, and a
    // caller has one situation to handle rather than two.
    let (world, extractor) = world_with("solid");
    assert!(
        voxel::aim_at(&world, extractor.components(), overhead(), [0.05, 0.05]).is_none(),
        "a ray that meets no block reports none"
    );
}

#[test]
fn a_projected_grid_has_no_side_to_point_at() {
    // A projected volume draws a convincing picture of blocks, and the picture
    // has no near side: its faces are arranged for one viewpoint rather than
    // standing in the world. Answering a pick from it would invent a side.
    let (world, extractor) = world_with("projected");
    assert!(
        voxel::aim_at(&world, extractor.components(), overhead(), [0.5, 0.5]).is_none(),
        "only a grid of boxes can be pointed at"
    );
}

#[test]
fn the_grid_has_to_be_where_the_scene_put_it() {
    // The ray is cast into the volume's own space, so a floor that has moved
    // takes its blocks with it. Without undoing the model the pointer would
    // keep hitting wherever the volume used to be.
    let (mut world, extractor) = world_with("solid");
    let floor = world
        .entities()
        .find(|(_, data): &(_, &EntityData)| {
            data.source_id
                .as_ref()
                .is_some_and(|value| value.as_str() == "floor")
        })
        .map(|(entity, _)| entity)
        .expect("the floor is there");
    let mut moved = world.get(floor).and_then(|data| data.transform_3d).unwrap();
    moved.position = [3.0, 0.0, 0.0];
    world.get_mut(floor).unwrap().transform_3d = Some(moved);

    assert!(
        voxel::aim_at(&world, extractor.components(), overhead(), [0.5, 0.5]).is_none(),
        "the tower moved out from under the pointer"
    );
    // Three units across, in a view eight wide, is three-eighths of the way
    // from the middle to the right-hand edge.
    let aim = voxel::aim_at(
        &world,
        extractor.components(),
        overhead(),
        [0.5 + 3.0 / 8.0, 0.5],
    )
    .expect("the pointer is on the moved tower");
    assert_eq!((aim.cell.x, aim.cell.y, aim.cell.z), (0, 0, 1));
}
