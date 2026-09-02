//! The part of a texture a sprite draws.
//!
//! Until this existed, one texture was one sprite: every sprite sampled the
//! whole image, so a sprite sheet was not expressible and neither was a tilemap.
//! `docs/2d-inventory.md` found both of Milestone 6's first two ports blocked on
//! exactly this.
//!
//! The rect is in normalized texture coordinates rather than pixels, and that is
//! the load-bearing choice. A grid-sliced sheet's frames are `column / columns`
//! wide wherever the sheet's resolution lands, so normalized rects survive an
//! artist doubling the sheet's size and pixel rects do not. It also means
//! nothing has to know a texture's dimensions to place a frame — not the scene,
//! not extraction, not the shader.

use thiserror::Error;

/// A rectangle in normalized texture coordinates, with the origin at the top
/// left corner of the image.
///
/// Constructed checked, because the ways to get this wrong are quiet: a rect of
/// zero width samples one column of texels down the whole sprite, and one that
/// runs past the edge samples whatever the sampler's addressing mode decides,
/// which is a different picture on a different clamp mode rather than an error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl UvRect {
    /// The whole texture, which is what every sprite drew before rects existed.
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    /// The part of this rect a quad's sub-area covers.
    ///
    /// `offset` and `scale` describe an area inside a unit quad centred on its
    /// own origin, with `+Y` up, which is how a quad is built. Texture
    /// coordinates run the other way vertically, and this is the one place that
    /// flip is written down — a caller that did the arithmetic itself would be
    /// a second place for it to be wrong.
    ///
    /// `None` for an empty area, because nothing is not a rect: a bar filled to
    /// zero draws no quad at all rather than a degenerate one.
    #[must_use]
    pub fn part(self, offset: [f32; 2], scale: [f32; 2]) -> Option<Self> {
        if scale[0] <= 0.0 || scale[1] <= 0.0 {
            return None;
        }
        Some(Self {
            x: self.width.mul_add(0.5 + offset[0] - scale[0] / 2.0, self.x),
            y: self
                .height
                .mul_add(0.5 - offset[1] - scale[1] / 2.0, self.y),
            width: self.width * scale[0],
            height: self.height * scale[1],
        })
    }

    /// A rect covering part of a texture.
    ///
    /// Refuses anything that is not a positive area inside the image. Clamping
    /// instead would turn a mistyped frame into a slightly wrong picture, which
    /// is the failure that survives review.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Result<Self, UvRectError> {
        let rect = Self {
            x,
            y,
            width,
            height,
        };
        if !rect.to_array().iter().all(|value| value.is_finite()) {
            return Err(UvRectError::NotFinite(rect.to_array()));
        }
        if width <= 0.0 || height <= 0.0 {
            return Err(UvRectError::Empty { width, height });
        }
        if x < 0.0 || y < 0.0 || x + width > 1.0 || y + height > 1.0 {
            return Err(UvRectError::OutsideTexture(rect.to_array()));
        }
        Ok(rect)
    }

    /// One cell of a sheet cut into an even grid.
    ///
    /// The way a sheet is almost always sliced, and the reason the rect is
    /// normalized: this answer does not depend on how large the sheet is.
    // Narrowing to f32 is the point rather than a hazard: the division happens
    // in f64 so the last cell of a sheet that does not divide evenly in binary
    // still lands on the edge, and the result is narrowed exactly once, here.
    #[allow(clippy::cast_possible_truncation)]
    pub fn cell(column: u32, row: u32, columns: u32, rows: u32) -> Result<Self, UvRectError> {
        if columns == 0 || rows == 0 {
            return Err(UvRectError::EmptyGrid { columns, rows });
        }
        if column >= columns || row >= rows {
            return Err(UvRectError::OutsideGrid {
                column,
                row,
                columns,
                rows,
            });
        }
        let width = 1.0 / f64::from(columns);
        let height = 1.0 / f64::from(rows);
        // Multiplied in f64 and narrowed once, so the last column of a sheet
        // that does not divide evenly into binary lands on the edge rather than
        // a hair past it.
        Self::new(
            (f64::from(column) * width) as f32,
            (f64::from(row) * height) as f32,
            width as f32,
            height as f32,
        )
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn width(self) -> f32 {
        self.width
    }

    pub const fn height(self) -> f32 {
        self.height
    }

    /// Whether this is the whole texture, which is what a scene omits rather
    /// than writes.
    pub fn is_full(self) -> bool {
        self == Self::FULL
    }

    /// `[x, y, width, height]`, which is how the instance buffer and a scene
    /// both carry it.
    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    /// Reads a rect back without checking it.
    ///
    /// Only for values that came from [`Self::to_array`]. Everything entering
    /// from outside goes through [`Self::new`].
    pub(crate) const fn from_array(values: [f32; 4]) -> Self {
        Self {
            x: values[0],
            y: values[1],
            width: values[2],
            height: values[3],
        }
    }
}

