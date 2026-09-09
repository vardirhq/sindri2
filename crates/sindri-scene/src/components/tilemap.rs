//! `sindri.tilemap`: a grid of sprites, and the projection it is laid out on.

use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_grid::{
    GridBounds, GridCoord, GridError, GridSpace, PlanePoint, PlaneYAxis, Projection,
};
use thiserror::Error;

use super::opaque_white;

/// How a tilemap's grid coordinates become world positions.
///
/// The serialized choice is owned by the tilemap; its coordinate maths is
/// supplied by `sindri-grid`, so rendering, picking, and gameplay can share one
/// meaning of a cell.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TileProjection {
    /// Columns run +X and rows run -Y, so the map reads like the array does.
    #[default]
    Orthogonal,
    /// Columns and rows run along the two diagonals, which is what makes a
    /// square grid look like a diamond floor. A tile's world size stays its
    /// full width and height; it is the *step* between tiles that halves, so
    /// neighbours overlap the way isometric art expects.
    Isometric,
}

/// A grid of tiles drawn from one sheet, as one batch, from one entity.
///
/// The point is not draw calls — loose sprites sharing a texture already batch
/// into one. It is that a floor stops being one entity per tile: 49 entities,
/// each with a transform, a name, a stable ID, and a sprite component, become
/// one component holding 49 small integers. That is the difference between a
/// scene file a person can read and one they cannot, and between a hierarchy
/// they can find the player in and one they cannot.
///
/// Variation comes from picking different cells of the sheet, not from tinting
/// each tile: the tint is the map's, because a per-tile tint is a second way to
/// say what a second tile already says.
///
/// A map is in the world. It used to carry the `space` a sprite carried, and
/// defaulted to the screen, so the ordinary case — a floor — had to say it was
/// not a HUD. Nothing authored a screen-space map, and a viewport-anchored grid
/// of tiles is a UI element rather than a tilemap, so the field is gone and
/// `sindri.ui.*` is where the overlay lives.
impl TileProjection {
    /// Every projection, by the name a scene stores, in the order a chooser
    /// should offer them.
    pub const ALL: [Self; 2] = [Self::Orthogonal, Self::Isometric];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Orthogonal => "orthogonal",
            Self::Isometric => "isometric",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TilemapComponent {
    /// The sliced image every tile is drawn from.
    pub texture: String,
    /// The sprites this map uses, by the names its sheet gives them.
    ///
    /// `tiles` indexes into *this*, not into the sheet, which is what keeps a
    /// 49-cell map 49 small integers instead of 49 repeated strings. It is also
    /// what makes a re-slice survivable: the sheet can move `floor` to another
    /// cell and every map using it still draws the right thing, because a map
    /// names sprites and the sheet places them.
    pub palette: Vec<String>,
    /// The map's size in tiles.
    pub columns: u32,
    pub rows: u32,
    /// One tile's size in world units.
    #[serde(default = "unit_tile")]
    pub tile_size: [f32; 2],
    /// How far below its cell a tile's art reaches, in world units.
    ///
    /// A flat floor is drawn tile-sized and needs none. A floor of *slabs* does:
    /// the top face still covers exactly one cell, and the sides that make it
    /// look like a slab hang below into the cells in front of it. Without this a
    /// slab would have to be squashed into its cell, which would make it a
    /// picture of a slab painted on flat ground.
    ///
    /// It is an overhang rather than a second size, because the cell is what
    /// everything else agrees on: the grid maths, picking, occupancy and
    /// gameplay all measure in cells, and a tile that drew larger without saying
    /// it was *hanging* would quietly move all of them.
    ///
    /// Zero by default, so a map that says nothing is the flat map it was.
    #[serde(default)]
    pub tile_overhang: f32,
    #[serde(default)]
    pub projection: TileProjection,
    /// `columns * rows` cells, row-major from the top-left, `null` where the
    /// map has no tile and otherwise an index into `palette`.
    ///
    /// Null rather than a sentinel index, because every index is a real tile:
    /// reserving 0 or -1 to mean "empty" is how a map ends up with an
    /// accidental floor in the corner nobody authored.
    pub tiles: Vec<Option<u32>>,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    #[serde(default)]
    pub layer: i32,
}

const fn unit_tile() -> [f32; 2] {
    [1.0, 1.0]
}

/// The quad a tile is drawn on, which is its cell unless it hangs below one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileDraw {
    /// Width and height in world units.
    pub size: [f32; 2],
    /// How far the quad's centre sits below the cell's.
    pub offset_y: f32,
}

