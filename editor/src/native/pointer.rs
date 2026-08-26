//! Turning a pointer in a viewport into a selection, a drag, or a painted tile.

use eframe::egui::{self, Pos2, Rect, Response};
use glam::Vec2 as GlamVec2;
use sindri_core::{CommandBuffer, EntityId, Transform3D, WorldCommand};
use sindri_scene::{
    CameraView, OverlayView, UiAnchor, UiImageComponent, UiTextComponent, ViewCamera,
    overlay_for_viewport,
};

use crate::{
    gizmo::{self, Anchoring, GizmoDrag},
    picking,
    tilemap::{self, TileBrush, paint as paint_tile},
};

use super::EditorApp;
use super::{UI_IMAGE_COMPONENT, UI_TEXT_COMPONENT};

/// One tile under the Scene-view pointer, already projected back into the
/// viewport so input and its feedback use the same answer.
pub(super) struct TilemapHover {
    pub(super) entity: EntityId,
    pub(super) column: u32,
    pub(super) row: u32,
    pub(super) outline: [Pos2; 4],
}

impl EditorApp {
    /// Resolves the selected tilemap and pointer through the same camera used
    /// for this frame.
    pub(super) fn tilemap_hover(
        &self,
        rect: Rect,
        pointer: Option<Pos2>,
        camera: CameraView,
    ) -> Option<TilemapHover> {
        self.tilemap_tool.brush()?;
        let pointer = pointer.filter(|pointer| rect.contains(*pointer))?;
        let entity = self.selection?;
        let data = self.world.get(entity)?;
        let payload = data.components.get(tilemap::TYPE_NAME)?;
        let map = tilemap::component(payload).ok()?;
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
        let (column, row) =
            tilemap::tile_at_viewport(&map, transform, camera.view_projection, normalized)?;
        let projected =
            tilemap::tile_outline(&map, transform, camera.view_projection, column, row)?;
        let outline = projected.map(|point| {
            Pos2::new(
                rect.min.x + point[0] * rect.width(),
                rect.min.y + point[1] * rect.height(),
            )
        });
        Some(TilemapHover {
            entity,
            column,
            row,
            outline,
        })
    }

    /// Resolves a Scene-view point through the exact camera that drew it.
    fn pick_viewport(
        &self,
        rect: Rect,
        pointer: Pos2,
        camera: CameraView,
    ) -> Result<Option<EntityId>, String> {
        if !rect.contains(pointer) {
            return Ok(None);
        }
        let aspect = rect.width() / rect.height().max(1.0);
        let Some(camera) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let point = [
            (pointer.x - rect.min.x) / rect.width().max(1.0),
            (pointer.y - rect.min.y) / rect.height().max(1.0),
        ];
        // The overlay is drawn over the world, so what is under the pointer
        // there wins. Asked first for that reason rather than as a fallback.
        if let Some((overlay, placement)) = overlay_for_viewport(aspect)
            && let Some(entity) = picking::pick_ui(
                &self.world,
                self.scene.components(),
                overlay,
                &placement,
                point,
            )
            .map_err(|error| error.to_string())?
        {
            return Ok(Some(entity));
        }
        picking::pick_world(
            &self.world,
            self.scene.components(),
            camera.view_projection,
            point,
        )
        .map_err(|error| error.to_string())
    }

    /// Whether this entity is laid out against the viewport rather than in the
    /// world, and so is drawn — and gizmoed — on the overlay.
    fn is_overlaid(&self, entity: EntityId) -> bool {
        self.world.get(entity).is_some_and(|data| {
            data.components.contains_key(UI_IMAGE_COMPONENT)
                || data.components.contains_key(UI_TEXT_COMPONENT)
        })
    }

