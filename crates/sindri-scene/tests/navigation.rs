use std::collections::BTreeMap;

use serde_json::{Value, json};
use sindri_core::{
    SCENE_FORMAT_VERSION, SceneComponent, SceneDocument, SceneEntity, SceneEntityId, Transform3D,
    World,
};
use sindri_grid::{GridCoord, GridPathfinder};
use sindri_scene::{
    GridNavigationComponent, GridNavigationError, GridOccupantComponent, SceneExtractor,
    WorldGridNavigation,
};

fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("test IDs are non-empty")
}

fn entity(
    entity_id: &str,
    position: Option<[f32; 3]>,
    components: impl IntoIterator<Item = (&'static str, Value)>,
) -> SceneEntity {
    let mut entity = SceneEntity::new(id(entity_id));
    entity.transform_3d = position.map(|position| Transform3D {
        position,
        ..Transform3D::default()
    });
    entity.components = components
        .into_iter()
        .map(|(name, payload)| (name.to_owned(), payload))
        .collect::<BTreeMap<_, _>>();
    entity
}

fn tilemap(columns: u32, rows: u32) -> Value {
    json!({
        "texture": "tiles",
        "palette": [],
        "columns": columns,
        "rows": rows,
        "tiles": vec![Value::Null; (columns * rows) as usize],
        "space": "world"
    })
}

fn world(entities: Vec<SceneEntity>) -> sindri_core::LoadedScene {
    World::from_scene(&SceneDocument {
        format_version: SCENE_FORMAT_VERSION,
        entities,
        ..SceneDocument::default()
    })
    .expect("test scene loads")
}

#[test]
fn built_in_navigation_components_are_registered_with_honest_defaults() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    assert_eq!(
        extractor
            .components()
            .default_payload(GridNavigationComponent::TYPE_NAME),
        Some(&json!({ "walls": [] }))
    );
    assert!(
        extractor
            .components()
            .default_payload(GridOccupantComponent::TYPE_NAME)
            .is_none(),
        "an occupant default cannot invent the stable ID of its grid"
    );
}

#[test]
fn a_snapshot_derives_footprints_and_walls_from_authored_components() {
    let mut floor = entity(
        "floor",
        Some([10.0, 20.0, 0.0]),
        [
            ("sindri.tilemap", tilemap(4, 3)),
            (
                "sindri.grid.navigation",
                json!({ "walls": [{ "first": [1, 1], "second": [1, 2] }] }),
            ),
        ],
    );
    let mut floor_transform = floor.transform_3d.expect("floor is positioned");
    floor_transform.set_rotation_z_radians(std::f32::consts::FRAC_PI_2);
    floor_transform.set_scale_2d([2.0, 3.0]);
    floor.transform_3d = Some(floor_transform);

    // Orthogonal cell (1, 1) is local (1.5, -1.5). Scale then turn it a
    // quarter-turn around the translated floor to get this world position.
    let actor = entity(
        "actor",
        Some([14.5, 23.0, 7.0]),
        [(
            "sindri.grid.occupant",
            json!({
                "grid": "floor",
                "footprint": [[0, 0], [1, 0]]
            }),
        )],
    );
    let loaded = world(vec![floor, actor]);
    let floor_id = loaded.entity_map[&id("floor")];
    let actor_id = loaded.entity_map[&id("actor")];

    let navigation = WorldGridNavigation::from_world(&loaded.world, floor_id)
        .expect("authored navigation derives");
    let placement = navigation.placement(actor_id).expect("actor is placed");
    assert_eq!(placement.anchor, GridCoord::new(1, 1));
    assert_eq!(
        navigation.occupancy().occupant(GridCoord::new(1, 1)),
        Some(&actor_id)
    );
    assert_eq!(
        navigation.occupancy().occupant(GridCoord::new(2, 1)),
        Some(&actor_id)
    );
    assert!(
        navigation
            .walls()
            .is_blocked(GridCoord::new(1, 2), GridCoord::new(1, 1))
            .expect("wall query is in bounds"),
        "authored walls are symmetric after normalization"
    );
}

