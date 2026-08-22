use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
    fmt,
};

use crate::{GridBounds, GridCoord, GridFootprint, GridOccupancy, GridWalls};

/// The neighbours A* may traverse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GridMovement {
    /// Move only across cell edges.
    #[default]
    Cardinal,
    /// Move across edges and corners.
    EightWay {
        /// Whether a diagonal may pass between two blocked cardinal neighbours.
        allow_corner_cutting: bool,
    },
}

/// Integer movement costs used by the pathfinder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridPathCosts {
    cardinal: u32,
    diagonal: u32,
}

impl GridPathCosts {
    pub fn new(cardinal: u32, diagonal: u32) -> Result<Self, GridPathError> {
        if cardinal == 0 {
            return Err(GridPathError::ZeroCardinalCost);
        }
        if diagonal == 0 {
            return Err(GridPathError::ZeroDiagonalCost);
        }
        Ok(Self { cardinal, diagonal })
    }

    #[must_use]
    pub const fn cardinal(self) -> u32 {
        self.cardinal
    }

    #[must_use]
    pub const fn diagonal(self) -> u32 {
        self.diagonal
    }
}

impl Default for GridPathCosts {
    fn default() -> Self {
        Self {
            cardinal: 10,
            diagonal: 14,
        }
    }
}

/// A deterministic A* configuration for a finite grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GridPathfinder {
    movement: GridMovement,
    costs: GridPathCosts,
}

impl GridPathfinder {
    #[must_use]
    pub const fn new(movement: GridMovement, costs: GridPathCosts) -> Self {
        Self { movement, costs }
    }

    #[must_use]
    pub const fn movement(self) -> GridMovement {
        self.movement
    }

    #[must_use]
    pub const fn costs(self) -> GridPathCosts {
        self.costs
    }

    /// Finds the least-cost path from `start` to `goal`.
    ///
    /// The returned path includes both endpoints. An impassable endpoint or a
    /// disconnected goal returns `Ok(None)`. Passability is memoized for the
    /// search, so callers may perform footprint or occupancy validation without
    /// repeating it for every edge.
    pub fn find_path(
        self,
        bounds: GridBounds,
        start: GridCoord,
        goal: GridCoord,
        is_passable: impl FnMut(GridCoord) -> bool,
    ) -> Result<Option<GridPath>, GridPathError> {
        self.find_path_with_transitions(bounds, start, goal, is_passable, |_, _| true)
    }

    /// Finds a path that also respects symmetric walls between cells.
    pub fn find_path_with_walls(
        self,
        bounds: GridBounds,
        walls: &GridWalls,
        start: GridCoord,
        goal: GridCoord,
        is_passable: impl FnMut(GridCoord) -> bool,
    ) -> Result<Option<GridPath>, GridPathError> {
        if walls.bounds() != bounds {
            return Err(GridPathError::WallBoundsMismatch {
                path: bounds,
                walls: walls.bounds(),
            });
        }
        self.find_path_with_transitions(bounds, start, goal, is_passable, |first, second| {
            !walls
                .is_blocked(first, second)
                .expect("pathfinder only checks in-bounds cardinal wall edges")
        })
    }