    /// Applies a primary Scene-view click without taking drags or paint strokes.
    pub(super) fn select_viewport_click(
        &mut self,
        rect: Rect,
        response: &Response,
        camera: CameraView,
        painting: bool,
    ) {
        if painting || !response.clicked_by(egui::PointerButton::Primary) {
            return;
        }
        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };
        match self.pick_viewport(rect, pointer, camera) {
            Ok(entity) => self.select(entity),
            Err(error) => self
                .console
                .warning(format!("Viewport selection failed: {error}")),
        }
    }

    /// Writes one cell through the command layer. Repeated calls during one
    /// drag share a merge key, and pointer release closes that merge run.
    ///
    /// Refused while the scene is playing, for the reason every other world
    /// write is: Stop would throw the painting away.
    pub(super) fn apply_tile_brush(&mut self, hover: &TilemapHover) {
        if !self.authoring_enabled() {
            return;
        }
        let Some(original) = self
            .world
            .get(hover.entity)
            .and_then(|data| data.components.get(tilemap::TYPE_NAME))
            .cloned()
        else {
            return;
        };
        let chosen = self.tilemap_tool.sprite.clone();
        let brush = if self.tilemap_tool.erase {
            TileBrush::Erase
        } else if let Some(chosen) = chosen.as_deref() {
            TileBrush::Sprite(chosen)
        } else {
            return;
        };
        let mut payload = original;
        match paint_tile(&mut payload, hover.column, hover.row, brush) {
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
            .validate_payload(tilemap::TYPE_NAME, &payload)
        {
            self.console
                .warning(format!("Tilemap paint was refused: {error}"));
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity: hover.entity,
            type_name: tilemap::TYPE_NAME.to_owned(),
            payload,
        });
        let transaction = buffer
            .into_transaction("Paint tilemap")
            .merging(format!("tilemap:{}", hover.entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Resolves the selected transform into the paths used by both drawing and
    /// hit-testing. The visible handle is therefore the handle the pointer can
    /// actually take.
    pub(super) fn gizmo_visual(
        &self,
        rect: Rect,
        camera: CameraView,
    ) -> Option<(ViewCamera, Anchoring, gizmo::GizmoVisual)> {
        let entity = self.selection?;
        let transform = self.world.get(entity)?.transform_3d?;
        let aspect = rect.width() / rect.height().max(1.0);
        // Which space this entity is drawn in decides which camera its handle
        // is projected through. Drawn through the world camera regardless, a
        // UI element's handle appeared wherever its anchor offset happened to
        // point in the world rather than where the element is.
        let (camera, anchoring) = self.gizmo_camera(entity, aspect, transform, camera)?;
        let visual = gizmo::visual(
            self.gizmo_mode,
            anchoring,
            transform,
            self.gizmo_space,
            camera.view_projection,
            GlamVec2::new(rect.width(), rect.height()),
            camera.framed_half_height,
        )?;
        Some((camera, anchoring, visual))
    }

    /// The camera an entity's handles are drawn through, and where they sit.
    fn gizmo_camera(
        &self,
        entity: EntityId,
        aspect: f32,
        transform: Transform3D,
        camera: CameraView,
    ) -> Option<(ViewCamera, Anchoring)> {
        if self.is_overlaid(entity) {
            let (overlay, placement) = overlay_for_viewport(aspect)?;
            let anchor = self.ui_anchor(entity);
            return Some((
                as_view_camera(overlay),
                Anchoring::on_overlay(placement.origin(transform, anchor)),
            ));
        }
        let camera = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .ok()
            .flatten()?;
        Some((camera, Anchoring::in_world(transform)))
    }

    /// Which corner of the viewport a UI element is measured from.
    ///
    /// Read from whichever of the two UI components the entity carries; a
    /// default anchor for one that carries neither is unreachable, because
    /// nothing is overlaid without one.
    fn ui_anchor(&self, entity: EntityId) -> UiAnchor {
        let Some(data) = self.world.get(entity) else {
            return UiAnchor::default();
        };
        data.components
            .get(UI_IMAGE_COMPONENT)
            .and_then(|payload| serde_json::from_value::<UiImageComponent>(payload.clone()).ok())
            .map(|image| image.anchor)
            .or_else(|| {
                data.components
                    .get(UI_TEXT_COMPONENT)
                    .and_then(|payload| {
                        serde_json::from_value::<UiTextComponent>(payload.clone()).ok()
                    })
                    .map(|text| text.anchor)
            })
            .unwrap_or_default()
    }

    /// Gives a transform handle first claim on primary drag and writes every
    /// intermediate answer through command history.
    ///
    /// Stands down entirely while the scene is playing: a drag is a world
    /// write, and Stop restores the world as it was when Play was pressed. It
    /// returns `false` there so the primary drag falls back to orbiting, which
    /// is what a viewport that cannot be edited is still good for.
    pub(super) fn interact_gizmo(
        &mut self,
        rect: Rect,
        response: &Response,
        camera: ViewCamera,
        anchoring: Anchoring,
        visual: &gizmo::GizmoVisual,
    ) -> bool {
        if !self.authoring_enabled() {
            self.gizmo_drag = None;
            return false;
        }
        let pointer = response
            .interact_pointer_pos()
            .map(|pointer| GlamVec2::new(pointer.x - rect.min.x, pointer.y - rect.min.y));
        let hovered = pointer.and_then(|pointer| gizmo::hit_test(visual, pointer));
        let owns_primary = self.gizmo_drag.is_some() || hovered.is_some();

        if response.drag_started_by(egui::PointerButton::Primary)
            && let (Some(entity), Some(axis), Some(pointer)) = (self.selection, hovered, pointer)
            && let Some(transform) = self.world.get(entity).and_then(|data| data.transform_3d)
        {
            self.gizmo_drag = gizmo::begin_drag(
                entity,
                self.gizmo_mode,
                axis,
                anchoring,
                transform,
                self.gizmo_space,
                camera.view_projection,
                pointer,
                GlamVec2::new(rect.width(), rect.height()),
            );
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let (Some(drag), Some(pointer)) = (self.gizmo_drag, pointer)
            && let Some(next) = gizmo::update_drag(
                drag,
                camera.view_projection,
                pointer,
                GlamVec2::new(rect.width(), rect.height()),
                self.gizmo_snapping,
            )
        {
            self.apply_gizmo_transform(drag, next);
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            self.gizmo_drag = None;
        }
        owns_primary
    }

    /// A whole drag is one undo step even though its current answer is applied
    /// every frame, because all of its transactions share this merge key.
    fn apply_gizmo_transform(&mut self, drag: GizmoDrag, transform: Transform3D) {
        if self
            .world
            .get(drag.entity)
            .and_then(|data| data.transform_3d)
            == Some(transform)
        {
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetTransform3D {
            entity: drag.entity,
            transform: Some(transform),
        });
        let transaction = buffer
            .into_transaction(format!("{} entity", drag.mode.label()))
            .merging(format!(
                "gizmo:{}:{}",
                drag.entity.index(),
                drag.mode.label()
            ));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
            self.gizmo_drag = None;
        }
    }
}

/// The overlay described the way a viewport's chrome asks for a camera.
///
/// The two carry the same three facts, and the gizmo does not care which of
/// them it is drawing through — only that the matrix and the framed height
/// belong to the same picture.
fn as_view_camera(overlay: OverlayView) -> ViewCamera {
    ViewCamera {
        view: glam::Mat4::IDENTITY,
        view_projection: overlay.view_projection,
        framed_half_height: overlay.framed_half_height,
    }
}