#[test]
fn path_queries_apply_occupancy_and_walls_together() {
    let open_floor = entity(
        "floor",
        Some([0.0, 0.0, 0.0]),
        [("sindri.tilemap", tilemap(4, 2))],
    );
    let actor = entity(
        "actor",
        Some([0.5, -0.5, 0.0]),
        [("sindri.grid.occupant", json!({ "grid": "floor" }))],
    );
    let blocker = entity(
        "blocker",
        Some([1.5, -0.5, 0.0]),
        [("sindri.grid.occupant", json!({ "grid": "floor" }))],
    );
    let open = world(vec![open_floor, actor.clone(), blocker.clone()]);
    let open_navigation =
        WorldGridNavigation::from_world(&open.world, open.entity_map[&id("floor")])
            .expect("open navigation derives");
    assert!(
        open_navigation
            .find_path(
                GridPathfinder::default(),
                open.entity_map[&id("actor")],
                GridCoord::new(3, 0),
            )
            .expect("open path query is valid")
            .is_some(),
        "without a wall the actor can route around its occupied neighbour"
    );

    let blocked_floor = entity(
        "floor",
        Some([0.0, 0.0, 0.0]),
        [
            ("sindri.tilemap", tilemap(4, 2)),
            (
                "sindri.grid.navigation",
                json!({ "walls": [{ "first": [0, 0], "second": [0, 1] }] }),
            ),
        ],
    );
    let loaded = world(vec![blocked_floor, actor, blocker]);
    let navigation =
        WorldGridNavigation::from_world(&loaded.world, loaded.entity_map[&id("floor")])
            .expect("navigation derives");

    assert_eq!(
        navigation
            .find_path(
                GridPathfinder::default(),
                loaded.entity_map[&id("actor")],
                GridCoord::new(3, 0),
            )
            .expect("path query is valid"),
        None,
        "the wall closes the only route around the occupied neighbour"
    );
}

#[test]
fn one_explicit_grid_does_not_collect_another_grids_occupants() {
    let first_grid = entity(
        "first_grid",
        Some([0.0, 0.0, 0.0]),
        [("sindri.tilemap", tilemap(2, 2))],
    );
    let second_grid = entity(
        "second_grid",
        Some([10.0, 0.0, 0.0]),
        [("sindri.tilemap", tilemap(2, 2))],
    );
    let first_actor = entity(
        "first_actor",
        Some([0.5, -0.5, 0.0]),
        [("sindri.grid.occupant", json!({ "grid": "first_grid" }))],
    );
    let second_actor = entity(
        "second_actor",
        Some([10.5, -0.5, 0.0]),
        [("sindri.grid.occupant", json!({ "grid": "second_grid" }))],
    );
    let loaded = world(vec![first_grid, second_grid, first_actor, second_actor]);
    let navigation =
        WorldGridNavigation::from_world(&loaded.world, loaded.entity_map[&id("first_grid")])
            .expect("the selected grid derives");

    assert_eq!(navigation.placements().count(), 1);
    assert!(
        navigation
            .placement(loaded.entity_map[&id("first_actor")])
            .is_some()
    );
    assert!(
        navigation
            .placement(loaded.entity_map[&id("second_actor")])
            .is_none()
    );
}

#[test]
fn conflicting_authored_placements_are_rejected_as_one_invalid_snapshot() {
    let floor = entity(
        "floor",
        Some([0.0, 0.0, 0.0]),
        [("sindri.tilemap", tilemap(2, 2))],
    );
    let first = entity(
        "first",
        Some([0.5, -0.5, 0.0]),
        [("sindri.grid.occupant", json!({ "grid": "floor" }))],
    );
    let second = entity(
        "second",
        Some([0.5, -0.5, 0.0]),
        [("sindri.grid.occupant", json!({ "grid": "floor" }))],
    );
    let loaded = world(vec![floor, first, second]);

    let error = WorldGridNavigation::from_world(&loaded.world, loaded.entity_map[&id("floor")])
        .expect_err("overlapping authored occupants cannot form a snapshot");
    assert!(matches!(
        &error,
        GridNavigationError::InvalidPlacement { .. }
    ));
    assert!(error.to_string().contains("already occupied"));
}

#[test]
fn invalid_authored_wall_endpoints_name_the_wall_and_grid() {
    let floor = entity(
        "floor",
        Some([0.0, 0.0, 0.0]),
        [
            ("sindri.tilemap", tilemap(2, 2)),
            (
                "sindri.grid.navigation",
                json!({ "walls": [{ "first": [0, 0], "second": [1, 1] }] }),
            ),
        ],
    );
    let loaded = world(vec![floor]);
    let floor_id = loaded.entity_map[&id("floor")];

    let error = WorldGridNavigation::from_world(&loaded.world, floor_id)
        .expect_err("diagonal cells do not define one wall edge");
    assert!(matches!(
        error,
        GridNavigationError::InvalidWall {
            grid,
            index: 0,
            ..
        } if grid == floor_id
    ));
}