    fn find_path_with_transitions(
        self,
        bounds: GridBounds,
        start: GridCoord,
        goal: GridCoord,
        mut is_passable: impl FnMut(GridCoord) -> bool,
        mut can_traverse: impl FnMut(GridCoord, GridCoord) -> bool,
    ) -> Result<Option<GridPath>, GridPathError> {
        if !bounds.contains(start) {
            return Err(GridPathError::StartOutsideBounds { start });
        }
        if !bounds.contains(goal) {
            return Err(GridPathError::GoalOutsideBounds { goal });
        }

        let mut passability = BTreeMap::new();
        if !cached_passability(start, &mut passability, &mut is_passable)
            || !cached_passability(goal, &mut passability, &mut is_passable)
        {
            return Ok(None);
        }
        if start == goal {
            return Ok(Some(GridPath {
                nodes: vec![start],
                cost: 0,
            }));
        }

        let mut open = BinaryHeap::new();
        let mut best_cost = BTreeMap::from([(start, 0_u64)]);
        let mut came_from = BTreeMap::new();
        let mut sequence = 0_u64;
        open.push(OpenNode {
            estimated_total: self.heuristic(start, goal),
            cost: 0,
            sequence,
            coord: start,
        });

        while let Some(current) = open.pop() {
            if best_cost.get(&current.coord) != Some(&current.cost) {
                continue;
            }
            if current.coord == goal {
                return Ok(Some(reconstruct_path(
                    start,
                    goal,
                    current.cost,
                    &came_from,
                )));
            }

            for (next, diagonal) in self.neighbours(bounds, current.coord) {
                if !cached_passability(next, &mut passability, &mut is_passable)
                    || !transition_is_open(current.coord, next, diagonal, &mut can_traverse)
                {
                    continue;
                }
                if diagonal
                    && matches!(
                        self.movement,
                        GridMovement::EightWay {
                            allow_corner_cutting: false
                        }
                    )
                {
                    let x_step = GridCoord::new(next.x, current.coord.y);
                    let y_step = GridCoord::new(current.coord.x, next.y);
                    if !cached_passability(x_step, &mut passability, &mut is_passable)
                        || !cached_passability(y_step, &mut passability, &mut is_passable)
                    {
                        continue;
                    }
                }

                let step_cost = u64::from(if diagonal {
                    self.costs.diagonal
                } else {
                    self.costs.cardinal
                });
                let next_cost = current
                    .cost
                    .checked_add(step_cost)
                    .ok_or(GridPathError::CostOverflow)?;
                if best_cost
                    .get(&next)
                    .is_some_and(|known| *known <= next_cost)
                {
                    continue;
                }

                best_cost.insert(next, next_cost);
                came_from.insert(next, current.coord);
                sequence = sequence
                    .checked_add(1)
                    .ok_or(GridPathError::SearchOverflow)?;
                open.push(OpenNode {
                    estimated_total: next_cost
                        .checked_add(self.heuristic(next, goal))
                        .ok_or(GridPathError::CostOverflow)?,
                    cost: next_cost,
                    sequence,
                    coord: next,
                });
            }
        }

        Ok(None)
    }

    fn neighbours(
        self,
        bounds: GridBounds,
        coord: GridCoord,
    ) -> impl Iterator<Item = (GridCoord, bool)> {
        const STEPS: [(i32, i32, bool); 8] = [
            (0, -1, false),
            (1, -1, true),
            (1, 0, false),
            (1, 1, true),
            (0, 1, false),
            (-1, 1, true),
            (-1, 0, false),
            (-1, -1, true),
        ];
        STEPS.into_iter().filter_map(move |(x, y, diagonal)| {
            if diagonal && matches!(self.movement, GridMovement::Cardinal) {
                return None;
            }
            coord
                .checked_offset(x, y)
                .filter(|next| bounds.contains(*next))
                .map(|next| (next, diagonal))
        })
    }

    fn heuristic(self, from: GridCoord, to: GridCoord) -> u64 {
        let x = u64::from(from.x.abs_diff(to.x));
        let y = u64::from(from.y.abs_diff(to.y));
        let cardinal = u64::from(self.costs.cardinal);
        match self.movement {
            GridMovement::Cardinal => (x + y) * cardinal,
            GridMovement::EightWay { .. } => {
                let diagonal_steps = x.min(y);
                let straight_steps = x.max(y) - diagonal_steps;
                let useful_diagonal = u64::from(self.costs.diagonal).min(cardinal * 2);
                diagonal_steps * useful_diagonal + straight_steps * cardinal
            }
        }
    }
}

impl<Owner: Eq> GridOccupancy<Owner> {
    /// Finds an anchor path whose complete footprint can occupy every node.
    ///
    /// Cells already owned by `owner` remain traversable, which lets a placed
    /// object plan from its current anchor without temporarily removing itself.
    pub fn find_path(
        &self,
        pathfinder: GridPathfinder,
        owner: &Owner,
        footprint: &GridFootprint,
        start: GridCoord,
        goal: GridCoord,
    ) -> Result<Option<GridPath>, GridPathError> {
        pathfinder.find_path(self.bounds(), start, goal, |anchor| {
            self.validate(owner, anchor, footprint).is_ok()
        })
    }

