use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_render::{UvRect, UvRectError};
use thiserror::Error;

/// A camera authored into a scene.
///
/// The projection tag chooses which fields apply, so a scene cannot describe a
/// perspective camera with an orthographic size.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "projection", rename_all = "snake_case")]
pub enum CameraComponent {
    /// Renders the 3D world. Its eye comes from the entity's `Transform3D`.
    Perspective {
        target: [f32; 3],
        up: [f32; 3],
        vertical_fov_degrees: f32,
        near: f32,
        far: f32,
    },
    /// Renders the 2D overlay, and defines the space sprite anchors resolve in.
    Orthographic {
        center: [f32; 2],
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

impl SceneComponent for CameraComponent {
    const TYPE_NAME: &'static str = "sindri.camera";
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MeshPrimitive {
    Cube,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MeshComponent {
    pub primitive: MeshPrimitive,
    pub texture: String,
    #[serde(default)]
    pub layer: i32,
}

impl SceneComponent for MeshComponent {
    const TYPE_NAME: &'static str = "sindri.mesh";
}

/// The space a sprite is placed and drawn in.
///
/// Screen is the default because it is what every sprite was before there was a
/// choice, so no existing scene changes meaning by gaining the field.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SpriteSpace {
    /// Drawn through the overlay camera, anchored to its extent. A HUD is not
    /// in the world, so no world camera moves it and nothing in the world can
    /// hide it. Its Z says how far back in the stack it sits and nothing else:
    /// it orders the sprite without moving it, so no HUD can be lost off the
    /// far plane by typing a big number.
    #[default]
    Screen,
    /// Placed in the world by its transform and drawn through the world camera,
    /// like any other thing in the scene: it moves when the camera moves, it
    /// has a Z, and opaque geometry in front of it hides it.
    World,
}

/// Where a screen-space sprite's origin sits inside the overlay camera's view.
///
/// Anchoring is resolved against the overlay camera's extent, so a sprite keeps
/// its relationship to an edge as the window changes shape. A world-space
/// sprite has no edge to hold on to, which is what [`SpriteComponent::screen_anchor`]
/// says in the type.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpriteAnchor {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl SpriteAnchor {
    /// The anchor as a fraction of the half-extent, in `[-1, 1]` per axis.
    pub const fn unit_offset(self) -> [f32; 2] {
        match self {
            Self::Center => [0.0, 0.0],
            Self::Top => [0.0, 1.0],
            Self::Bottom => [0.0, -1.0],
            Self::Left => [-1.0, 0.0],
            Self::Right => [1.0, 0.0],
            Self::TopLeft => [-1.0, 1.0],
            Self::TopRight => [1.0, 1.0],
            Self::BottomLeft => [-1.0, -1.0],
            Self::BottomRight => [1.0, -1.0],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpriteComponent {
    pub texture: String,
    #[serde(default)]
    pub space: SpriteSpace,
    /// Only a screen-space sprite anchors. Read it through
    /// [`SpriteComponent::screen_anchor`] rather than directly, so a
    /// world-space sprite cannot be quietly anchored to an edge it does not
    /// have.
    #[serde(default)]
    pub anchor: SpriteAnchor,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    /// Which part of the texture to draw, as `[x, y, width, height]` in
    /// normalized coordinates.
    ///
    /// The whole texture unless a scene says otherwise, so every scene written
    /// before sheets existed reads exactly as it did — and saves exactly as it
    /// did too, since a component is a typed view of a stored payload and the
    /// payload is what gets written back.
    ///
    /// Read through [`SpriteComponent::uv_rect`], which checks it: a rect of no
    /// area or one reaching past the edge is a picture that is quietly wrong
    /// rather than an error.
    #[serde(default = "full_uv_rect")]
    pub uv_rect: [f32; 4],
    /// The explicit override on draw order. Within a layer sprites sort by how
    /// far from the camera they are; a layer beats that, so a sprite in a
    /// higher one draws in front of something nearer the camera.
    #[serde(default)]
    pub layer: i32,
}

const fn opaque_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn full_uv_rect() -> [f32; 4] {
    UvRect::FULL.to_array()
}

impl SpriteComponent {
    /// The part of the texture this sprite draws, checked.
    ///
    /// Checked here rather than at deserialization because a scene carrying a
    /// bad rect should still open — the editor exists to fix it, and refusing
    /// the file would be refusing to let anyone.
    pub fn uv_rect(&self) -> Result<UvRect, UvRectError> {
        let [x, y, width, height] = self.uv_rect;
        UvRect::new(x, y, width, height)
    }

    /// The anchor this sprite resolves against, or `None` when it is in the
    /// world, where there is no screen edge to anchor to.
    pub const fn screen_anchor(&self) -> Option<SpriteAnchor> {
        match self.space {
            SpriteSpace::Screen => Some(self.anchor),
            SpriteSpace::World => None,
        }
    }
}

impl SceneComponent for SpriteComponent {
    const TYPE_NAME: &'static str = "sindri.sprite";
}

/// How a tilemap's grid coordinates become world positions.
///
/// This is the tilemap's own layout rule and not the grid module Milestone 9
/// schedules. That module owns coordinates, neighbours, and pathfinding for
/// whatever wants them; this owns only where a tile is drawn, which a renderer
/// needs before any of that exists.
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
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TilemapComponent {
    /// The sheet every tile is cut from.
    pub texture: String,
    /// The sheet's grid, which `tiles` indexes into row-major from its
    /// top-left — the same order and the same origin as the map's own.
    pub sheet_columns: u32,
    pub sheet_rows: u32,
    /// The map's size in tiles.
    pub columns: u32,
    pub rows: u32,
    /// One tile's size in world units.
    #[serde(default = "unit_tile")]
    pub tile_size: [f32; 2],
    #[serde(default)]
    pub projection: TileProjection,
    /// `columns * rows` cells, row-major from the top-left, `null` where the
    /// map has no tile.
    ///
    /// Null rather than a sentinel index, because every index is a real tile:
    /// reserving 0 or -1 to mean "empty" is how a map ends up with an
    /// accidental floor in the corner nobody authored.
    pub tiles: Vec<Option<u32>>,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    #[serde(default)]
    pub layer: i32,
    #[serde(default)]
    pub space: SpriteSpace,
}

const fn unit_tile() -> [f32; 2] {
    [1.0, 1.0]
}

/// What is wrong with a tilemap, named specifically enough to fix.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TilemapError {
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
        "tile {index} at column {column}, row {row} is not in a {sheet_columns}x{sheet_rows} sheet"
    )]
    TileOutsideSheet {
        column: u32,
        row: u32,
        index: u32,
        sheet_columns: u32,
        sheet_rows: u32,
    },
    #[error("tilemap cell: {0}")]
    Cell(#[from] UvRectError),
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
    pub fn validate(&self) -> Result<(), TilemapError> {
        if self.tiles.len() != self.expected_cells() {
            return Err(TilemapError::WrongCellCount {
                columns: self.columns,
                rows: self.rows,
                expected: self.expected_cells(),
                actual: self.tiles.len(),
            });
        }
        for (column, row, index) in self.filled() {
            let cells = self.sheet_columns.saturating_mul(self.sheet_rows);
            if index >= cells || self.sheet_columns == 0 || self.sheet_rows == 0 {
                return Err(TilemapError::TileOutsideSheet {
                    column,
                    row,
                    index,
                    sheet_columns: self.sheet_columns,
                    sheet_rows: self.sheet_rows,
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
        let width = f64::from(self.tile_size[0]);
        let height = f64::from(self.tile_size[1]);
        let column = f64::from(column);
        let row = f64::from(row);
        let [x, y] = match self.projection {
            TileProjection::Orthogonal => [(column + 0.5) * width, -(row + 0.5) * height],
            // Half steps, so the diamonds tile edge to edge rather than leaving
            // a gap of their own size between them.
            TileProjection::Isometric => {
                [(column - row) * width * 0.5, -(column + row) * height * 0.5]
            }
        };
        [x as f32, y as f32]
    }

    /// Which tile covers a point in the map's own space, or `None` when the
    /// point is off the map.
    ///
    /// The inverse of [`Self::tile_to_local`], and a test holds it to that on
    /// every cell of both projections. It is what turns a click into a tile,
    /// which is the whole of what painting a map needs from the maths.
    // Both coordinates are bounds-checked against the map's own size as f64
    // just below, so by the time either is narrowed it is known to sit in
    // `0..columns` or `0..rows` and cannot truncate to something else.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[must_use]
    pub fn local_to_tile(&self, x: f32, y: f32) -> Option<(u32, u32)> {
        let width = f64::from(self.tile_size[0]);
        let height = f64::from(self.tile_size[1]);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let x = f64::from(x);
        let y = f64::from(y);
        let (column, row) = match self.projection {
            TileProjection::Orthogonal => ((x / width).floor(), (-y / height).floor()),
            TileProjection::Isometric => {
                let across = x / width;
                let down = -y / height;
                ((down + across).round(), (down - across).round())
            }
        };
        let holds = |value: f64, limit: u32| value >= 0.0 && value < f64::from(limit);
        (holds(column, self.columns) && holds(row, self.rows))
            .then_some((column as u32, row as u32))
    }

    /// The part of the sheet tile `index` draws.
    pub fn cell_rect(&self, index: u32) -> Result<UvRect, TilemapError> {
        let columns = self.sheet_columns;
        let rows = self.sheet_rows;
        if columns == 0 || rows == 0 {
            return Err(TilemapError::TileOutsideSheet {
                column: 0,
                row: 0,
                index,
                sheet_columns: columns,
                sheet_rows: rows,
            });
        }
        Ok(UvRect::cell(
            index % columns,
            index / columns,
            columns,
            rows,
        )?)
    }

    /// The anchor this map resolves against, matching a sprite's rule so the
    /// two spaces cannot mean different things.
    #[must_use]
    pub const fn is_screen_space(&self) -> bool {
        matches!(self.space, SpriteSpace::Screen)
    }
}

impl SceneComponent for TilemapComponent {
    const TYPE_NAME: &'static str = "sindri.tilemap";
}

#[cfg(test)]
mod tilemap_tests {
    use super::{SpriteSpace, TileProjection, TilemapComponent, TilemapError};

    fn map(projection: TileProjection, columns: u32, rows: u32) -> TilemapComponent {
        TilemapComponent {
            texture: "tiles".to_owned(),
            sheet_columns: 2,
            sheet_rows: 2,
            columns,
            rows,
            tile_size: [1.1, 0.55],
            projection,
            tiles: vec![Some(0); (columns * rows) as usize],
            tint: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
            space: SpriteSpace::World,
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
    fn a_tile_outside_the_sheet_is_rejected() {
        let mut map = map(TileProjection::Orthogonal, 1, 1);
        map.tiles = vec![Some(9)];
        assert!(matches!(
            map.validate(),
            Err(TilemapError::TileOutsideSheet { index: 9, .. })
        ));
    }

    /// Cell indices run row-major across the sheet, the same way the map's own
    /// cells do, so there is one reading order to remember rather than two.
    #[test]
    fn cell_rects_run_row_major_across_the_sheet() {
        let map = map(TileProjection::Orthogonal, 1, 1);
        let first = map.cell_rect(0).expect("tile 0 is in a 2x2 sheet");
        let second = map.cell_rect(1).expect("tile 1 is in a 2x2 sheet");
        let third = map.cell_rect(2).expect("tile 2 is in a 2x2 sheet");
        assert!(first.x() < second.x(), "tile 1 is to the right of tile 0");
        assert!(
            (first.y() - second.y()).abs() < f32::EPSILON,
            "tiles 0 and 1 share a row"
        );
        assert!(third.y() > first.y(), "tile 2 is on the next row down");
    }
}
