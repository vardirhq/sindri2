//! Projection, its inverse, and the cell a point falls in.

use super::*;

const EPSILON: f64 = 1.0e-10;

fn assert_point(actual: PlanePoint, expected: PlanePoint) {
    assert!((actual.x - expected.x).abs() < EPSILON, "x: {actual:?}");
    assert!((actual.y - expected.y).abs() < EPSILON, "y: {actual:?}");
}

#[test]
fn orthogonal_projection_uses_full_cell_steps() {
    let grid = GridSpace::new(Projection::Orthogonal, 64.0, 32.0).unwrap();
    assert_point(
        grid.grid_to_plane(GridCoord::new(3, -2)).unwrap(),
        PlanePoint::new(192.0, -64.0),
    );
}

#[test]
fn isometric_projection_uses_half_cell_diagonals() {
    let grid = GridSpace::new(Projection::Isometric, 64.0, 32.0).unwrap();
    assert_point(
        grid.grid_to_plane(GridCoord::new(3, 2)).unwrap(),
        PlanePoint::new(32.0, 80.0),
    );
}

#[test]
fn origin_moves_both_projection_directions_together() {
    let grid = GridSpace::with_origin(
        Projection::Isometric,
        64.0,
        32.0,
        PlanePoint::new(400.0, 200.0),
    )
    .unwrap();
    assert_point(
        grid.grid_to_plane(GridCoord::new(1, 0)).unwrap(),
        PlanePoint::new(432.0, 216.0),
    );
}

#[test]
fn upward_plane_y_flips_both_projections_without_changing_the_inverse() {
    for projection in [Projection::Orthogonal, Projection::Isometric] {
        let down = GridSpace::with_origin_and_y_axis(
            projection,
            64.0,
            32.0,
            PlanePoint::new(10.0, 20.0),
            PlaneYAxis::Down,
        )
        .unwrap();
        let up = GridSpace::with_origin_and_y_axis(
            projection,
            64.0,
            32.0,
            PlanePoint::new(10.0, 20.0),
            PlaneYAxis::Up,
        )
        .unwrap();
        let coord = GridCoord::new(3, 2);
        let down_point = down.grid_to_plane(coord).unwrap();
        let up_point = up.grid_to_plane(coord).unwrap();
        assert!((down_point.x - up_point.x).abs() < EPSILON);
        assert!((down_point.y + up_point.y - 40.0).abs() < EPSILON);
        assert_eq!(up.plane_to_grid(up_point).unwrap(), coord);
    }
}

#[test]
fn both_projections_round_trip_cells_across_negative_and_positive_space() {
    for projection in [Projection::Orthogonal, Projection::Isometric] {
        let grid =
            GridSpace::with_origin(projection, 63.5, 29.25, PlanePoint::new(173.0, -91.0)).unwrap();
        for y in -128..=128 {
            for x in -128..=128 {
                let coord = GridCoord::new(x, y);
                let plane = grid.grid_to_plane(coord).unwrap();
                assert_eq!(grid.plane_to_grid(plane).unwrap(), coord);
            }
        }
    }
}

#[test]
fn continuous_projection_is_its_own_inverse() {
    let points = [
        GridPoint::new(-100.25, 37.75),
        GridPoint::new(-0.49, 0.49),
        GridPoint::new(0.0, 0.0),
        GridPoint::new(17.125, -9.875),
    ];
    for projection in [Projection::Orthogonal, Projection::Isometric] {
        let grid = GridSpace::new(projection, 57.0, 31.0).unwrap();
        for point in points {
            let actual = grid.unproject(grid.project(point).unwrap()).unwrap();
            assert!((actual.x - point.x).abs() < EPSILON);
            assert!((actual.y - point.y).abs() < EPSILON);
        }
    }
}

