//! Drawing whatever arrangement the workspace happens to be in.
//!
//! The frame loop used to name the panels in the order they claimed space, so
//! the arrangement was the code. Here it is the data: this walks [`Place::all`]
//! and draws whichever group it finds, and knows nothing about which panel that
//! turns out to be beyond how to draw each one.
//!
//! Docks are drawn first and claim space; the scene view is what is left. Then
//! overlays are drawn over that, anchored to its corners, because where an
//! overlay goes is not known until the docks have taken their share.
//!
//! Two things are worth knowing about the sizing. Every dock is given a
//! generous range rather than a tight one, because a resize handle with a few
//! pixels of travel reads as a broken control rather than as a considered
//! limit. And every body opens by claiming the full width of its slot: an
//! `egui::Panel` persists the size of its *contents*, not of itself, so a panel
//! whose contents are narrower than the space they were given collapses back to
//! its minimum on the next frame — which is why the project column could not be
//! widened, however far its edge was dragged.

use eframe::egui::{self, Rect};

use crate::dock::{Corner, Drag, DropTarget, Panel as DockPanel, Place, Slot};
use crate::ui::theme::metric;
use crate::ui::widgets::panel;

use super::{EditorApp, WorkspaceTab};

/// One group's geometry this frame, so a drop can be resolved against it.
///
/// Collected while drawing rather than predicted beforehand: the tab strip
/// measures its own tabs from the text in them, and a second guess at where
/// each one ended up would be a second layout that could disagree with the
/// first.
pub(super) struct Zone {
    pub(super) place: Place,
    pub(super) strip: Rect,
    pub(super) body: Rect,
    pub(super) tabs: Vec<Rect>,
}

/// What a drag needs to survive between frames, and what one frame measured.
pub(crate) struct DockLayout {
    pub(super) drag: Option<Drag>,
    pub(super) zones: Vec<Zone>,
    /// The scene view's rectangle, which is what overlays anchor to.
    ///
    /// Nothing until the centre has been drawn once. `Rect::ZERO` rather than
    /// an `Option`, because the only frame it is wrong on is the first, where
    /// an overlay drawn at the origin for one frame is invisible and an
    /// `Option` unwrapped at four call sites is not.
    pub(super) canvas: Rect,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            drag: None,
            zones: Vec::new(),
            canvas: Rect::ZERO,
        }
    }
}

/// How much of the window one dock may take.
///
/// A fraction rather than a number of points, so the limit means the same thing
/// on a laptop and on a monitor. It exists only to stop a dock swallowing the
/// window whole; anything short of that is the user's business.
const MAX_SHARE: f32 = 0.75;

/// The gap between an overlay and the edges of the scene it floats over, and
/// between two overlays stacked in the same corner.
const OVERLAY_GUTTER: f32 = 12.0;

impl EditorApp {
    /// Draws every place that holds anything: docks, then the scene, then the
    /// overlays over it.
    pub(super) fn workspace(&mut self, ui: &mut egui::Ui) {
        self.dock.zones.clear();
        let window = ui.ctx().content_rect();
        for slot in Slot::ALL {
            if slot == Slot::Main {
                self.draw_main(ui);
            } else {
                self.draw_dock(ui, slot, window);
            }
        }
        for corner in Corner::ALL {
            self.draw_overlays(ui, corner);
        }
        self.resolve_drag(ui);
    }

