//! What a search finds, what it refuses, and what it costs.

use std::collections::BTreeSet;

use super::*;

#[test]
fn cardinal_search_finds_a_least_cost_detour_deterministically() {
    let bounds = GridBounds::new(4, 3).unwrap();
    let blocked = BTreeSet::from([GridCoord::new(1, 0)]);
    let pathfinder = GridPathfinder::default();
    let search = || {
        pathfinder
            .find_path(bounds, GridCoord::new(0, 0), GridCoord::new(3, 0), |cell| {
                !blocked.contains(&cell)
            })
            .unwrap()
            .unwrap()
    };

    let first = search();
    let second = search();
    assert_eq!(first, second);
    assert_eq!(first.cost(), 50);
    assert_eq!(first.nodes().first(), Some(&GridCoord::new(0, 0)));
    assert_eq!(first.nodes().last(), Some(&GridCoord::new(3, 0)));
    assert!(!first.nodes().contains(&GridCoord::new(1, 0)));
}

#[test]
fn eight_way_search_uses_the_configured_diagonal_cost() {
    let bounds = GridBounds::new(4, 4).unwrap();
    let pathfinder = GridPathfinder::new(
        GridMovement::EightWay {
            allow_corner_cutting: false,
        },
        GridPathCosts::default(),
    );
    let path = pathfinder
        .find_path(bounds, GridCoord::new(0, 0), GridCoord::new(3, 2), |_| true)
        .unwrap()
        .unwrap();

    assert_eq!(path.cost(), 38);
    assert_eq!(path.len(), 4);
}

#[test]
fn corner_cutting_is_an_explicit_policy() {
    let bounds = GridBounds::new(2, 2).unwrap();
    let blocked = BTreeSet::from([GridCoord::new(1, 0), GridCoord::new(0, 1)]);
    let search = |allow_corner_cutting| {
        GridPathfinder::new(
            GridMovement::EightWay {
                allow_corner_cutting,
            },
            GridPathCosts::default(),
        )
        .find_path(bounds, GridCoord::new(0, 0), GridCoord::new(1, 1), |cell| {
            !blocked.contains(&cell)
        })
        .unwrap()
    };

    assert!(search(false).is_none());
    assert_eq!(search(true).unwrap().cost(), 14);
}

#[test]
fn impassable_and_disconnected_goals_return_no_path() {
    let bounds = GridBounds::new(3, 1).unwrap();
    let pathfinder = GridPathfinder::default();

    assert!(
        pathfinder
            .find_path(bounds, GridCoord::new(0, 0), GridCoord::new(2, 0), |cell| {
                cell != GridCoord::new(2, 0)
            })
            .unwrap()
            .is_none()
    );
    assert!(
        pathfinder
            .find_path(bounds, GridCoord::new(0, 0), GridCoord::new(2, 0), |cell| {
                cell != GridCoord::new(1, 0)
            })
            .unwrap()
            .is_none()
    );
}

#[test]
fn endpoints_outside_bounds_are_reported() {
    let bounds = GridBounds::new(2, 2).unwrap();
    let pathfinder = GridPathfinder::default();

    assert_eq!(
        pathfinder.find_path(bounds, GridCoord::new(-1, 0), GridCoord::new(1, 1), |_| {
            true
        }),
        Err(GridPathError::StartOutsideBounds {
            start: GridCoord::new(-1, 0)
        })
    );
    assert_eq!(
        pathfinder.find_path(bounds, GridCoord::new(0, 0), GridCoord::new(2, 1), |_| true),
        Err(GridPathError::GoalOutsideBounds {
            goal: GridCoord::new(2, 1)
        })
    );
}

#[test]
fn a_stationary_path_contains_its_single_endpoint() {
    let bounds = GridBounds::new(1, 1).unwrap();
    let path = GridPathfinder::default()
        .find_path(bounds, GridCoord::new(0, 0), GridCoord::new(0, 0), |_| true)
        .unwrap()
        .unwrap();

    assert_eq!(path.nodes(), &[GridCoord::new(0, 0)]);
    assert_eq!(path.cost(), 0);
}