/// What is wrong with a tilemap, named specifically enough to fix.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TilemapError {
    #[error(transparent)]
    Grid(#[from] GridError),
    #[error("a tile's overhang reaches below its cell, so it cannot be {0}")]
    NegativeOverhang(f32),
    #[error(
        "tilemap is {columns}x{rows} tiles, which needs {expected} cells, but {actual} were given"
    )]
    WrongCellCount {
        columns: u32,
        rows: u32,
        expected: usize,
        actual: usize,
    },
    #[error(
        "tile {index} at column {column}, row {row} is not one of the {palette} sprites in the map's palette"
    )]
    TileOutsidePalette {
        column: u32,
        row: u32,
        index: u32,
        palette: usize,
    },
    #[error("a tilemap that draws anything needs at least one sprite in its palette")]
    EmptyPalette,
}

impl TilemapComponent {
    /// How many cells the map's size calls for.
    #[must_use]
    pub const fn expected_cells(&self) -> usize {
        self.columns as usize * self.rows as usize
    }

    /// Checks that the map is the shape it claims and every tile exists in the
    /// sheet.
    ///
    /// Checked here rather than at deserialization for the reason a bad UV rect
    /// is: a scene carrying a broken tilemap has to open, because the editor is
    /// where it gets fixed.
    /// The quad one tile is drawn on: the cell, plus whatever hangs below it.
    ///
    /// Returned with the offset that keeps the *top* of the art on the cell, so
    /// the overhang grows downward rather than around the middle.
    #[must_use]
    pub fn tile_draw(&self) -> TileDraw {
        TileDraw {
            size: [self.tile_size[0], self.tile_size[1] + self.tile_overhang],
            offset_y: -self.tile_overhang / 2.0,
        }
    }

    pub fn validate(&self) -> Result<(), TilemapError> {
        self.grid_bounds()?;
        self.grid_space()?;
        if !self.tile_overhang.is_finite() || self.tile_overhang < 0.0 {
            return Err(TilemapError::NegativeOverhang(self.tile_overhang));
        }
        if self.tiles.len() != self.expected_cells() {
            return Err(TilemapError::WrongCellCount {
                columns: self.columns,
                rows: self.rows,
                expected: self.expected_cells(),
                actual: self.tiles.len(),
            });
        }
        for (column, row, index) in self.filled() {
            if self.palette.is_empty() {
                return Err(TilemapError::EmptyPalette);
            }
            if index as usize >= self.palette.len() {
                return Err(TilemapError::TileOutsidePalette {
                    column,
                    row,
                    index,
                    palette: self.palette.len(),
                });
            }
        }
        Ok(())
    }

