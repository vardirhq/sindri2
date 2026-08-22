use std::{collections::BTreeMap, fmt};

use crate::{GridBounds, GridCoord};

/// The cells an object covers, expressed as offsets from its anchor cell.
///
/// Offsets are stored once in deterministic row-major order. A footprint may
/// extend in any direction around its anchor, but it may not be empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridFootprint {
    offsets: Vec<GridCoord>,
}

impl GridFootprint {
    /// Creates an arbitrary footprint, sorting and removing repeated offsets.
    pub fn new(offsets: impl IntoIterator<Item = GridCoord>) -> Result<Self, FootprintError> {
        let mut offsets = offsets.into_iter().collect::<Vec<_>>();
        offsets.sort_unstable_by_key(|offset| (offset.y, offset.x));
        offsets.dedup();
        if offsets.is_empty() {
            return Err(FootprintError::Empty);
        }
        Ok(Self { offsets })
    }

    /// A footprint containing only its anchor cell.
    #[must_use]
    pub fn single() -> Self {
        Self {
            offsets: vec![GridCoord::default()],
        }
    }

    /// A rectangle extending right and down from its anchor.
    pub fn rectangle(width: u32, height: u32) -> Result<Self, FootprintError> {
        if width == 0 || height == 0 {
            return Err(FootprintError::Empty);
        }
        let width = i32::try_from(width)
            .map_err(|_| FootprintError::DimensionsOutsideIntegerRange { width, height })?;
        let height =
            i32::try_from(height).map_err(|_| FootprintError::DimensionsOutsideIntegerRange {
                width: u32::try_from(width).expect("the width was checked above"),
                height,
            })?;
        Ok(Self {
            offsets: (0..height)
                .flat_map(|y| (0..width).map(move |x| GridCoord::new(x, y)))
                .collect(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    #[must_use]
    pub fn offsets(&self) -> &[GridCoord] {
        &self.offsets
    }
}

impl Default for GridFootprint {
    fn default() -> Self {
        Self::single()
    }
}

/// A finite grid whose occupied cells identify their owning object.
///
/// An owner has at most one placement. Re-placing it validates the complete
/// destination before releasing its previous cells, so failed moves never
/// partially mutate occupancy.
#[derive(Clone, Debug)]
pub struct GridOccupancy<Owner> {
    bounds: GridBounds,
    cells: BTreeMap<GridCoord, Owner>,
}

impl<Owner> GridOccupancy<Owner> {
    #[must_use]
    pub fn new(bounds: GridBounds) -> Self {
        Self {
            bounds,
            cells: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn bounds(&self) -> GridBounds {
        self.bounds
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[must_use]
    pub fn occupant(&self, coord: GridCoord) -> Option<&Owner> {
        self.cells.get(&coord)
    }

    pub fn iter(&self) -> impl Iterator<Item = (GridCoord, &Owner)> {
        self.cells.iter().map(|(coord, owner)| (*coord, owner))
    }
}

impl<Owner: Eq> GridOccupancy<Owner> {
    /// Checks a placement without changing occupancy.
    pub fn validate(
        &self,
        owner: &Owner,
        anchor: GridCoord,
        footprint: &GridFootprint,
    ) -> Result<(), GridPlacementError> {
        self.placement_cells(owner, anchor, footprint).map(|_| ())
    }

    /// Removes every cell belonging to an owner.
    pub fn remove(&mut self, owner: &Owner) -> usize {
        let previous_len = self.cells.len();
        self.cells.retain(|_, occupant| occupant != owner);
        previous_len - self.cells.len()
    }

    fn placement_cells(
        &self,
        owner: &Owner,
        anchor: GridCoord,
        footprint: &GridFootprint,
    ) -> Result<Vec<GridCoord>, GridPlacementError> {
        footprint
            .offsets()
            .iter()
            .copied()
            .map(|offset| {
                let cell = anchor
                    .checked_offset(offset.x, offset.y)
                    .ok_or(GridPlacementError::CoordinateOverflow { anchor, offset })?;
                if !self.bounds.contains(cell) {
                    return Err(GridPlacementError::OutsideBounds { cell });
                }
                if self
                    .cells
                    .get(&cell)
                    .is_some_and(|occupant| occupant != owner)
                {
                    return Err(GridPlacementError::Occupied { cell });
                }
                Ok(cell)
            })
            .collect()
    }
}

impl<Owner: Clone + Eq> GridOccupancy<Owner> {
    /// Places or atomically moves an owner to the requested footprint.
    pub fn place(
        &mut self,
        owner: Owner,
        anchor: GridCoord,
        footprint: &GridFootprint,
    ) -> Result<(), GridPlacementError> {
        let cells = self.placement_cells(&owner, anchor, footprint)?;
        self.remove(&owner);
        let (last, preceding) = cells
            .split_last()
            .expect("a validated footprint always contains a cell");
        for cell in preceding {
            self.cells.insert(*cell, owner.clone());
        }
        self.cells.insert(*last, owner);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FootprintError {
    Empty,
    DimensionsOutsideIntegerRange { width: u32, height: u32 },
}

impl fmt::Display for FootprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a grid footprint must contain at least one cell"),
            Self::DimensionsOutsideIntegerRange { width, height } => write!(
                formatter,
                "grid footprint dimensions {width}x{height} exceed the signed coordinate range"
            ),
        }
    }
}

impl std::error::Error for FootprintError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridPlacementError {
    CoordinateOverflow {
        anchor: GridCoord,
        offset: GridCoord,
    },
    OutsideBounds {
        cell: GridCoord,
    },
    Occupied {
        cell: GridCoord,
    },
}

impl fmt::Display for GridPlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOverflow { anchor, offset } => write!(
                formatter,
                "grid footprint offset ({}, {}) overflows anchor ({}, {})",
                offset.x, offset.y, anchor.x, anchor.y
            ),
            Self::OutsideBounds { cell } => write!(
                formatter,
                "grid footprint cell ({}, {}) is outside occupancy bounds",
                cell.x, cell.y
            ),
            Self::Occupied { cell } => write!(
                formatter,
                "grid footprint cell ({}, {}) is already occupied",
                cell.x, cell.y
            ),
        }
    }
}

impl std::error::Error for GridPlacementError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_footprints_are_normalized_in_row_major_order() {
        let footprint = GridFootprint::new([
            GridCoord::new(1, 0),
            GridCoord::new(0, 1),
            GridCoord::new(0, 0),
            GridCoord::new(1, 0),
        ])
        .unwrap();
        assert_eq!(
            footprint.offsets(),
            &[
                GridCoord::new(0, 0),
                GridCoord::new(1, 0),
                GridCoord::new(0, 1),
            ]
        );
    }

    #[test]
    fn rectangles_extend_right_and_down_from_the_anchor() {
        let footprint = GridFootprint::rectangle(2, 2).unwrap();
        assert_eq!(
            footprint.offsets(),
            &[
                GridCoord::new(0, 0),
                GridCoord::new(1, 0),
                GridCoord::new(0, 1),
                GridCoord::new(1, 1),
            ]
        );
        assert!(matches!(
            GridFootprint::rectangle(0, 2),
            Err(FootprintError::Empty)
        ));
    }

    #[test]
    fn overlapping_owners_are_rejected_without_partial_mutation() {
        let bounds = GridBounds::new(4, 4).unwrap();
        let mut occupancy = GridOccupancy::new(bounds);
        let footprint = GridFootprint::rectangle(2, 2).unwrap();
        occupancy
            .place("first", GridCoord::new(0, 0), &footprint)
            .unwrap();

        assert_eq!(
            occupancy.place("second", GridCoord::new(1, 1), &footprint),
            Err(GridPlacementError::Occupied {
                cell: GridCoord::new(1, 1)
            })
        );
        assert_eq!(occupancy.len(), 4);
        assert!(occupancy.iter().all(|(_, owner)| *owner == "first"));
    }

    #[test]
    fn an_owner_moves_atomically_and_releases_its_old_cells() {
        let bounds = GridBounds::new(5, 3).unwrap();
        let mut occupancy = GridOccupancy::new(bounds);
        let footprint = GridFootprint::rectangle(2, 1).unwrap();
        occupancy
            .place(7, GridCoord::new(0, 0), &footprint)
            .unwrap();
        occupancy
            .place(7, GridCoord::new(2, 1), &footprint)
            .unwrap();

        assert_eq!(occupancy.occupant(GridCoord::new(0, 0)), None);
        assert_eq!(occupancy.occupant(GridCoord::new(1, 0)), None);
        assert_eq!(occupancy.occupant(GridCoord::new(2, 1)), Some(&7));
        assert_eq!(occupancy.occupant(GridCoord::new(3, 1)), Some(&7));
    }

    #[test]
    fn failed_moves_keep_the_previous_placement() {
        let bounds = GridBounds::new(3, 2).unwrap();
        let mut occupancy = GridOccupancy::new(bounds);
        let footprint = GridFootprint::rectangle(2, 1).unwrap();
        occupancy
            .place(7, GridCoord::new(0, 0), &footprint)
            .unwrap();

        assert_eq!(
            occupancy.place(7, GridCoord::new(2, 1), &footprint),
            Err(GridPlacementError::OutsideBounds {
                cell: GridCoord::new(3, 1)
            })
        );
        assert_eq!(occupancy.occupant(GridCoord::new(0, 0)), Some(&7));
        assert_eq!(occupancy.occupant(GridCoord::new(1, 0)), Some(&7));
    }

    #[test]
    fn negative_offsets_and_coordinate_overflow_are_checked() {
        let bounds = GridBounds::new(3, 3).unwrap();
        let occupancy = GridOccupancy::<u8>::new(bounds);
        let footprint = GridFootprint::new([GridCoord::new(-1, 0)]).unwrap();
        assert_eq!(
            occupancy.validate(&1, GridCoord::new(0, 0), &footprint),
            Err(GridPlacementError::OutsideBounds {
                cell: GridCoord::new(-1, 0)
            })
        );
        assert_eq!(
            occupancy.validate(&1, GridCoord::new(i32::MIN, 0), &footprint),
            Err(GridPlacementError::CoordinateOverflow {
                anchor: GridCoord::new(i32::MIN, 0),
                offset: GridCoord::new(-1, 0)
            })
        );
    }

    #[test]
    fn removing_an_owner_reports_released_cells() {
        let bounds = GridBounds::new(2, 2).unwrap();
        let mut occupancy = GridOccupancy::new(bounds);
        occupancy
            .place(
                3,
                GridCoord::new(0, 0),
                &GridFootprint::rectangle(2, 1).unwrap(),
            )
            .unwrap();
        assert_eq!(occupancy.remove(&3), 2);
        assert!(occupancy.is_empty());
        assert_eq!(occupancy.remove(&3), 0);
    }
}
