//! Reading and writing a tilemap's cells.
//!
//! The other `Grid.*` calls are about where an entity stands. These are about
//! what it is standing *on*, which in a game where the ground changes — tilled,
//! watered, grown, harvested — is gameplay state rather than scenery. Without
//! them a script can only put things on top of the floor and hope the picture
//! agrees.
//!
//! A cell is named by column and row rather than by a world position, because
//! the map is the authority on which cell a position falls in and a script that
//! did that arithmetic itself would be a second, disagreeing answer.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use sindri_core::{EntityId, SceneComponent};
use sindri_scene::TilemapComponent;

use crate::surface::{GridCall, TILE_GRID_COMPONENT};

use super::WorldHost;
use super::convert::number;
use super::geometry::{self, GridSource};

/// What a cell write is allowed to say, beyond an index into the palette.
///
/// Negative means the cell holds nothing. A sentinel rather than a second call,
/// because clearing a cell and setting one are the same authoring gesture and a
/// script chooses between them with the number it already has.
const EMPTY: f32 = -1.0;

/// One map's shape, read straight off the stored payload.
struct MapShape {
    columns: usize,
    rows: usize,
    /// `None` on a grid whose geometry came from `sindri.tile_grid`, which has
    /// no flat palette to index into.
    palette: Option<usize>,
}