    /// Every cell that holds a tile, as `(column, row, index)`, in the order the
    /// array stores them.
    ///
    /// Reading order is the map's order, so a frame extracted from a tilemap is
    /// the same frame every time without anything having to sort it.
    pub fn filled(&self) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        // Bounded by the size the map claims, so a `tiles` array longer than
        // the map draws what the map says rather than trailing off its edge.
        self.tiles
            .iter()
            .take(self.expected_cells())
            .enumerate()
            .filter_map(move |(cell, tile)| {
                let index = (*tile)?;
                let columns = self.columns.max(1);
                let cell = u32::try_from(cell).ok()?;
                Some((cell % columns, cell / columns, index))
            })
    }

    /// The tile at `column`, `row`, or `None` where the map is empty or the
    /// coordinates are off it.
    #[must_use]
    pub fn tile(&self, column: u32, row: u32) -> Option<u32> {
        if column >= self.columns || row >= self.rows {
            return None;
        }
        let cell = (row as usize) * (self.columns as usize) + column as usize;
        self.tiles.get(cell).copied().flatten()
    }

    /// Where the centre of `column`, `row` sits, relative to the map's own
    /// origin — the entity's transform puts that origin in the world.
    // The grid arithmetic happens in f64 and is narrowed exactly once, here,
    // so a map wider than an f32 mantissa still places its last column where
    // the integers say rather than a hair off it. Same reasoning as
    // `UvRect::cell`.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn tile_to_local(&self, column: u32, row: u32) -> [f32; 2] {
        let Ok(column) = i32::try_from(column) else {
            return [0.0, 0.0];
        };
        let Ok(row) = i32::try_from(row) else {
            return [0.0, 0.0];
        };
        let Ok(point) = self
            .grid_space()
            .and_then(|grid| grid.grid_to_plane(GridCoord::new(column, row)))
        else {
            return [0.0, 0.0];
        };
        [point.x as f32, point.y as f32]
    }

    /// Which tile covers a point in the map's own space, or `None` when the
    /// point is off the map.
    ///
    /// The inverse of [`Self::tile_to_local`], and a test holds it to that on
    /// every cell of both projections. It is what turns a click into a tile,
    /// which is the whole of what painting a map needs from the maths.
    #[must_use]
    pub fn local_to_tile(&self, x: f32, y: f32) -> Option<(u32, u32)> {
        let coord = self
            .grid_space()
            .ok()?
            .plane_to_grid(PlanePoint::new(f64::from(x), f64::from(y)))
            .ok()?;
        self.grid_bounds().ok()?.contains(coord).then(|| {
            (
                u32::try_from(coord.x).expect("bounds rejected negative columns"),
                u32::try_from(coord.y).expect("bounds rejected negative rows"),
            )
        })
    }

    /// The logical bounds shared by rendering, picking, and future gameplay.
    pub fn grid_bounds(&self) -> Result<GridBounds, GridError> {
        GridBounds::new(self.columns, self.rows)
    }

    /// The exact mapping the tilemap uses from cells into entity-local XY.
    pub fn grid_space(&self) -> Result<GridSpace, GridError> {
        let width = f64::from(self.tile_size[0]);
        let height = f64::from(self.tile_size[1]);
        let (projection, origin) = match self.projection {
            TileProjection::Orthogonal => (
                Projection::Orthogonal,
                PlanePoint::new(width * 0.5, -height * 0.5),
            ),
            TileProjection::Isometric => (Projection::Isometric, PlanePoint::default()),
        };
        GridSpace::with_origin_and_y_axis(projection, width, height, origin, PlaneYAxis::Up)
    }

    /// What tile `index` is called in the sheet.
    ///
    /// A name and not a rect: where the sprite *is* belongs to the sheet, and
    /// this component's business ends at saying which one it wants.
    #[must_use]
    pub fn sprite_of(&self, index: u32) -> Option<&str> {
        self.palette.get(index as usize).map(String::as_str)
    }
}

