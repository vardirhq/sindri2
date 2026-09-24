//! `sindri.physics2d.tilemap_collider`: a tilemap's tiles, solid.
//!
//! A platformer's level is painted, and its floor and walls are the tiles it
//! was painted with. Without this every wall needed a collider entity of its
//! own, placed by hand over tiles that already said where the wall was, and
//! moved again whenever the wall was repainted. With it, the tiles are the
//! collision: paint a ledge and it can be stood on.
//!
//! Painted tiles are solid unless their sprite is listed as passable, which is
//! what lets grass tufts and background vines share a map with the ground.
//! The solid cells are merged into as few rectangles as cover them, rather
//! than one box a tile: a character running along a floor made of separate
//! boxes catches on the seams between them, and a floor of one box has none.

use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_physics::{Collider2d, ColliderShape2d, CollisionLayers};
use thiserror::Error;

use crate::components::{TileProjection, TilemapComponent};

/// Makes the tilemap on the same entity solid.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TilemapCollider2dComponent {
    /// Palette sprites that are not solid: decoration a character walks in
    /// front of rather than into.
    #[serde(default)]
    pub passable: Vec<String>,
    #[serde(default)]
    pub layers: CollisionLayers,
    #[serde(default = "default_friction")]
    pub friction: f32,
    #[serde(default)]
    pub restitution: f32,
}

const fn default_friction() -> f32 {
    0.5
}

impl SceneComponent for TilemapCollider2dComponent {
    const TYPE_NAME: &'static str = "sindri.physics2d.tilemap_collider";
}

/// Why a tilemap cannot be made solid.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum TilemapCollisionError {
    #[error(
        "a tilemap collider needs a tilemap on the same entity; add a Tilemap or remove the Tilemap Collider 2D"
    )]
    NoTilemap,
    #[error(
        "a tilemap collider needs an orthogonal map; an isometric map is a floor seen at an angle, and its tiles have no side to collide with"
    )]
    Isometric,
}

impl TilemapCollider2dComponent {
    /// The collider pieces that make `tilemap`'s solid tiles solid, in the
    /// entity's own space, scaled as the entity is so the collision covers
    /// the tiles where they are drawn.
    pub fn pieces(
        &self,
        tilemap: &TilemapComponent,
        scale: [f32; 2],
    ) -> Result<Vec<Collider2d>, TilemapCollisionError> {
        if tilemap.projection != TileProjection::Orthogonal {
            return Err(TilemapCollisionError::Isometric);
        }
        let solid_sprite: Vec<bool> = tilemap
            .palette
            .iter()
            .map(|sprite| !self.passable.contains(sprite))
            .collect();
        let solid = |column: u32, row: u32| {
            tilemap
                .tile(column, row)
                .is_some_and(|index| solid_sprite.get(index as usize).copied().unwrap_or(true))
        };
        let [width, height] = [
            tilemap.tile_size[0] * scale[0].abs(),
            tilemap.tile_size[1] * scale[1].abs(),
        ];
        let [flip_x, flip_y] = scale.map(f32::signum);
        Ok(merged_rectangles(tilemap.columns, tilemap.rows, solid)
            .into_iter()
            .map(|cells| {
                // A map's origin is its top-left corner, and rows run down.
                let centre = [
                    cells.centre_column() * width * flip_x,
                    -cells.centre_row() * height * flip_y,
                ];
                Collider2d {
                    shape: ColliderShape2d::Box {
                        half_extents: [cells.columns() * width / 2.0, cells.rows() * height / 2.0],
                    },
                    offset: centre,
                    rotation: 0.0,
                    sensor: false,
                    layers: self.layers,
                    friction: self.friction,
                    restitution: self.restitution,
                }
            })
            .collect())
    }
}

/// A block of cells: columns `left..right`, rows `top..bottom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cells {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

// Cell counts are small integers; f32 holds them exactly.
#[allow(clippy::cast_precision_loss)]
impl Cells {
    fn columns(self) -> f32 {
        (self.right - self.left) as f32
    }

    fn rows(self) -> f32 {
        (self.bottom - self.top) as f32
    }

    fn centre_column(self) -> f32 {
        (self.left + self.right) as f32 / 2.0
    }

    fn centre_row(self) -> f32 {
        (self.top + self.bottom) as f32 / 2.0
    }
}

