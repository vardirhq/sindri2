//! Pointer projection and command-backed edits for stackable block volumes.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Shape, Stroke};
use sindri_core::{CommandBuffer, EntityId, WorldCommand};
use sindri_scene::{CameraView, TileGridComponent};

use crate::tile_volume::{self, TileBrush, TilePlacement, paint as paint_block};
use crate::ui::theme::{color, text};

use super::EditorApp;

/// How far a click reaches into a scene, in cells. Far enough to cross any
/// volume worth building by hand, and short enough that a click at the sky
/// stops rather than walking to the horizon.
const PICK_REACH: f32 = 512.0;

pub(super) struct TileVolumeHover {
    pub(super) entity: EntityId,
    pub(super) coord: sindri_grid::GridCoord3,
    /// Where a block goes if one is placed, and where one is taken from.
    ///
    /// On a solid grid these differ: a click attaches against the side you are
    /// looking at, and removes the block that side belongs to. On a flattened
    /// one there is no side to speak of, so both are the cell the plane hit.
    pub(super) against: sindri_grid::GridCoord3,
    pub(super) outline: [Pos2; 4],
}

impl EditorApp {
    pub(super) fn paint_tile_volume_hover(&self, ui: &egui::Ui, hover: &TileVolumeHover) {
        let tint = if self.tile_volume_tool.erase {
            color::DANGER
        } else {
            color::FORGE
        };
        ui.painter().add(Shape::convex_polygon(
            hover.outline.to_vec(),
            tint.gamma_multiply(0.16),
            Stroke::new(2.0, tint),
        ));
        ui.painter().text(
            hover.outline[0],
            Align2::LEFT_BOTTOM,
            format!("{}, {}, {}", hover.coord.x, hover.coord.y, hover.coord.z),
            FontId::proportional(text::NOTE),
            color::TEXT,
        );
    }

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
        let volume = tile_volume::component(data.components.get(tile_volume::TYPE_NAME)?).ok()?;
        let grid: TileGridComponent =
            serde_json::from_value(data.components.get(tile_volume::GRID_TYPE_NAME)?.clone())
                .ok()?;
        let transform = self.world.world_transform(entity).unwrap_or_default();
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
        // A grid whose cells are boxes is picked by walking the blocks, which
        // is the only way to know which *side* of one the pointer is over.
        // Everything the other branch needs -- a target mode, a level, whether
        // this click places or removes -- exists because a plane cannot say.
        if let Some(cell_size) = grid.solid_cell() {
            let (origin, direction) =
                tile_volume::ray_at_viewport(transform, camera.view_projection, normalized)?;
            let hit = sindri_scene::voxel::pick(&volume, cell_size, origin, direction, PICK_REACH)?;
            let corners = sindri_scene::voxel::face_quad(hit.cell, hit.face, cell_size);
            let model = tile_volume::model_of(transform);
            let outline = corners.map(|corner| {
                let clip = camera.view_projection * model.transform_point3(corner).extend(1.0);
                let ndc = clip.truncate() / clip.w;
                Pos2::new(
                    rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width(),
                    rect.min.y + (0.5 - ndc.y * 0.5) * rect.height(),
                )
            });
            return Some(TileVolumeHover {
                entity,
                coord: hit.cell,
                against: hit.against(),
                outline,
            });
        }
        let coord = match self.tile_volume_tool.placement {
            TilePlacement::Surface => tile_volume::surface_target_at_viewport(
                &grid,
                transform,
                camera.view_projection,
                normalized,
                &volume,
                self.tile_volume_tool.erase,
                self.tile_volume_tool.level,
            ),
            TilePlacement::Level => tile_volume::cell_at_viewport(
                &grid,
                transform,
                camera.view_projection,
                normalized,
                self.tile_volume_tool.level,
            ),
        }?;
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
            against: coord,
            outline,
        })
    }

    pub(super) fn apply_volume_brush(&mut self, hover: &TileVolumeHover, remove: bool) {
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
        let brush = if remove {
            TileBrush::Erase
        } else if let Some(chosen) = chosen.as_deref() {
            TileBrush::Tile(chosen)
        } else {
            return;
        };
        // Removing takes the block you clicked; placing attaches against the
        // side of it you are looking at. On a flattened grid these are the same
        // cell, because there is no side to have clicked.
        let cell = if remove { hover.coord } else { hover.against };
        match paint_block(&mut payload, cell, brush) {
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