#[test]
fn occupancy_search_validates_the_whole_moving_footprint() {
    let bounds = GridBounds::new(5, 3).unwrap();
    let mut occupancy = GridOccupancy::new(bounds);
    let mover = GridFootprint::rectangle(2, 1).unwrap();
    occupancy.place(1, GridCoord::new(0, 0), &mover).unwrap();
    occupancy
        .place(2, GridCoord::new(2, 0), &GridFootprint::single())
        .unwrap();

    let path = occupancy
        .find_path(
            GridPathfinder::default(),
            &1,
            &mover,
            GridCoord::new(0, 0),
            GridCoord::new(3, 0),
        )
        .unwrap()
        .unwrap();

    assert_eq!(path.cost(), 50);
    assert!(path.nodes().contains(&GridCoord::new(2, 1)));
    assert!(!path.nodes().contains(&GridCoord::new(1, 0)));
}

#[test]
fn cardinal_paths_detour_around_walls() {
    let bounds = GridBounds::new(3, 2).unwrap();
    let mut walls = GridWalls::new(bounds);
    walls
        .block(GridCoord::new(0, 0), GridCoord::new(1, 0))
        .unwrap();

    let path = GridPathfinder::default()
        .find_path_with_walls(
            bounds,
            &walls,
            GridCoord::new(0, 0),
            GridCoord::new(2, 0),
            |_| true,
        )
        .unwrap()
        .unwrap();

    assert_eq!(path.cost(), 40);
    assert!(path.nodes().contains(&GridCoord::new(0, 1)));
}

#[test]
fn diagonals_cannot_cross_a_wall_corner() {
    let bounds = GridBounds::new(2, 2).unwrap();
    let mut walls = GridWalls::new(bounds);
    walls
        .block(GridCoord::new(0, 0), GridCoord::new(1, 0))
        .unwrap();
    let pathfinder = GridPathfinder::new(
        GridMovement::EightWay {
            allow_corner_cutting: true,
        },
        GridPathCosts::default(),
    );

    let path = pathfinder
        .find_path_with_walls(
            bounds,
            &walls,
            GridCoord::new(0, 0),
            GridCoord::new(1, 1),
            |_| true,
        )
        .unwrap()
        .unwrap();

    assert_eq!(path.cost(), 20);
}

#[test]
fn wall_and_path_bounds_must_match() {
    let path_bounds = GridBounds::new(2, 2).unwrap();
    let wall_bounds = GridBounds::new(3, 2).unwrap();
    assert_eq!(
        GridPathfinder::default().find_path_with_walls(
            path_bounds,
            &GridWalls::new(wall_bounds),
            GridCoord::new(0, 0),
            GridCoord::new(1, 1),
            |_| true,
        ),
        Err(GridPathError::WallBoundsMismatch {
            path: path_bounds,
            walls: wall_bounds
        })
    );
}

#[test]
fn occupancy_paths_can_respect_footprints_and_walls_together() {
    let bounds = GridBounds::new(5, 2).unwrap();
    let mut occupancy = GridOccupancy::new(bounds);
    let footprint = GridFootprint::rectangle(2, 1).unwrap();
    occupancy
        .place(1, GridCoord::new(0, 0), &footprint)
        .unwrap();
    let mut walls = GridWalls::new(bounds);
    walls
        .block(GridCoord::new(1, 0), GridCoord::new(2, 0))
        .unwrap();

    let path = occupancy
        .find_path_with_walls(
            GridPathfinder::default(),
            &walls,
            &1,
            &footprint,
            GridCoord::new(0, 0),
            GridCoord::new(3, 0),
        )
        .unwrap()
        .unwrap();

    assert_eq!(path.cost(), 50);
    assert!(path.nodes().contains(&GridCoord::new(0, 1)));
    assert!(!path.nodes().contains(&GridCoord::new(1, 0)));
}

#[test]
fn movement_costs_must_be_positive() {
    assert_eq!(
        GridPathCosts::new(0, 14),
        Err(GridPathError::ZeroCardinalCost)
    );
    assert_eq!(
        GridPathCosts::new(10, 0),
        Err(GridPathError::ZeroDiagonalCost)
    );
}