    /// One docked slot: an `egui::Panel` on the side the slot names.
    fn draw_dock(&mut self, ui: &mut egui::Ui, slot: Slot, window: Rect) {
        let place = Place::Dock(slot);
        let Some(group) = self.preferences.workspace.group(place) else {
            return;
        };
        let size = group.size;
        let column = slot.is_column();
        let max = if column {
            window.width() * MAX_SHARE
        } else {
            window.height() * MAX_SHARE
        };
        let edge = match slot {
            Slot::FarLeft | Slot::Left => egui::Panel::left(slot.id()),
            Slot::FarRight | Slot::Right => egui::Panel::right(slot.id()),
            Slot::Bottom | Slot::MainBottom => egui::Panel::bottom(slot.id()),
            // Drawn by `draw_main`, which has no edge to resize.
            Slot::Main => return,
        };
        let response = edge
            .default_size(size)
            .min_size(place.min_size())
            .max_size(max.max(place.min_size()))
            .resizable(true)
            .frame(self.place_frame(place))
            .show(ui, |ui| {
                // Claim the whole slot before anything else draws, or the
                // panel persists the size of its contents and springs back.
                // See `panel::fill_slot`.
                panel::fill_slot(ui, column);
                self.draw_group(ui, place);
            });
        let measured = if column {
            response.response.rect.width()
        } else {
            response.response.rect.height()
        };
        // Written back every frame, so the arrangement that is saved is the one
        // on screen rather than the one it started as.
        if (measured - size).abs() > 0.5 {
            self.preferences.workspace.resize(place, measured);
        }
    }

    /// The centre, which is whatever every other dock left behind.
    fn draw_main(&mut self, ui: &mut egui::Ui) {
        let response = egui::CentralPanel::default()
            .frame(self.place_frame(Place::MAIN))
            .show(ui, |ui| {
                self.draw_group(ui, Place::MAIN);
            });
        // Remembered for the overlays, which anchor to the scene rather than to
        // the window: an overlay in the top-left corner belongs against the
        // scene's corner, not underneath whatever dock is covering it.
        self.dock.canvas = response.response.rect;
    }

