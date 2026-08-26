//! Painting a tile, resizing a map, and undoing both.

use super::{TileBrush, paint, resize, tile_at_viewport, tile_outline};
use glam::Mat4;
use serde_json::json;
use sindri_core::{CommandBuffer, CommandHistory, EntityData, Transform3D, World, WorldCommand};
use sindri_scene::TilemapComponent;

fn payload() -> serde_json::Value {
    json!({
        "texture": "tiles.png",
        "palette": ["floor"],
        "columns": 2,
        "rows": 2,
        "tile_size": [1.0, 1.0],
        "projection": "orthogonal",
        "tiles": [0, null, null, 0],
        "space": "world",
        "future": { "kept": true }
    })
}

#[test]
fn resizing_preserves_the_overlap_and_unknown_fields() {
    let mut map = payload();
    assert!(resize(&mut map, 3, 2).expect("the map resizes"));
    assert_eq!(map["tiles"], json!([0, null, null, null, 0, null]));
    assert_eq!(map["future"], json!({ "kept": true }));
}

#[test]
fn a_stroke_adds_one_palette_entry_and_erase_writes_null() {
    let mut map = payload();
    assert!(paint(&mut map, 1, 0, TileBrush::Sprite("wall")).expect("paint works"));
    assert!(paint(&mut map, 0, 1, TileBrush::Sprite("wall")).expect("paint works"));
    assert_eq!(map["palette"], json!(["floor", "wall"]));
    assert_eq!(map["tiles"], json!([0, 1, 1, 0]));
    assert!(paint(&mut map, 1, 0, TileBrush::Erase).expect("erase works"));
    assert_eq!(map["tiles"], json!([0, null, 1, 0]));
}

#[test]
fn a_drag_is_one_undoable_world_edit() {
    let original = payload();
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        components: [(super::TYPE_NAME.to_owned(), original.clone())]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    let mut history = CommandHistory::default();

    for (column, row) in [(1, 0), (0, 1)] {
        let mut changed = world
            .get(entity)
            .and_then(|data| data.components.get(super::TYPE_NAME))
            .cloned()
            .expect("the map exists");
        assert!(
            paint(&mut changed, column, row, TileBrush::Sprite("wall")).expect("painting works")
        );
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: super::TYPE_NAME.to_owned(),
            payload: changed,
        });
        history
            .apply(
                buffer
                    .into_transaction("Paint tilemap")
                    .merging("tilemap stroke"),
                &mut world,
            )
            .expect("the cell is written");
    }

    history.undo(&mut world).expect("the stroke undoes");
    assert_eq!(
        world
            .get(entity)
            .and_then(|data| data.components.get(super::TYPE_NAME)),
        Some(&original),
    );
    assert_eq!(history.undo_label(), None, "the whole drag was one step");
}

#[test]
fn a_viewport_click_meets_the_map_in_its_own_space() {
    let map: TilemapComponent = serde_json::from_value(payload()).expect("a map");
    assert_eq!(
        tile_at_viewport(&map, Transform3D::default(), Mat4::IDENTITY, [0.75, 0.75]),
        Some((0, 0))
    );
    let outline = tile_outline(&map, Transform3D::default(), Mat4::IDENTITY, 0, 0)
        .expect("the cell projects");
    assert_eq!(outline, [[0.5, 0.5], [1.0, 0.5], [1.0, 1.0], [0.5, 1.0]]);
}