impl SceneComponent for TilemapComponent {
    const TYPE_NAME: &'static str = "sindri.tilemap";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compared with a tolerance because these are floats; the values are exact
    /// sums of authored numbers, so the tolerance is only there to say so.
    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1.0e-6
    }

    /// A map that says nothing about overhang draws exactly its cells, which is
    /// what every tilemap authored before the field did.
    #[test]
    fn a_map_without_an_overhang_draws_its_cell() {
        let map = map(TileProjection::Isometric, 2, 2);
        let draw = map.tile_draw();
        assert!(close(draw.size[0], map.tile_size[0]));
        assert!(close(draw.size[1], map.tile_size[1]));
        assert!(close(draw.offset_y, 0.0));
    }

    /// An overhang grows the quad downward and leaves the top of the art where
    /// the cell is. Both halves matter: a quad that grew around its middle
    /// would lift the tile's face off the grid everything else measures in.
    #[test]
    fn an_overhang_hangs_below_the_cell_it_is_drawn_for() {
        let mut map = map(TileProjection::Isometric, 2, 2);
        map.tile_size = [1.1, 0.55];
        map.tile_overhang = 0.125;

        let draw = map.tile_draw();
        assert!(close(draw.size[0], 1.1), "{:?}", draw.size);
        assert!(close(draw.size[1], 0.675), "{:?}", draw.size);
        assert!(close(draw.offset_y, -0.0625), "{}", draw.offset_y);

        // The top edge is the thing that must not move: half the quad above its
        // own centre, which the offset puts back on the cell's top edge.
        let quad_top = draw.offset_y + draw.size[1] / 2.0;
        let cell_top = map.tile_size[1] / 2.0;
        assert!(
            (quad_top - cell_top).abs() < 1.0e-6,
            "{quad_top} != {cell_top}"
        );
    }

    /// An overhang reaching *up* is refused rather than drawn, because a tile
    /// hanging above its cell would occlude the row behind it.
    #[test]
    fn an_overhang_cannot_reach_above_the_cell() {
        let mut map = map(TileProjection::Isometric, 2, 2);
        map.tile_overhang = -0.5;
        assert!(matches!(
            map.validate(),
            Err(TilemapError::NegativeOverhang(_))
        ));
    }

    fn map(projection: TileProjection, columns: u32, rows: u32) -> TilemapComponent {
        TilemapComponent {
            texture: "tiles".to_owned(),
            tile_overhang: 0.0,
            palette: vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned(),
            ],
            columns,
            rows,
            tile_size: [1.1, 0.55],
            projection,
            tiles: vec![Some(0); (columns * rows) as usize],
            tint: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
        }
    }

    /// Turning a click into a tile is the whole of what painting a map needs
    /// from the maths, and it is only ever right if it undoes placement
    /// exactly. Both projections, every cell.
    #[test]
    fn a_tile_centre_maps_back_to_its_own_tile() {
        for projection in [TileProjection::Orthogonal, TileProjection::Isometric] {
            let map = map(projection, 7, 5);
            for row in 0..map.rows {
                for column in 0..map.columns {
                    let [x, y] = map.tile_to_local(column, row);
                    assert_eq!(
                        map.local_to_tile(x, y),
                        Some((column, row)),
                        "{projection:?} lost tile {column},{row} at {x},{y}"
                    );
                }
            }
        }
    }

    /// A point off the map is off the map, rather than the nearest edge tile.
    #[test]
    fn a_point_outside_the_map_belongs_to_no_tile() {
        let map = map(TileProjection::Orthogonal, 3, 3);
        assert_eq!(map.local_to_tile(-1.0, -0.5), None, "left of the origin");
        assert_eq!(map.local_to_tile(0.5, 1.0), None, "above the origin");
        assert_eq!(map.local_to_tile(99.0, -0.5), None, "past the last column");
        assert_eq!(
            map.local_to_tile(0.0, -0.5),
            Some((0, 0)),
            "the map includes its left edge"
        );
        assert_eq!(
            map.local_to_tile(1.1, -0.5),
            Some((1, 0)),
            "a shared edge belongs to the cell on its right"
        );
    }

    /// Empty cells are `null` and every index is a real tile, so a map of
    /// nothing is not a map of tile zero.
    #[test]
    fn an_empty_cell_is_not_tile_zero() {
        let mut map = map(TileProjection::Orthogonal, 2, 1);
        map.tiles = vec![None, Some(0)];
        assert_eq!(map.tile(0, 0), None);
        assert_eq!(map.tile(1, 0), Some(0));
        assert_eq!(
            map.filled().collect::<Vec<_>>(),
            vec![(1, 0, 0)],
            "only the cell holding a tile is drawn"
        );
    }

    #[test]
    fn a_map_of_the_wrong_length_is_rejected() {
        let mut map = map(TileProjection::Orthogonal, 4, 4);
        map.tiles = vec![Some(0), Some(0)];
        assert!(matches!(
            map.validate(),
            Err(TilemapError::WrongCellCount {
                expected: 16,
                actual: 2,
                ..
            })
        ));
    }

    #[test]
    fn a_map_with_no_real_cell_size_is_rejected() {
        let mut map = map(TileProjection::Isometric, 1, 1);
        map.tile_size = [0.0, 1.0];
        assert!(matches!(map.validate(), Err(TilemapError::Grid(_))));
    }

    #[test]
    fn a_tile_outside_the_palette_is_rejected() {
        let mut map = map(TileProjection::Orthogonal, 1, 1);
        map.tiles = vec![Some(9)];
        assert!(matches!(
            map.validate(),
            Err(TilemapError::TileOutsidePalette { index: 9, .. })
        ));
    }

    /// A tile names a sprite; where that sprite is belongs to the sheet.
    #[test]
    fn a_tile_names_a_sprite_from_the_palette() {
        let map = map(TileProjection::Orthogonal, 1, 1);
        assert_eq!(map.sprite_of(0), Some("a"));
        assert_eq!(map.sprite_of(3), Some("d"));
        assert_eq!(
            map.sprite_of(9),
            None,
            "a tile past the palette names nothing"
        );
    }
}
