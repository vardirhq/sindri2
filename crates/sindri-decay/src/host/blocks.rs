//! Reading and writing one cell of a stacked volume.
//!
//! The flat-map calls beside these name a column and a row and answer with an
//! index into the map's own palette. A volume has neither shape: its cells are
//! named at an integer level as well as a column and row, and they name tiles
//! in a tile set several scenes may share rather than in a palette one map
//! carries.
//!
//! So these are separate calls rather than a third meaning for `Grid.tile`. A
//! script that asked for a tile index and got a level-shaped answer would be a
//! worse outcome than being told the two models are different.
//!
//! Empty is the empty string. A volume stores absence as absence — a cell that
//! is not there — so unlike the flat map it needs no sentinel number standing
//! in for nothing.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use serde_json::{Value as Json, json};
use sindri_core::{EntityId, SceneComponent};
use sindri_scene::TileVolumeComponent;

use crate::surface::GridCall;

use super::WorldHost;
use super::convert::number;

impl WorldHost<'_> {
    /// `Grid.block`, `Grid.set_block` and `Grid.tagged`.
    pub(super) fn block_call(
        &mut self,
        call: GridCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let map = self.entity_argument(path, args, 0, "the tile volume")?;
        let position = Self::block_position(path, args)?;
        match call {
            GridCall::Block => Ok(Value::String(self.read_block(path, map, position)?)),
            GridCall::SetBlock => {
                let tile = match args.get(4) {
                    Some(Value::String(tile)) => tile.clone(),
                    _ => {
                        return Err(RuntimeError::Host(format!(
                            "{} needs the name of a tile, or \"\" to clear the cell",
                            path.dotted()
                        )));
                    }
                };
                self.write_block(path, map, position, &tile)?;
                Ok(Value::Unit)
            }
            GridCall::Tagged => {
                let Some(Value::String(tag)) = args.get(4) else {
                    return Err(RuntimeError::Host(format!(
                        "{} needs the tag to ask about",
                        path.dotted()
                    )));
                };
                let tile = self.read_block(path, map, position)?;
                if tile.is_empty() {
                    return Ok(Value::Bool(false));
                }
                let tile_set = self.tile_set_of(path, map, &tile)?;
                Ok(Value::Bool(tile_set.tile(&tile).is_some_and(|block| {
                    block.tags.iter().any(|carried| carried == tag)
                })))
            }
            _ => unreachable!("dispatched to the flat-map calls instead"),
        }
    }

    /// The column, row and level a block call names.
    ///
    /// Whole numbers, because a cell between two levels is not a cell. Refused
    /// rather than rounded: a script computing a level from a float it got
    /// wrong should hear about it where the mistake is.
    fn block_position(path: &Path, args: &[Value]) -> Result<[i32; 3], RuntimeError> {
        let mut position = [0_i32; 3];
        for (index, which) in [(1, "a column"), (2, "a row"), (3, "a level")] {
            let value = number(
                path,
                args.get(index).ok_or_else(|| {
                    RuntimeError::Host(format!("{} needs {which}", path.dotted()))
                })?,
            )?;
            if value.fract() != 0.0 {
                return Err(RuntimeError::Host(format!(
                    "{} needs {which} that is a whole number",
                    path.dotted()
                )));
            }
            #[allow(clippy::cast_possible_truncation)]
            let whole = value as i64;
            position[index - 1] = i32::try_from(whole).map_err(|_| {
                RuntimeError::Host(format!(
                    "{} was given {which} far outside any grid",
                    path.dotted()
                ))
            })?;
        }
        Ok(position)
    }

    fn read_block(
        &self,
        path: &Path,
        map: EntityId,
        position: [i32; 3],
    ) -> Result<String, RuntimeError> {
        let cells = self.volume_cells(path, map)?;
        Ok(cells
            .iter()
            .find(|cell| Self::cell_position(cell) == Some(position))
            .and_then(|cell| cell.get("tile"))
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    /// Writes one cell in place, leaving every other field of the volume alone.
    ///
    /// Into the stored payload rather than through `TileVolumeComponent`, for
    /// the reason the sprite and tilemap paths do the same: the view is
    /// `Deserialize`-only, so rebuilding and reserializing it would drop any
    /// field this build does not know about — a deliberate visual override
    /// among them.
    fn write_block(
        &mut self,
        path: &Path,
        map: EntityId,
        position: [i32; 3],
        tile: &str,
    ) -> Result<(), RuntimeError> {
        if !tile.is_empty() {
            self.check_tile_is_defined(path, map, tile)?;
        }
        let cells = self
            .world
            .get_mut(map)
            .and_then(|data| data.components.get_mut(TileVolumeComponent::TYPE_NAME))
            .and_then(|payload| payload.get_mut("cells"))
            .and_then(Json::as_array_mut)
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{}'s {} has no cells",
                    path.dotted(),
                    TileVolumeComponent::TYPE_NAME
                ))
            })?;
        let existing = cells
            .iter()
            .position(|cell| Self::cell_position(cell) == Some(position));
        match (existing, tile.is_empty()) {
            (Some(index), true) => {
                cells.remove(index);
            }
            (Some(index), false) => {
                cells[index]["tile"] = json!(tile);
            }
            (None, true) => {}
            (None, false) => cells.push(json!({ "position": position, "tile": tile })),
        }
        Ok(())
    }

    /// Refuses a tile the volume's own tile set does not define.
    ///
    /// A cell naming a tile nothing can resolve fails the next extraction,
    /// which is a frame later and nowhere near the script that wrote it. The
    /// host has the tile set precisely so the refusal lands on the call.
    fn check_tile_is_defined(
        &self,
        path: &Path,
        map: EntityId,
        tile: &str,
    ) -> Result<(), RuntimeError> {
        let tile_set = self.tile_set_of(path, map, tile)?;
        if tile_set.tile(tile).is_none() {
            return Err(RuntimeError::Host(format!(
                "{} was given tile `{tile}`, which the volume's tile set does not define",
                path.dotted()
            )));
        }
        Ok(())
    }

    /// The tile set a volume names, from the ones this host has bound.
    fn tile_set_of(
        &self,
        path: &Path,
        map: EntityId,
        tile: &str,
    ) -> Result<&sindri_core::TileSetDocument, RuntimeError> {
        let name = self
            .world
            .get(map)
            .and_then(|data| data.components.get(TileVolumeComponent::TYPE_NAME))
            .and_then(|payload| payload.get("tileset"))
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned();
        let Some(tile_sets) = self.tile_sets else {
            return Err(RuntimeError::Host(format!(
                "{} cannot check tile `{tile}`: this host has loaded no tile sets",
                path.dotted()
            )));
        };
        tile_sets.get(&name).ok_or_else(|| {
            RuntimeError::Host(format!(
                "{} cannot check tile `{tile}`: tile set `{name}` is not bound",
                path.dotted()
            ))
        })
    }

    fn volume_cells(&self, path: &Path, map: EntityId) -> Result<&Vec<Json>, RuntimeError> {
        self.world
            .get(map)
            .and_then(|data| data.components.get(TileVolumeComponent::TYPE_NAME))
            .and_then(|payload| payload.get("cells"))
            .and_then(Json::as_array)
            .ok_or_else(|| {
                RuntimeError::Host(format!(
                    "{} needs a {} on the entity it was given",
                    path.dotted(),
                    TileVolumeComponent::TYPE_NAME
                ))
            })
    }

    fn cell_position(cell: &Json) -> Option<[i32; 3]> {
        let stored = cell.get("position")?.as_array()?;
        let [x, y, z] = stored.as_slice() else {
            return None;
        };
        Some([
            i32::try_from(x.as_i64()?).ok()?,
            i32::try_from(y.as_i64()?).ok()?,
            i32::try_from(z.as_i64()?).ok()?,
        ])
    }
}