    /// Finds a whole-footprint anchor path that also respects wall edges.
    pub fn find_path_with_walls(
        &self,
        pathfinder: GridPathfinder,
        walls: &GridWalls,
        owner: &Owner,
        footprint: &GridFootprint,
        start: GridCoord,
        goal: GridCoord,
    ) -> Result<Option<GridPath>, GridPathError> {
        if walls.bounds() != self.bounds() {
            return Err(GridPathError::WallBoundsMismatch {
                path: self.bounds(),
                walls: walls.bounds(),
            });
        }
        pathfinder.find_path_with_transitions(
            self.bounds(),
            start,
            goal,
            |anchor| self.validate(owner, anchor, footprint).is_ok(),
            |first, second| {
                footprint.offsets().iter().all(|offset| {
                    let first = first
                        .checked_offset(offset.x, offset.y)
                        .expect("a traversable footprint cell cannot overflow");
                    let second = second
                        .checked_offset(offset.x, offset.y)
                        .expect("a traversable footprint cell cannot overflow");
                    !walls
                        .is_blocked(first, second)
                        .expect("a traversable footprint stays inside wall bounds")
                })
            },
        )
    }
}

fn transition_is_open(
    current: GridCoord,
    next: GridCoord,
    diagonal: bool,
    can_traverse: &mut impl FnMut(GridCoord, GridCoord) -> bool,
) -> bool {
    if !diagonal {
        return can_traverse(current, next);
    }
    let x_step = GridCoord::new(next.x, current.y);
    let y_step = GridCoord::new(current.x, next.y);
    can_traverse(current, x_step)
        && can_traverse(current, y_step)
        && can_traverse(x_step, next)
        && can_traverse(y_step, next)
}

fn cached_passability(
    coord: GridCoord,
    cache: &mut BTreeMap<GridCoord, bool>,
    is_passable: &mut impl FnMut(GridCoord) -> bool,
) -> bool {
    if let Some(passable) = cache.get(&coord) {
        return *passable;
    }
    let passable = is_passable(coord);
    cache.insert(coord, passable);
    passable
}

fn reconstruct_path(
    start: GridCoord,
    goal: GridCoord,
    cost: u64,
    came_from: &BTreeMap<GridCoord, GridCoord>,
) -> GridPath {
    let mut nodes = vec![goal];
    let mut current = goal;
    while current != start {
        current = *came_from
            .get(&current)
            .expect("a reached A* node always has a predecessor");
        nodes.push(current);
    }
    nodes.reverse();
    GridPath { nodes, cost }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenNode {
    estimated_total: u64,
    cost: u64,
    sequence: u64,
    coord: GridCoord,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total
            .cmp(&self.estimated_total)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.sequence.cmp(&self.sequence))
            .then_with(|| other.coord.cmp(&self.coord))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A least-cost route including its start and goal cells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridPath {
    nodes: Vec<GridCoord>,
    cost: u64,
}

impl GridPath {
    #[must_use]
    pub fn nodes(&self) -> &[GridCoord] {
        &self.nodes
    }

    #[must_use]
    pub fn into_nodes(self) -> Vec<GridCoord> {
        self.nodes
    }

    #[must_use]
    pub const fn cost(&self) -> u64 {
        self.cost
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridPathError {
    ZeroCardinalCost,
    ZeroDiagonalCost,
    StartOutsideBounds { start: GridCoord },
    GoalOutsideBounds { goal: GridCoord },
    WallBoundsMismatch { path: GridBounds, walls: GridBounds },
    CostOverflow,
    SearchOverflow,
}

impl fmt::Display for GridPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCardinalCost => formatter.write_str("cardinal path cost must be positive"),
            Self::ZeroDiagonalCost => formatter.write_str("diagonal path cost must be positive"),
            Self::StartOutsideBounds { start } => write!(
                formatter,
                "path start ({}, {}) is outside grid bounds",
                start.x, start.y
            ),
            Self::GoalOutsideBounds { goal } => write!(
                formatter,
                "path goal ({}, {}) is outside grid bounds",
                goal.x, goal.y
            ),
            Self::WallBoundsMismatch { path, walls } => write!(
                formatter,
                "path bounds {}x{} do not match wall bounds {}x{}",
                path.width(),
                path.height(),
                walls.width(),
                walls.height()
            ),
            Self::CostOverflow => formatter.write_str("path cost exceeded the u64 range"),
            Self::SearchOverflow => formatter.write_str("path search exceeded the u64 node range"),
        }
    }
}

impl std::error::Error for GridPathError {}

#[cfg(test)]
mod tests {
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
}