impl Default for UvRect {
    fn default() -> Self {
        Self::FULL
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum UvRectError {
    #[error("texture rect {0:?} is not finite")]
    NotFinite([f32; 4]),
    #[error("a texture rect must have area, and this one is {width} by {height}")]
    Empty { width: f32, height: f32 },
    #[error("texture rect {0:?} reaches outside the texture")]
    OutsideTexture([f32; 4]),
    #[error("a sheet cut into {columns} by {rows} has no cells")]
    EmptyGrid { columns: u32, rows: u32 },
    #[error("cell ({column}, {row}) is outside a sheet of {columns} by {rows}")]
    OutsideGrid {
        column: u32,
        row: u32,
        columns: u32,
        rows: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact comparison, because these numbers are exact: every rect asserted
    /// below is built from halves and quarters, and a slice that came out a
    /// bit-width off is a slice that samples a neighbouring cell's edge texel.
    fn bits(rect: UvRect) -> [u32; 4] {
        rect.to_array().map(f32::to_bits)
    }

    fn exact(values: [f32; 4]) -> [u32; 4] {
        values.map(f32::to_bits)
    }

    #[test]
    fn the_whole_texture_is_the_default() {
        assert_eq!(UvRect::default(), UvRect::FULL);
        assert!(UvRect::FULL.is_full());
        assert_eq!(bits(UvRect::FULL), exact([0.0, 0.0, 1.0, 1.0]));
        assert!(!UvRect::new(0.0, 0.0, 0.5, 1.0).unwrap().is_full());
    }

    /// Every way of asking for a rect that is not a piece of the image.
    #[test]
    fn a_rect_that_is_not_part_of_the_texture_is_refused() {
        assert_eq!(
            UvRect::new(0.0, 0.0, 0.0, 0.5),
            Err(UvRectError::Empty {
                width: 0.0,
                height: 0.5
            })
        );
        assert!(matches!(
            UvRect::new(0.0, 0.0, -0.5, 0.5),
            Err(UvRectError::Empty { .. })
        ));
        assert!(matches!(
            UvRect::new(0.75, 0.0, 0.5, 0.5),
            Err(UvRectError::OutsideTexture(_))
        ));
        assert!(matches!(
            UvRect::new(-0.1, 0.0, 0.5, 0.5),
            Err(UvRectError::OutsideTexture(_))
        ));
        assert!(matches!(
            UvRect::new(f32::NAN, 0.0, 0.5, 0.5),
            Err(UvRectError::NotFinite(_))
        ));
    }

    /// A rect that exactly reaches the far edge is inside the texture, and is
    /// the one every sheet's last column and bottom row produce.
    #[test]
    fn a_rect_touching_the_far_edge_is_inside() {
        assert!(UvRect::new(0.5, 0.5, 0.5, 0.5).is_ok());
    }

    /// The property that makes normalized rects the right choice: a grid slice
    /// is the same answer whatever the sheet's resolution is.
    #[test]
    fn a_grid_cell_covers_its_share_of_the_sheet() {
        let cell = UvRect::cell(1, 2, 4, 4).expect("a cell of a four by four sheet");
        assert_eq!(bits(cell), exact([0.25, 0.5, 0.25, 0.25]));

        // The four corners of a two by two, which between them tile the whole
        // texture with no gap and no overlap.
        let corners: Vec<[u32; 4]> = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .into_iter()
            .map(|(column, row)| bits(UvRect::cell(column, row, 2, 2).unwrap()))
            .collect();
        assert_eq!(
            corners,
            [
                exact([0.0, 0.0, 0.5, 0.5]),
                exact([0.5, 0.0, 0.5, 0.5]),
                exact([0.0, 0.5, 0.5, 0.5]),
                exact([0.5, 0.5, 0.5, 0.5]),
            ]
        );
    }

    /// A sheet whose cell size does not divide evenly in binary still has a last
    /// cell that ends on the edge rather than a hair past it, which is what the
    /// checked constructor would otherwise refuse.
    #[test]
    fn the_last_cell_of_an_awkward_sheet_still_fits() {
        for columns in [3_u32, 5, 6, 7, 9, 10, 11, 12, 24, 100] {
            let last = UvRect::cell(columns - 1, 0, columns, 1);
            assert!(last.is_ok(), "the last cell of {columns} was {last:?}");
        }
    }

    #[test]
    fn a_cell_outside_the_sheet_is_refused() {
        assert!(matches!(
            UvRect::cell(4, 0, 4, 4),
            Err(UvRectError::OutsideGrid { .. })
        ));
        assert!(matches!(
            UvRect::cell(0, 0, 0, 4),
            Err(UvRectError::EmptyGrid { .. })
        ));
    }
}

#[cfg(test)]
mod part_tests {
    use super::UvRect;

    #[test]
    fn the_whole_quad_is_the_whole_rect() {
        let part = UvRect::FULL.part([0.0, 0.0], [1.0, 1.0]).expect("an area");
        assert_eq!(part, UvRect::FULL);
    }

    /// A bar filled to a third keeps its left third, and so does its texture.
    #[test]
    fn keeping_the_left_of_a_quad_keeps_the_left_of_the_texture() {
        let part = UvRect::FULL
            .part([-1.0 / 3.0, 0.0], [1.0 / 3.0, 1.0])
            .expect("an area");
        assert!((part.x()).abs() < 1.0e-5, "{part:?}");
        assert!((part.width() - 1.0 / 3.0).abs() < 1.0e-5, "{part:?}");
    }

    /// `+Y` is up on a quad and down in a texture, which is the flip this
    /// method exists to own.
    #[test]
    fn the_top_of_a_quad_is_the_start_of_the_texture() {
        let part = UvRect::FULL.part([0.0, 0.25], [1.0, 0.5]).expect("an area");
        assert!(part.y().abs() < 1.0e-5, "{part:?}");
        assert!((part.height() - 0.5).abs() < 1.0e-5, "{part:?}");
    }

    #[test]
    fn a_part_of_a_sheet_frame_stays_inside_that_frame() {
        let frame = UvRect::new(0.5, 0.0, 0.5, 1.0).expect("a frame");
        let part = frame.part([-0.25, 0.0], [0.5, 1.0]).expect("an area");
        assert!((part.x() - 0.5).abs() < 1.0e-5, "{part:?}");
        assert!((part.width() - 0.25).abs() < 1.0e-5, "{part:?}");
    }

    #[test]
    fn nothing_is_not_a_rect() {
        assert!(UvRect::FULL.part([0.0, 0.0], [0.0, 1.0]).is_none());
    }
}
