use serde::Deserialize;
use sindri_core::{SceneComponent, SpriteRef, SpriteRefError};
use sindri_grid::{
    GridBounds, GridCoord, GridError, GridSpace, PlanePoint, PlaneYAxis, Projection,
};
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
    /// The explicit override on draw order. Within a layer sprites sort by how
    /// far from the camera they are; a layer beats that, so a sprite in a
    /// higher one draws in front of something nearer the camera.
    #[serde(default)]
    pub layer: i32,
}

const fn opaque_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

impl SpriteComponent {
    /// The texture this sprite draws, and which named part of it.
    ///
    /// `textures/tiles.png#floor` draws one sprite of a sliced sheet;
    /// `textures/badge.png` draws the whole image. Which part is no longer the
    /// sprite's business to describe — the sheet beside the image says how it
    /// is cut, and this only picks one of the names it gives.
    ///
    /// Checked here rather than at deserialization because a scene carrying a
    /// bad reference should still open — the editor exists to fix it, and
    /// refusing the file would be refusing to let anyone.
    pub fn reference(&self) -> Result<SpriteRef, SpriteRefError> {
        SpriteRef::parse(&self.texture)
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

/// Screen-space text drawn through the overlay camera.
///
/// The font is a project asset reference rather than a family installed on the
/// machine. That keeps a scene reproducible across the editor, captures, and
/// the browser: a host binds the bytes at that reference before drawing.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TextComponent {
    pub text: String,
    pub font: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "opaque_white")]
    pub color: [f32; 4],
    #[serde(default)]
    pub anchor: SpriteAnchor,
    #[serde(default)]
    pub layer: i32,
}

const fn default_font_size() -> f32 {
    24.0
}

const fn default_line_height() -> f32 {
    30.0
}

impl SceneComponent for TextComponent {
    const TYPE_NAME: &'static str = "sindri.text";
}

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
    #[serde(default)]
    pub space: SpriteSpace,
}

const fn unit_tile() -> [f32; 2] {
    [1.0, 1.0]
}

/// What is wrong with a tilemap, named specifically enough to fix.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TilemapError {
    #[error(transparent)]
    Grid(#[from] GridError),
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
    pub fn validate(&self) -> Result<(), TilemapError> {
        self.grid_bounds()?;
        self.grid_space()?;
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
        self.grid_bounds()
            .ok()?
            .contains(coord)
            .then(|| {
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
        GridSpace::with_origin_and_y_axis(
            projection,
            width,
            height,
            origin,
            PlaneYAxis::Up,
        )
    }

    /// What tile `index` is called in the sheet.
    ///
    /// A name and not a rect: where the sprite *is* belongs to the sheet, and
    /// this component's business ends at saying which one it wants.
    #[must_use]
    pub fn sprite_of(&self, index: u32) -> Option<&str> {
        self.palette.get(index as usize).map(String::as_str)
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
