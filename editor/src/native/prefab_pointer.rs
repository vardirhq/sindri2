//! Putting a chosen prefab on the cell somebody clicked.
//!
//! The block brush beside this one picks against the *selected* entity, because
//! building blocks is something you do to one volume you are looking at.
//! Placing a prefab is not: choosing it in the browser clears the entity
//! selection, so there is nothing selected to pick against, and the question is
//! "which grid did that click land on" rather than "where on this one".
//!
//! So every grid in the scene is asked, and the frontmost answer wins — the
//! same tie-break the renderer uses, so the cell an author sees on top is the
//! one they get.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Shape, Stroke};
use sindri_core::{CommandBuffer, EntityId, SceneEntityId};
use sindri_grid::GridCoord3;
use sindri_scene::{CameraView, TileGridComponent};

use super::EditorApp;
use crate::prefab;
use crate::tile_volume;
use crate::ui::theme::{color, text};

/// A cell a prefab would land on, and the grid that owns it.
pub(super) struct PrefabTarget {
    pub(super) grid: SceneEntityId,
    pub(super) coord: GridCoord3,
    pub(super) outline: [Pos2; 4],
}

impl EditorApp {
    /// The cell under the pointer, when a prefab is armed to be placed.
    pub(super) fn prefab_target(
        &self,
        rect: Rect,
        pointer: Option<Pos2>,
        camera: CameraView,
    ) -> Option<PrefabTarget> {
        let brush = self.prefab_brush.as_ref()?;
        if !brush.placing || brush.document().is_none() {
            return None;
        }
        let pointer = pointer.filter(|pointer| rect.contains(*pointer))?;
        let normalized = [
            (pointer.x - rect.min.x) / rect.width().max(1.0),
            (pointer.y - rect.min.y) / rect.height().max(1.0),
        ];
        let aspect = rect.width() / rect.height().max(1.0);
        let camera = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .ok()
            .flatten()?;

        let mut best: Option<(GridCoord3, SceneEntityId, TileGridComponent, EntityId)> = None;
        for (entity, data) in self.world.entities() {
            let Some(id) = data.source_id.clone() else {
                continue;
            };
            let Some(grid_payload) = data.components.get(tile_volume::GRID_TYPE_NAME) else {
                continue;
            };
            let Ok(grid) = serde_json::from_value::<TileGridComponent>(grid_payload.clone()) else {
                continue;
            };
            let Some(volume) = data
                .components
                .get(tile_volume::TYPE_NAME)
                .and_then(|payload| tile_volume::component(payload).ok())
            else {
                continue;
            };
            let transform = self.world.world_transform(entity).unwrap_or_default();
            let Some(coord) = tile_volume::surface_cell_at_viewport(
                &grid,
                transform,
                camera.view_projection,
                normalized,
                &volume,
            ) else {
                continue;
            };
            // Frontmost wins, which is the cell whose top face is the one drawn
            // over the others.
            if best.as_ref().is_none_or(|(chosen, _, chosen_grid, _)| {
                grid.depth_key(coord) > chosen_grid.depth_key(*chosen)
            }) {
                best = Some((coord, id, grid, entity));
            }
        }

        let (coord, id, grid, entity) = best?;
        let transform = self.world.world_transform(entity).unwrap_or_default();
        let projected = tile_volume::cell_outline(&grid, transform, camera.view_projection, coord)?;
        let outline = projected.map(|point| {
            Pos2::new(
                rect.min.x + point[0] * rect.width(),
                rect.min.y + point[1] * rect.height(),
            )
        });
        Some(PrefabTarget {
            grid: id,
            coord,
            outline,
        })
    }

    /// The cell an armed prefab is pointing at, placing it if this is a click.
    ///
    /// Asked before the block brush, because arming a prefab is deliberate and
    /// the block brush is whatever was last held.
    pub(super) fn prefab_interaction(
        &mut self,
        rect: Rect,
        response: &egui::Response,
        camera: CameraView,
        editing: bool,
    ) -> Option<PrefabTarget> {
        if !editing {
            return None;
        }
        let target = self.prefab_target(rect, response.hover_pos(), camera)?;
        if response.clicked_by(egui::PointerButton::Primary) {
            self.place_prefab(&target);
        }
        Some(target)
    }

    /// Puts the armed prefab on that cell, as one undoable step.
    pub(super) fn place_prefab(&mut self, target: &PrefabTarget) {
        if !self.authoring_enabled() {
            return;
        }
        let Some(document) = self
            .prefab_brush
            .as_ref()
            .and_then(|brush| brush.document().cloned())
        else {
            return;
        };
        let label = self
            .prefab_brush
            .as_ref()
            .map_or_else(|| "Prefab".to_owned(), crate::prefab::PrefabBrush::name);

        let mut rehearsal = self.world.clone();
        let mut buffer = CommandBuffer::new();
        let root = match prefab::instantiate_on_cell(
            &mut rehearsal,
            &document,
            &target.grid,
            [target.coord.x, target.coord.y],
            &mut buffer,
        ) {
            Ok(root) => root,
            Err(error) => {
                self.console.warning(error);
                return;
            }
        };
        self.history.break_merge_run();
        if let Err(error) = self.history.apply(
            buffer.into_transaction(format!("Place {label}")),
            &mut self.world,
        ) {
            self.report(error.to_string());
            return;
        }
        self.select(Some(root));
        self.refresh_textures();
    }
}

/// The cell a prefab would land on, outlined.
///
/// A free function because it reads nothing but the target: where the cell is
/// was worked out when the pointer was resolved, and drawing it again from the
/// editor would be deriving the same answer twice.
pub(super) fn paint_prefab_target(ui: &egui::Ui, target: Option<&PrefabTarget>) {
    let Some(target) = target else {
        return;
    };
    ui.painter().add(Shape::convex_polygon(
        target.outline.to_vec(),
        color::FORGE.gamma_multiply(0.16),
        Stroke::new(2.0, color::FORGE),
    ));
    ui.painter().text(
        target.outline[0],
        Align2::LEFT_BOTTOM,
        format!("{}, {}", target.coord.x, target.coord.y),
        FontId::proportional(text::NOTE),
        color::TEXT,
    );
}