#[test]
fn a_point_inside_each_cell_resolves_to_that_cell() {
    for projection in [Projection::Orthogonal, Projection::Isometric] {
        let grid = GridSpace::new(projection, 64.0, 32.0).unwrap();
        for y in -16..=16 {
            for x in -16..=16 {
                let expected = GridCoord::new(x, y);
                for offset in [
                    GridPoint::new(-0.49, -0.49),
                    GridPoint::new(0.49, -0.49),
                    GridPoint::new(-0.49, 0.49),
                    GridPoint::new(0.49, 0.49),
                ] {
                    let sample = GridPoint::new(f64::from(x) + offset.x, f64::from(y) + offset.y);
                    assert_eq!(
                        grid.plane_to_grid(grid.project(sample).unwrap()).unwrap(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn a_half_cell_boundary_belongs_to_the_positive_neighbour() {
    for projection in [Projection::Orthogonal, Projection::Isometric] {
        let grid = GridSpace::new(projection, 64.0, 32.0).unwrap();
        assert_eq!(
            grid.plane_to_grid(grid.project(GridPoint::new(-0.5, -0.5)).unwrap())
                .unwrap(),
            GridCoord::new(0, 0)
        );
        assert_eq!(
            grid.plane_to_grid(grid.project(GridPoint::new(0.5, 0.5)).unwrap())
                .unwrap(),
            GridCoord::new(1, 1)
        );
    }
}

#[test]
fn bounded_neighbours_never_leave_the_grid() {
    let bounds = GridBounds::new(3, 2).unwrap();
    assert_eq!(
        bounds
            .cardinal_neighbours(GridCoord::new(0, 0))
            .collect::<Vec<_>>(),
        vec![GridCoord::new(1, 0), GridCoord::new(0, 1)]
    );
    assert_eq!(
        bounds
            .surrounding_neighbours(GridCoord::new(2, 1))
            .collect::<Vec<_>>(),
        vec![
            GridCoord::new(1, 0),
            GridCoord::new(2, 0),
            GridCoord::new(1, 1)
        ]
    );
}

#[test]
fn bounds_iterate_in_row_major_order() {
    assert_eq!(
        GridBounds::new(2, 2).unwrap().iter().collect::<Vec<_>>(),
        vec![
            GridCoord::new(0, 0),
            GridCoord::new(1, 0),
            GridCoord::new(0, 1),
            GridCoord::new(1, 1),
        ]
    );
}

#[test]
fn coordinate_neighbours_do_not_overflow() {
    assert_eq!(GridCoord::new(i32::MAX, 0).cardinal_neighbours().count(), 3);
    assert_eq!(
        GridCoord::new(i32::MIN, i32::MIN)
            .surrounding_neighbours()
            .count(),
        3
    );
}

#[test]
fn invalid_projection_inputs_are_rejected() {
    assert!(matches!(
        GridSpace::new(Projection::Orthogonal, 0.0, 1.0),
        Err(GridError::InvalidCellWidth(0.0))
    ));
    assert!(matches!(
        GridSpace::new(Projection::Orthogonal, 1.0, f64::NAN),
        Err(GridError::InvalidCellHeight(value)) if value.is_nan()
    ));
    let grid = GridSpace::new(Projection::Orthogonal, 1.0, 1.0).unwrap();
    assert!(matches!(
        grid.plane_to_grid(PlanePoint::new(f64::INFINITY, 0.0)),
        Err(GridError::NonFinitePoint(_))
    ));
    assert!(matches!(
        GridBounds::new(u32::MAX, 1),
        Err(GridError::BoundsOutsideIntegerRange { .. })
    ));
}

#[test]
fn arbitrary_continuous_points_round_trip_on_the_plane() {
    let grid = GridSpace::with_origin(
        Projection::Isometric,
        70.0,
        34.0,
        PlanePoint::new(900.0, 450.0),
    )
    .unwrap();
    let expected = PlanePoint::new(947.25, 412.75);
    assert_point(
        grid.project(grid.unproject(expected).unwrap()).unwrap(),
        expected,
    );
}
