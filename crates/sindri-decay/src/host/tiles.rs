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

use crate::surface::GridCall;

use super::WorldHost;
use super::convert::number;

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
    palette: usize,
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
        let map = self.entity_argument(path, args, 0, "the tilemap")?;
        let shape = self.map_shape(path, map)?;

        match call {
            GridCall::Columns => Ok(Value::Number(as_number(shape.columns))),
            GridCall::Rows => Ok(Value::Number(as_number(shape.rows))),
            GridCall::Tile => {
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
        }
    }

    /// The map's size and palette, read from the payload rather than through
    /// the typed view, so a map carrying a field this build does not know about
    /// is still answerable.
    fn map_shape(&self, path: &Path, map: EntityId) -> Result<MapShape, RuntimeError> {
        let payload = self
            .world
            .get(map)
            .and_then(|data| data.components.get(TilemapComponent::TYPE_NAME))
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{} needs a {} on the entity it was given",
                    path.dotted(),
                    TilemapComponent::TYPE_NAME
                ))
            })?;
        let read = |field: &str| -> Result<usize, RuntimeError> {
            usize::try_from(
                payload
                    .get(field)
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| {
                        RuntimeError::Host(format!("{}'s tilemap has no {field}", path.dotted()))
                    })?,
            )
            .map_err(|_| {
                RuntimeError::Host(format!("{}'s tilemap {field} is too large", path.dotted()))
            })
        };
        Ok(MapShape {
            columns: read("columns")?,
            rows: read("rows")?,
            palette: payload
                .get("palette")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
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
        // Refused rather than stored, because a map holding an index its
        // palette cannot answer fails validation on the next load — long after
        // the script that wrote it ran, and nowhere near it.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let slot = (wanted >= 0.0).then_some(wanted as usize);
        if let Some(slot) = slot
            && slot >= shape.palette
        {
            return Err(RuntimeError::Host(format!(
                "{} was given palette index {slot}, but the map's palette has {} sprite(s)",
                path.dotted(),
                shape.palette
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
