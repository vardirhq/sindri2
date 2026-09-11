//! The strip along the top of every group, and the drag that rearranges it.
//!
//! One strip does the job a tab row and a panel header used to do between them.
//! That was one row of chrome too many — every docked panel spent twenty-odd
//! pixels saying its own name directly underneath a tab that had just said it —
//! and it left the editor with two competing ideas of what labels a region.
//! So the tab is the header: it names the panel, it selects it, it is what you
//! grab to move it, and the panel's own controls sit at the far end of the same
//! strip.

use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, Stroke, Vec2};

use crate::dock::{Drag, DropTarget, Panel as DockPanel, Place, Slot};
use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::tabs::{self, Shape, Weight};

use super::EditorApp;

/// The icon a panel is known by, wherever it is named.
pub(super) const fn panel_icon(panel: DockPanel) -> egui_material_icons::MaterialIcon {
    match panel {
        DockPanel::Scene => icons::WORLD,
        DockPanel::Game => icons::CAMERA,
        DockPanel::Hierarchy => icons::HIERARCHY,
        DockPanel::Inspector => icons::INSPECTOR,
        DockPanel::Project => icons::PROJECT,
        DockPanel::Console => icons::CONSOLE,
        DockPanel::History => icons::UNDO,
    }
}

impl EditorApp {
    /// Draws a group's tabs and returns the strip's rect and each tab's.
    ///
    /// The rects are what a drop is resolved against, so they are returned
    /// rather than recorded: the caller knows which slot this was, and a
    /// measurement that travels with the call cannot be attributed to the
    /// wrong one.
    pub(super) fn dock_strip(
        &mut self,
        ui: &mut egui::Ui,
        place: Place,
        panels: &[DockPanel],
        active: usize,
    ) -> (Rect, Vec<Rect>) {
        // Rounded when it is the top of a floating card, square when it is the
        // top of a dock that reaches the window's edges.
        let top_radius = if place.is_overlay() {
            crate::ui::widgets::panel::OVERLAY_RADIUS
        } else {
            0
        };
        self.dock_strip_as(ui, place, panels, active, Shape::Full { top_radius })
    }

