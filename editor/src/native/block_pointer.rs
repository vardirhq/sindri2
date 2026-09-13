//! Pointer projection and command-backed edits for stackable block volumes.

use eframe::egui::{Pos2, Rect};
use sindri_core::{CommandBuffer, EntityId, WorldCommand};
use sindri_scene::{CameraView, TileGridComponent};

use crate::tile_volume::{self, TileBrush, paint as paint_block};

use super::EditorApp;

pub(super) struct TileVolumeHover {
    pub(super) entity: EntityId,
    pub(super) coord: sindri_grid::GridCoord3,
    pub(super) outline: [Pos2; 4],
}

impl EditorApp {
    pub(super) fn tile_volume_hover(
        &self,
        rect: Rect,
        pointer: Option<Pos2>,
        camera: CameraView,
    ) -> Option<TileVolumeHover> {
        self.tile_volume_tool.brush()?;
        let pointer = pointer.filter(|pointer| rect.contains(*pointer))?;
        let entity = self.selection.primary()?;
        let data = self.world.get(entity)?;
        data.components.get(tile_volume::TYPE_NAME)?;
        let grid: TileGridComponent =
            serde_json::from_value(data.components.get(tile_volume::GRID_TYPE_NAME)?.clone())
                .ok()?;
        let transform = data.transform_3d.unwrap_or_default();
        let aspect = rect.width() / rect.height().max(1.0);
        let camera = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .ok()
            .flatten()?;
        let normalized = [
            (pointer.x - rect.min.x) / rect.width().max(1.0),
            (pointer.y - rect.min.y) / rect.height().max(1.0),
        ];
        let coord = tile_volume::cell_at_viewport(
            &grid,
            transform,
            camera.view_projection,
            normalized,
            self.tile_volume_tool.level,
        )?;
        let projected = tile_volume::cell_outline(&grid, transform, camera.view_projection, coord)?;
        let outline = projected.map(|point| {
            Pos2::new(
                rect.min.x + point[0] * rect.width(),
                rect.min.y + point[1] * rect.height(),
            )
        });
        Some(TileVolumeHover {
            entity,
            coord,
            outline,
        })
    }

    pub(super) fn apply_volume_brush(&mut self, hover: &TileVolumeHover) {
        if !self.authoring_enabled() {
            return;
        }
        let Some(mut payload) = self
            .world
            .get(hover.entity)
            .and_then(|data| data.components.get(tile_volume::TYPE_NAME))
            .cloned()
        else {
            return;
        };
        let chosen = self.tile_volume_tool.tile.clone();
        let brush = if self.tile_volume_tool.erase {
            TileBrush::Erase
        } else if let Some(chosen) = chosen.as_deref() {
            TileBrush::Tile(chosen)
        } else {
            return;
        };
        match paint_block(&mut payload, hover.coord, brush) {
            Ok(false) => return,
            Err(error) => {
                self.console.warning(error);
                return;
            }
            Ok(true) => {}
        }
        if let Err(error) = self
            .scene
            .components()
            .validate_payload(tile_volume::TYPE_NAME, &payload)
        {
            self.console
                .warning(format!("Block placement was refused: {error}"));
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity: hover.entity,
            type_name: tile_volume::TYPE_NAME.to_owned(),
            payload,
        });
        let transaction = buffer
            .into_transaction("Build tile volume")
            .merging(format!("tile-volume:{}", hover.entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }
}
