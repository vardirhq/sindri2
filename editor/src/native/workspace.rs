//! Drawing whatever arrangement the workspace happens to be in.
//!
//! The frame loop used to name the panels in the order they claimed space, so
//! the arrangement was the code. Here it is the data: this walks [`Slot::ALL`]
//! and draws whichever group it finds, and knows nothing about which panel
//! that turns out to be beyond how to draw each one.
//!
//! Two things are worth knowing about the sizing. Every slot is given a
//! generous range rather than a tight one, because a resize handle with a few
//! pixels of travel reads as a broken control rather than as a considered
//! limit. And every body opens by claiming the full width of its slot: an
//! `egui::Panel` persists the size of its *contents*, not of itself, so a panel
//! whose contents are narrower than the space they were given collapses back to
//! its minimum on the next frame — which is why the project column could not be
//! widened, however far its edge was dragged.

use eframe::egui::{self, Rect};

use crate::dock::{Drag, DropTarget, Panel as DockPanel, Slot};
use crate::ui::widgets::panel;

use super::{EditorApp, WorkspaceTab};

/// One group's geometry this frame, so a drop can be resolved against it.
///
/// Collected while drawing rather than predicted beforehand: the tab strip
/// measures its own tabs from the text in them, and a second guess at where
/// each one ended up would be a second layout that could disagree with the
/// first.
pub(super) struct Zone {
    pub(super) slot: Slot,
    pub(super) strip: Rect,
    pub(super) body: Rect,
    pub(super) tabs: Vec<Rect>,
}

/// What a drag needs to survive between frames, and what one frame measured.
#[derive(Default)]
pub(crate) struct DockLayout {
    pub(super) drag: Option<Drag>,
    pub(super) zones: Vec<Zone>,
}

/// How much of the window one slot may take.
///
/// A fraction rather than a number of points, so the limit means the same thing
/// on a laptop and on a monitor. It exists only to stop a slot swallowing the
/// window whole; anything short of that is the user's business.
const MAX_SHARE: f32 = 0.75;

impl EditorApp {
    /// Draws every slot that holds anything, outermost first.
    pub(super) fn workspace(&mut self, ui: &mut egui::Ui) {
        self.dock.zones.clear();
        let window = ui.ctx().content_rect();
        for slot in Slot::ALL {
            if slot == Slot::Main {
                self.draw_main(ui);
            } else {
                self.draw_slot(ui, slot, window);
            }
        }
        self.resolve_drag(ui);
    }

    /// One docked slot: an `egui::Panel` on the side the slot names.
    fn draw_slot(&mut self, ui: &mut egui::Ui, slot: Slot, window: Rect) {
        let Some(group) = self.preferences.workspace.group(slot) else {
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
            .min_size(slot.min_size())
            .max_size(max.max(slot.min_size()))
            .resizable(true)
            .frame(self.slot_frame(slot))
            .show(ui, |ui| {
                // Claim the whole slot before anything else draws, or the
                // panel persists the size of its contents and springs back.
                // See `panel::fill_slot`.
                panel::fill_slot(ui, column);
                self.draw_group(ui, slot);
            });
        let measured = if column {
            response.response.rect.width()
        } else {
            response.response.rect.height()
        };
        // Written back every frame, so the arrangement that is saved is the one
        // on screen rather than the one it started as.
        if (measured - size).abs() > 0.5 {
            self.preferences.workspace.resize(slot, measured);
        }
    }

    /// The centre, which is whatever every other slot left behind.
    fn draw_main(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(self.slot_frame(Slot::Main))
            .show(ui, |ui| {
                self.draw_group(ui, Slot::Main);
            });
    }

    /// A slot's frame: viewports sit on ink, everything else on panel ground.
    fn slot_frame(&self, slot: Slot) -> egui::Frame {
        let showing_viewport = self
            .preferences
            .workspace
            .group(slot)
            .and_then(crate::dock::Group::selected)
            .is_some_and(DockPanel::is_viewport);
        if showing_viewport {
            panel::viewport_frame()
        } else {
            panel::frame()
        }
    }

    /// A group: its tab strip, and the contents of whichever tab is showing.
    fn draw_group(&mut self, ui: &mut egui::Ui, slot: Slot) {
        let Some(group) = self.preferences.workspace.group(slot) else {
            return;
        };
        let panels = group.panels.clone();
        let active = group.active_index();
        let (strip, tabs) = self.dock_strip(ui, slot, &panels, active);
        let body = ui.available_rect_before_wrap();
        self.dock.zones.push(Zone {
            slot,
            strip,
            body,
            tabs,
        });
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
    /// Drawn after every slot because the highlight belongs over the panels
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
                    .can_place(drag.panel, target.slot)
            });
        self.paint_drag(context, &drag);
        if context.input(|input| input.pointer.any_released()) {
            if let Some(DropTarget { slot, index }) = drag.target {
                self.preferences.workspace.place(drag.panel, slot, index);
            }
            self.dock.drag = None;
        } else {
            self.dock.drag = Some(drag);
        }
        context.request_repaint();
    }

    /// Which slot and tab position a pointer is over.
    ///
    /// A tab strip first, because dropping between two tabs is the precise
    /// gesture and must win wherever the two overlap; then a body, which means
    /// "join this group at the end"; then the window's edge, which is the only
    /// way to reach a slot that is currently empty.
    fn drop_target(&self, window: Rect, pointer: egui::Pos2) -> Option<DropTarget> {
        for zone in &self.dock.zones {
            if zone.strip.contains(pointer) {
                return Some(DropTarget {
                    slot: zone.slot,
                    index: crate::dock::tab_index(&zone.tabs, pointer),
                });
            }
        }
        for zone in &self.dock.zones {
            if zone.body.contains(pointer) {
                return Some(DropTarget {
                    slot: zone.slot,
                    index: usize::MAX,
                });
            }
        }
        crate::dock::edge_slot(window, pointer).map(|slot| DropTarget {
            slot,
            index: usize::MAX,
        })
    }
}