    /// The same, saying whether the strip spans its container or stands where it
    /// is. Inline is for the title bar, where a full-width strip would swallow
    /// the rest of the bar.
    pub(super) fn dock_strip_as(
        &mut self,
        ui: &mut egui::Ui,
        place: Place,
        panels: &[DockPanel],
        active: usize,
        shape: Shape,
    ) -> (Rect, Vec<Rect>) {
        let weight = if matches!(place, Place::Dock(Slot::Main | Slot::MainBottom)) {
            Weight::Primary
        } else {
            Weight::Secondary
        };
        let dragging = self.dock.drag.map(|drag| drag.panel);
        let mut rects = Vec::with_capacity(panels.len());
        let mut chosen = None;
        let mut closed = None;
        let mut started = None;
        let mut rolled = false;
        let strip = tabs::row(ui, shape, |ui| {
            for (index, panel) in panels.iter().copied().enumerate() {
                let response = tabs::tab(
                    ui,
                    weight,
                    index == active,
                    Some(panel_icon(panel)),
                    panel.label(),
                    dragging == Some(panel),
                );
                rects.push(response.rect);
                if response.clicked() {
                    // Clicking the tab already showing rolls an overlay up to
                    // its strip and back down. That is the answer to the honest
                    // objection to overlays -- they cover the world -- and it
                    // costs no travel to a control somewhere else.
                    if index == active && place.is_overlay() {
                        rolled = true;
                    } else {
                        chosen = Some(index);
                    }
                }
                // Middle-click closes, the way it does in every editor and
                // every browser. The View menu puts it back.
                if response.middle_clicked() {
                    closed = Some(panel);
                }
                if response.drag_started()
                    && let Some(pointer) = ui.ctx().pointer_latest_pos()
                {
                    started = Some(Drag::new(panel, pointer));
                }
            }
            // The panel's own controls, at the far end of the strip that names
            // it. Only the showing panel's: a control belonging to a tab nobody
            // is looking at would act on something off screen.
            if let Some(panel) = panels.get(active).copied() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(metric::GUTTER);
                    self.panel_actions(ui, panel);
                });
            }
        });
        if let Some(index) = chosen {
            self.preferences.workspace.select(place, index);
        }
        if rolled {
            self.preferences.workspace.toggle_collapsed(place);
        }
        if let Some(panel) = closed {
            self.preferences.workspace.take(panel);
        }
        if let Some(drag) = started {
            self.dock.drag = Some(drag);
        }
        (strip, rects)
    }

    /// The showing panel's own controls, at the far end of its tab strip.
    ///
    /// Only the panels with controls worth a strip appear here. The rest say
    /// what they can do inside their own body, where there is room to label it.
    fn panel_actions(&mut self, ui: &mut egui::Ui, panel: DockPanel) {
        match panel {
            DockPanel::Hierarchy => self.hierarchy_actions(ui),
            DockPanel::Inspector => self.inspector_actions(ui),
            DockPanel::Game => {
                let note = self.game_device.note();
                ui.label(
                    egui::RichText::new(note)
                        .size(text::NOTE)
                        .color(color::TEXT_FAINT),
                );
            }
            // An error nobody is looking at is the reason the console exists,
            // so the count is shown on whichever strip the console is in,
            // whether or not the console is the tab showing.
            DockPanel::Console => {
                let counts = self.console.counts();
                if counts.errors > 0 {
                    crate::ui::widgets::toolbar::chip(ui, &counts.summary(), color::DANGER_TEXT);
                }
            }
            DockPanel::Scene | DockPanel::Project | DockPanel::History => {}
        }
    }

    /// The highlight that says where a released tab would land, and the label
    /// that follows the pointer while it has not been.
    pub(super) fn paint_drag(&self, context: &egui::Context, drag: &Drag) {
        let painter = context.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("dock-drag"),
        ));
        if let Some(DropTarget { place, index }) = drag.target {
            if let Some(zone) = self.dock.zones.iter().find(|zone| zone.place == place) {
                // A landing inside a group is shown as a wash over the whole
                // group plus a caret between the tabs it would land between, so
                // both halves of the answer — which group, and where in it —
                // are visible at once.
                let whole = zone.strip.union(zone.body);
                painter.rect_filled(whole, 2.0, color::FORGE.gamma_multiply(0.14));
                painter.rect_stroke(
                    whole,
                    2.0,
                    Stroke::new(1.5, color::FORGE),
                    egui::StrokeKind::Inside,
                );
                let caret = index
                    .checked_sub(1)
                    .and_then(|before| zone.tabs.get(before).map(Rect::right))
                    .or_else(|| zone.tabs.get(index).map(Rect::left))
                    .unwrap_or(zone.strip.left());
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(caret - 1.0, zone.strip.top() + 3.0),
                        Vec2::new(2.0, zone.strip.height() - 6.0),
                    ),
                    1.0,
                    color::FORGE_BRIGHT,
                );
            } else {
                // An empty slot has no zone to highlight, so the band along the
                // edge it would open is the highlight.
                painter.rect_filled(
                    empty_place_band(self.dock.canvas, place),
                    2.0,
                    color::FORGE.gamma_multiply(0.2),
                );
            }
        }
        // The tab itself, under the pointer, so the gesture has something in it.
        let label = drag.panel.label();
        let font = FontId::proportional(text::LABEL);
        let galley = painter.layout_no_wrap(label.to_owned(), font.clone(), color::TEXT);
        let size = Vec2::new(galley.size().x + 34.0, 26.0);
        let rect = Rect::from_min_size(drag.pointer + Vec2::new(12.0, 10.0), size);
        painter.rect_filled(rect, 3.0, color::RAISED);
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, color::FORGE),
            egui::StrokeKind::Inside,
        );
        let icon = panel_icon(drag.panel).outlined();
        painter.text(
            Pos2::new(rect.left() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            icon.codepoint,
            FontId::new(14.0, icon.font_family()),
            color::FORGE,
        );
        painter.galley(
            Pos2::new(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0),
            galley,
            Color32::PLACEHOLDER,
        );
    }
}

/// Where a place nothing is currently in would appear.
///
/// A dock is shown as the full band it would claim, because that is what
/// dropping there costs the scene. An overlay is shown at the size and corner
/// it would take, because that is what it would cover.
fn empty_place_band(canvas: Rect, place: Place) -> Rect {
    match place {
        Place::Dock(slot) => {
            let depth = 220.0_f32.min(canvas.width() * 0.25);
            match slot {
                Slot::FarLeft | Slot::Left => {
                    Rect::from_min_size(canvas.min, Vec2::new(depth, canvas.height()))
                }
                Slot::FarRight | Slot::Right => Rect::from_min_size(
                    Pos2::new(canvas.right() - depth, canvas.top()),
                    Vec2::new(depth, canvas.height()),
                ),
                Slot::Bottom | Slot::MainBottom | Slot::Main => Rect::from_min_size(
                    Pos2::new(canvas.left(), canvas.bottom() - 220.0),
                    Vec2::new(canvas.width(), 220.0),
                ),
            }
        }
        Place::Overlay(corner) => {
            let size = Vec2::new(260.0, 240.0);
            let x = if corner.is_left() {
                canvas.left() + 12.0
            } else {
                canvas.right() - 12.0 - size.x
            };
            let y = if corner.is_bottom() {
                canvas.bottom() - 12.0 - size.y
            } else {
                canvas.top() + 12.0
            };
            Rect::from_min_size(Pos2::new(x, y), size)
        }
    }
}