impl WorldHost<'_> {
    /// `Grid.tile`, `Grid.set_tile`, `Grid.columns` and `Grid.rows`.
    ///
    /// Taken before the occupant-and-grid pair the other `Grid.*` calls share,
    /// because these name the map first and then a cell rather than an entity
    /// and the map it stands on.
    pub(super) fn tile_call(
        &mut self,
        call: GridCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        // A volume's cells are named and stacked rather than indexed and flat,
        // so they never reach the shape a flat map describes.
        if matches!(
            call,
            GridCall::Block | GridCall::SetBlock | GridCall::Tagged
        ) {
            return self.block_call(call, path, args);
        }
        // Answered from the volume's walkable surface, so it needs no flat
        // map's shape and a volume-only grid can be asked.
        if matches!(call, GridCall::Walkable) {
            return self.walkable_call(path, args);
        }
        let map = self.entity_argument(path, args, 0, "the tilemap")?;
        let shape = self.map_shape(path, map)?;

        match call {
            GridCall::Columns => Ok(Value::Number(as_number(shape.columns))),
            GridCall::Rows => Ok(Value::Number(as_number(shape.rows))),
            GridCall::Tile => {
                Self::flat_palette(path, &shape)?;
                let Some(index) = Self::cell_index(path, args, &shape)? else {
                    // Off the map reads as empty rather than as an error: a
                    // script asking what is under a moving thing will ask about
                    // the edge, and refusing that would make every caller wrap
                    // the call in a bounds check the map could do itself.
                    return Ok(Value::Number(f64::from(EMPTY)));
                };
                Ok(Value::Number(self.read_cell(path, args, index)?))
            }
            GridCall::SetTile => {
                let wanted = number(
                    path,
                    args.get(3).ok_or_else(|| {
                        RuntimeError::Host(format!("{} needs a palette index", path.dotted()))
                    })?,
                )?;
                let Some(index) = Self::cell_index(path, args, &shape)? else {
                    return Err(RuntimeError::Host(format!(
                        "{} was given a cell outside a {}x{} map",
                        path.dotted(),
                        shape.columns,
                        shape.rows
                    )));
                };
                self.write_cell(path, args, index, wanted, &shape)
            }
            GridCall::PositionX
            | GridCall::PositionY
            | GridCall::Place
            | GridCall::CanReach
            | GridCall::StepToward => {
                unreachable!("dispatched to the entity-and-grid calls instead")
            }
            GridCall::Block | GridCall::SetBlock | GridCall::Walkable | GridCall::Tagged => {
                unreachable!("answered above, before a flat map was looked for")
            }
        }
    }

    /// `Grid.walkable`: whether a walker can stand at a point on this grid.
    ///
    /// Which cell the point falls in is `sindri_scene::nearest_cell`, the same
    /// rule that decides which column a walker's own placement reads, because
    /// a script that rounded differently would be standing in one cell and
    /// asking about another.
    fn walkable_call(&mut self, path: &Path, args: &[Value]) -> Result<Value, RuntimeError> {
        let map = self.entity_argument(path, args, 0, "the grid")?;
        let mut point = [0.0_f64; 2];
        for (index, which) in [(1, "a column"), (2, "a row")] {
            point[index - 1] = number(
                path,
                args.get(index).ok_or_else(|| {
                    RuntimeError::Host(format!("{} needs {which}", path.dotted()))
                })?,
            )?;
        }
        // It has to be a grid. Everything below answers "nothing known about
        // the ground stops you", and a mistyped entity reading as open ground
        // is not a fallback, it is a bug dressed as scenery.
        {
            let data = self.world.get(map).ok_or_else(|| {
                RuntimeError::Host(format!("{}'s grid no longer exists", path.dotted()))
            })?;
            geometry::read(path, data)?;
        }
        let coord = sindri_scene::nearest_cell(point[0], point[1]);
        // Without a volume, or without the tile sets that say what its cells
        // mean, the shape of the ground is not knowable -- and the reasoning
        // `Grid.can_reach` uses applies: a host binding no tile sets is one
        // where nothing could draw the volume either. So nothing about the
        // ground blocks, which is how a flat map behaved before any of this.
        let Some(surfaces) = self.walkable_surface(path, map)? else {
            return Ok(Value::Bool(true));
        };
        // Nothing walkable in the column is the answer for water, for a hole,
        // for decoration, and for anywhere off the grid: none of them is ground
        // a walker stands on, and a caller asking about the edge should not
        // have to bound the question itself.
        Ok(Value::Bool(surfaces.walkable_height(coord).is_some()))
    }

    /// The walkable tops of this grid's volume, derived at most once an update.
    fn walkable_surface(
        &mut self,
        path: &Path,
        map: EntityId,
    ) -> Result<Option<&sindri_scene::TileSurfaces>, RuntimeError> {
        let revision = self.world.revision(map).unwrap_or_default();
        let fresh = self
            .walkable
            .as_ref()
            .is_some_and(|(grid, seen, _)| *grid == map && *seen == revision);
        if !fresh {
            let Some(surfaces) = self.derive_walkable(path, map)? else {
                self.walkable = None;
                return Ok(None);
            };
            self.walkable = Some((map, revision, surfaces));
        }
        Ok(self.walkable.as_ref().map(|(_, _, surfaces)| surfaces))
    }

    /// The walkable tops of this grid's volume, where both it and the tile set
    /// naming its cells can be read.
    fn derive_walkable(
        &self,
        path: &Path,
        map: EntityId,
    ) -> Result<Option<sindri_scene::TileSurfaces>, RuntimeError> {
        let Some(payload) = self.world.get(map).and_then(|data| {
            data.components
                .get(sindri_scene::TileVolumeComponent::TYPE_NAME)
        }) else {
            return Ok(None);
        };
        let Some(tile_sets) = self.tile_sets else {
            return Ok(None);
        };
        let volume: sindri_scene::TileVolumeComponent = serde_json::from_value(payload.clone())
            .map_err(|error| {
                RuntimeError::Host(format!(
                    "{} could not read the volume: {error}",
                    path.dotted()
                ))
            })?;
        // A bound tile set that does not name the volume's own set is a scene
        // mistake rather than a host without art, so it is not quietly treated
        // as open ground.
        let tile_set = tile_sets.get(&volume.tileset).ok_or_else(|| {
            RuntimeError::Host(format!(
                "{}: tile set `{}` is not bound",
                path.dotted(),
                volume.tileset
            ))
        })?;
        sindri_scene::TileSurfaces::derive(&volume, tile_set)
            .map(Some)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))
    }

    /// The map's size, and its palette when a flat map is what carries it.
    ///
    /// Size is geometry, which `sindri.tile_grid` describes as well as
    /// `sindri.tilemap` does, so `Grid.columns` and `Grid.rows` answer for a
    /// scene that has moved on to volumes. A palette is not geometry: it is the
    /// flat map's own idea of what a cell may hold, so a volume-only grid has
    /// none and the calls that need one say which component they are missing.
    fn map_shape(&self, path: &Path, map: EntityId) -> Result<MapShape, RuntimeError> {
        let data = self.world.get(map).ok_or_else(|| {
            RuntimeError::Host(format!("{}'s grid no longer exists", path.dotted()))
        })?;
        let (geometry, source) = geometry::read(path, data)?;
        Ok(MapShape {
            columns: geometry.columns,
            rows: geometry.rows,
            palette: match source {
                GridSource::Tilemap => Some(
                    data.components
                        .get(TilemapComponent::TYPE_NAME)
                        .and_then(|payload| payload.get("palette"))
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len),
                ),
                GridSource::TileGrid => None,
            },
        })
    }

    /// How many palette entries this map offers, refusing on a grid whose
    /// cells are not a flat map's to begin with.
    ///
    /// Every call that names a *cell* goes through here first, so a script
    /// reaching for `Grid.tile` on a volume is told which component it is
    /// missing rather than that a tilemap it never had has no tiles.
    fn flat_palette(path: &Path, shape: &MapShape) -> Result<usize, RuntimeError> {
        shape.palette.ok_or_else(|| {
            RuntimeError::Host(format!(
                "{} needs a {} on the entity it was given; a {} holds stacked cells rather than a flat palette",
                path.dotted(),
                TilemapComponent::TYPE_NAME,
                TILE_GRID_COMPONENT
            ))
        })
    }

    /// Which cell of the stored row-major array a column and row name, or
    /// `None` when they name somewhere the map does not reach.
    fn cell_index(
        path: &Path,
        args: &[Value],
        shape: &MapShape,
    ) -> Result<Option<usize>, RuntimeError> {
        let column = whole(path, args, 1, "a column")?;
        let row = whole(path, args, 2, "a row")?;
        if column < 0.0 || row < 0.0 {
            return Ok(None);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (column, row) = (column as usize, row as usize);
        if column >= shape.columns || row >= shape.rows {
            return Ok(None);
        }
        Ok(Some(row * shape.columns + column))
    }

    fn read_cell(&self, path: &Path, args: &[Value], index: usize) -> Result<f64, RuntimeError> {
        let map = self.entity_argument(path, args, 0, "the tilemap")?;
        let tiles = self
            .world
            .get(map)
            .and_then(|data| data.components.get(TilemapComponent::TYPE_NAME))
            .and_then(|payload| payload.get("tiles"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                RuntimeError::Host(format!("{}'s tilemap has no tiles", path.dotted()))
            })?;
        Ok(tiles
            .get(index)
            .and_then(serde_json::Value::as_u64)
            .and_then(|tile| u32::try_from(tile).ok())
            .map_or(f64::from(EMPTY), f64::from))
    }

    /// Writes one cell in place.
    ///
    /// Into the stored payload rather than through `TilemapComponent`, for the
    /// reason the sprite and shape paths do the same: the view is
    /// `Deserialize`-only, so rebuilding and reserializing it would silently
    /// drop any field the view does not know about.
    fn write_cell(
        &mut self,
        path: &Path,
        args: &[Value],
        index: usize,
        wanted: f64,
        shape: &MapShape,
    ) -> Result<Value, RuntimeError> {
        if wanted.fract() != 0.0 {
            return Err(RuntimeError::Host(format!(
                "{} needs a whole palette index, or {EMPTY} to empty the cell",
                path.dotted()
            )));
        }
        let palette = Self::flat_palette(path, shape)?;
        // Refused rather than stored, because a map holding an index its
        // palette cannot answer fails validation on the next load — long after
        // the script that wrote it ran, and nowhere near it.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let slot = (wanted >= 0.0).then_some(wanted as usize);
        if let Some(slot) = slot
            && slot >= palette
        {
            return Err(RuntimeError::Host(format!(
                "{} was given palette index {slot}, but the map's palette has {palette} sprite(s)",
                path.dotted()
            )));
        }
        let map = self.entity_argument(path, args, 0, "the tilemap")?;
        let tiles = self
            .world
            .get_mut(map)
            .and_then(|data| data.components.get_mut(TilemapComponent::TYPE_NAME))
            .and_then(|payload| payload.get_mut("tiles"))
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| {
                RuntimeError::Host(format!("{}'s tilemap has no tiles", path.dotted()))
            })?;
        let held = tiles.len();
        let cell = tiles.get_mut(index).ok_or_else(|| {
            RuntimeError::Host(format!(
                "{}'s tilemap holds {held} cells, which does not reach {index}",
                path.dotted()
            ))
        })?;
        *cell = slot.map_or(serde_json::Value::Null, |slot| {
            serde_json::Value::Number(slot.into())
        });
        Ok(Value::Unit)
    }
}

/// One argument, as a whole number.
fn whole(path: &Path, args: &[Value], at: usize, what: &str) -> Result<f64, RuntimeError> {
    let value = number(
        path,
        args.get(at)
            .ok_or_else(|| RuntimeError::Host(format!("{} needs {what}", path.dotted())))?,
    )?;
    if value.fract() != 0.0 {
        return Err(RuntimeError::Host(format!(
            "{} needs {what} that is a whole number",
            path.dotted()
        )));
    }
    Ok(value)
}

#[allow(clippy::cast_precision_loss)]
fn as_number(value: usize) -> f64 {
    value as f64
}