/// Rectangles that together cover exactly the solid cells, each cell once.
///
/// Greedy, in reading order: a run along a row as wide as it goes, then as
/// many rows down as are solid the whole width of it. Not the fewest
/// rectangles possible, but a floor is one and a wall is one, which is what
/// keeps a character from catching on seams.
fn merged_rectangles(columns: u32, rows: u32, solid: impl Fn(u32, u32) -> bool) -> Vec<Cells> {
    let width = columns as usize;
    let mut taken = vec![false; width * rows as usize];
    let free = |taken: &[bool], column: u32, row: u32| {
        !taken[row as usize * width + column as usize] && solid(column, row)
    };
    let mut found = Vec::new();
    for top in 0..rows {
        for left in 0..columns {
            if !free(&taken, left, top) {
                continue;
            }
            let right = (left..columns)
                .take_while(|&column| free(&taken, column, top))
                .last()
                .map_or(left + 1, |last| last + 1);
            let bottom = (top + 1..rows)
                .take_while(|&row| (left..right).all(|column| free(&taken, column, row)))
                .last()
                .map_or(top + 1, |last| last + 1);
            for row in top..bottom {
                for column in left..right {
                    taken[row as usize * width + column as usize] = true;
                }
            }
            found.push(Cells {
                left,
                right,
                top,
                bottom,
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map drawn as text: `#` is ground, `~` is grass, `.` is empty.
    fn map(rows: &[&str]) -> TilemapComponent {
        let columns = rows[0].len();
        let tiles: Vec<Option<u32>> = rows
            .iter()
            .flat_map(|row| row.chars())
            .map(|cell| match cell {
                '#' => Some(0),
                '~' => Some(1),
                _ => None,
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "texture": "tiles.png",
            "palette": ["ground", "grass"],
            "columns": columns,
            "rows": rows.len(),
            "tile_size": [2.0, 1.0],
            "tiles": tiles
        }))
        .unwrap()
    }

    fn collider() -> TilemapCollider2dComponent {
        serde_json::from_value(serde_json::json!({ "passable": ["grass"] })).unwrap()
    }

    fn boxes(pieces: &[Collider2d]) -> Vec<([f32; 2], [f32; 2])> {
        pieces
            .iter()
            .map(|piece| {
                let ColliderShape2d::Box { half_extents } = piece.shape else {
                    panic!("tiles are boxes")
                };
                (piece.offset, half_extents)
            })
            .collect()
    }

    #[test]
    fn a_floor_is_one_box_however_many_tiles_it_is() {
        let pieces = collider()
            .pieces(&map(&["....", "####"]), [1.0, 1.0])
            .unwrap();
        // Four tiles 2 wide and 1 high, in the second row: centred at x 4,
        // half a tile below the first row.
        assert_eq!(boxes(&pieces), [([4.0, -1.5], [4.0, 0.5])]);
    }

    #[test]
    fn a_wall_and_a_floor_cover_every_solid_tile_once() {
        let level = map(&["#...", "#...", "####"]);
        let pieces = collider().pieces(&level, [1.0, 1.0]).unwrap();
        let area: f32 = boxes(&pieces)
            .iter()
            .map(|(_, half)| half[0] * 2.0 * half[1] * 2.0)
            .sum();
        assert!((area - 6.0 * 2.0).abs() < 1.0e-6, "six tiles of area 2");
        assert_eq!(pieces.len(), 2, "the wall and the floor");
    }

    #[test]
    fn passable_tiles_are_not_solid() {
        let pieces = collider()
            .pieces(&map(&["~~~~", "####"]), [1.0, 1.0])
            .unwrap();
        assert_eq!(boxes(&pieces), [([4.0, -1.5], [4.0, 0.5])]);
    }

    #[test]
    fn the_collision_is_scaled_with_the_map() {
        let pieces = collider().pieces(&map(&["##"]), [2.0, -1.0]).unwrap();
        assert_eq!(boxes(&pieces), [([4.0, 0.5], [4.0, 0.5])]);
    }

    #[test]
    fn an_empty_map_has_nothing_solid() {
        let pieces = collider().pieces(&map(&["....", "~~~~"]), [1.0, 1.0]);
        assert_eq!(pieces, Ok(Vec::new()));
    }

    #[test]
    fn an_isometric_map_is_refused_by_name() {
        let mut level = map(&["##"]);
        level.projection = TileProjection::Isometric;
        assert_eq!(
            collider().pieces(&level, [1.0, 1.0]),
            Err(TilemapCollisionError::Isometric)
        );
    }
}