    /// The overlays anchored to one corner of the scene, stacked in tab order.
    ///
    /// Stacked rather than placed, so two overlays sharing a corner cannot end
    /// up on top of each other. The stack grows inwards from the corner: down
    /// from the top ones, up from the bottom ones.
    fn draw_overlays(&mut self, ui: &mut egui::Ui, corner: Corner) {
        let place = Place::Overlay(corner);
        let Some(group) = self.preferences.workspace.group(place) else {
            return;
        };
        let canvas = self.dock.canvas;
        let width = group.size.clamp(
            place.min_size(),
            (canvas.width() - OVERLAY_GUTTER * 2.0).max(place.min_size()),
        );
        let height = if group.collapsed {
            metric::HEADER_HEIGHT
        } else {
            group.height.clamp(
                metric::HEADER_HEIGHT,
                (canvas.height() - OVERLAY_GUTTER * 2.0).max(metric::HEADER_HEIGHT),
            )
        };
        let x = if corner.is_left() {
            canvas.left() + OVERLAY_GUTTER
        } else {
            canvas.right() - OVERLAY_GUTTER - width
        };
        let y = if corner.is_bottom() {
            canvas.bottom() - OVERLAY_GUTTER - height
        } else {
            canvas.top() + OVERLAY_GUTTER
        };
        let rect = Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height));
        egui::Area::new(egui::Id::new(place.id()))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            // Interactable so a click on an overlay is a click on the overlay
            // rather than a camera drag through it onto the scene behind.
            .interactable(true)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                panel::overlay_frame().show(ui, |ui| {
                    panel::fill_slot(ui, true);
                    self.draw_group(ui, place);
                });
            });
    }

    /// A place's frame: viewports sit on ink, everything else on panel ground.
    fn place_frame(&self, place: Place) -> egui::Frame {
        let showing_viewport = self
            .preferences
            .workspace
            .group(place)
            .and_then(crate::dock::Group::selected)
            .is_some_and(DockPanel::is_viewport);
        if showing_viewport {
            panel::viewport_frame()
        } else {
            panel::frame()
        }
    }

    /// A group: its tab strip, and the contents of whichever tab is showing.
    fn draw_group(&mut self, ui: &mut egui::Ui, place: Place) {
        let Some(group) = self.preferences.workspace.group(place) else {
            return;
        };
        let panels = group.panels.clone();
        let active = group.active_index();
        let collapsed = group.collapsed && place.is_overlay();
        let (strip, tabs) = self.dock_strip(ui, place, &panels, active);
        let body = ui.available_rect_before_wrap();
        self.dock.zones.push(Zone {
            place,
            strip,
            body,
            tabs,
        });
        if collapsed {
            return;
        }
        if let Some(panel) = panels.get(active).copied() {
            self.draw_panel(ui, panel);
        }
    }

    /// One panel's contents, with nothing around them.
    ///
    /// Every arm is a call into the module that owns that region: what a panel
    /// contains is that module's business, and where it is drawn is this one's.
    fn draw_panel(&mut self, ui: &mut egui::Ui, panel: DockPanel) {
        match panel {
            DockPanel::Scene => {
                self.scene_tools(ui, true);
                self.render_view(ui, WorkspaceTab::Scene);
            }
            DockPanel::Game => {
                self.game_tools(ui);
                self.render_view(ui, WorkspaceTab::Game);
            }
            DockPanel::Hierarchy => self.hierarchy_body(ui),
            DockPanel::Inspector => self.inspector_body(ui),
            DockPanel::Project => self.project_body(ui),
            DockPanel::Console => self.console_body(ui),
            DockPanel::History => self.history_body(ui),
        }
    }

    /// Works out where a dragged tab would land, shows it, and lands it.
    ///
    /// Drawn after every place because the highlight belongs over the panels
    /// rather than under whichever one happened to be drawn last, and resolved
    /// here because the answer needs every zone this frame measured.
    fn resolve_drag(&mut self, ui: &mut egui::Ui) {
        let Some(mut drag) = self.dock.drag else {
            return;
        };
        let context = ui.ctx();
        let pointer = context.pointer_latest_pos().unwrap_or(drag.pointer);
        drag.pointer = pointer;
        drag.target = self
            .drop_target(ui.ctx().content_rect(), pointer)
            // A refused move is never shown as one that will be obeyed: the
            // last tab in the centre stays where it is, and the highlight says
            // so by not appearing.
            .filter(|target| {
                self.preferences
                    .workspace
                    .can_place(drag.panel, target.place)
            });
        self.paint_drag(context, &drag);
        if context.input(|input| input.pointer.any_released()) {
            if let Some(DropTarget { place, index }) = drag.target {
                self.preferences.workspace.place(drag.panel, place, index);
            }
            self.dock.drag = None;
        } else {
            self.dock.drag = Some(drag);
        }
        context.request_repaint();
    }

    /// Which place and tab position a pointer is over.
    ///
    /// A tab strip first, because dropping between two tabs is the precise
    /// gesture and must win wherever the two overlap; then a body, which means
    /// "join this group at the end"; then the window's edge, which is the only
    /// way to reach a place that is currently empty. Overlays are searched
    /// before docks because they are drawn over them, so where they overlap the
    /// one on top is the one the pointer is pointing at.
    fn drop_target(&self, window: Rect, pointer: egui::Pos2) -> Option<DropTarget> {
        for overlays in [true, false] {
            for zone in self
                .dock
                .zones
                .iter()
                .filter(|zone| zone.place.is_overlay() == overlays)
            {
                if zone.strip.contains(pointer) {
                    return Some(DropTarget {
                        place: zone.place,
                        index: crate::dock::tab_index(&zone.tabs, pointer),
                    });
                }
            }
        }
        for overlays in [true, false] {
            for zone in self
                .dock
                .zones
                .iter()
                .filter(|zone| zone.place.is_overlay() == overlays)
            {
                if zone.body.contains(pointer) {
                    return Some(DropTarget {
                        place: zone.place,
                        index: usize::MAX,
                    });
                }
            }
        }
        crate::dock::edge_place(window, pointer).map(|place| DropTarget {
            place,
            index: usize::MAX,
        })
    }
}
