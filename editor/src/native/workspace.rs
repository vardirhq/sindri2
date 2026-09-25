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

mod floating;

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
    /// The rectangle overlays anchor to: the world as actually drawn, not the
    /// panel it was drawn in.
    ///
    /// The difference is the centre's own furniture. A group wears a tab strip,
    /// and the Scene view wears a toolbar under it; anchoring to the panel puts
    /// the first overlay on top of both, so the tabs that switch Scene and Game
    /// end up underneath the hierarchy and cannot be clicked. Anchoring to the
    /// viewport instead means an overlay covers the world — which is what it is
    /// for — and never the controls.
    ///
    /// Nothing until the centre has been drawn once. `Rect::ZERO` rather than
    /// an `Option`, because the only frame it is wrong on is the first, where
    /// an overlay drawn at the origin for one frame is invisible and an
    /// `Option` unwrapped at four call sites is not.
    pub(super) canvas: Rect,
    /// Whether the group being drawn is the centre, so the viewport inside it
    /// knows whether its rectangle is the one overlays should anchor to.
    pub(super) drawing_main: bool,
    /// Where each overlay was last drawn, which a dragged one snaps to.
    pub(super) overlays: Vec<(Place, Rect)>,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            drag: None,
            zones: Vec::new(),
            canvas: Rect::ZERO,
            drawing_main: false,
            overlays: Vec::new(),
        }
    }
}

/// How much of the window one dock may take.
///
/// A fraction rather than a number of points, so the limit means the same thing
/// on a laptop and on a monitor. It exists only to stop a dock swallowing the
/// window whole; anything short of that is the user's business.
const MAX_SHARE: f32 = 0.75;

/// What every dock together leaves the scene, at the least.
///
/// Each dock was capped only against the window, so three columns could each
/// take their share and leave the scene a few hundred points: Wide at 1366
/// wide gave it about 450. Docks are drawn one after another, and each is now
/// capped by what the ones before it left, less this.
const CENTRE_MIN_WIDTH: f32 = 420.0;
const CENTRE_MIN_HEIGHT: f32 = 240.0;

/// How much of the view's bottom-right corner a top-right overlay leaves for
/// the axis gizmo.
const AXES_CLEARANCE: f32 = 96.0;

/// The gap between an overlay and the edges of the scene it floats over, and
/// between two overlays stacked in the same corner.
const OVERLAY_GUTTER: f32 = 12.0;

/// The widest the floating tool island may be.
///
/// A cap rather than a size: the island shrinks to the controls in it, and this
/// stops it growing to whatever `available_width` reports. Inside an `Area`
/// that is auto-sizing, that number is enormous — which is what first made the
/// island a full-width band again, wearing rounded ends and hiding half its own
/// controls behind the hierarchy.
const ISLAND_WIDTH: f32 = 620.0;
/// The Game view's island: a device preset, its size and the picker.
const GAME_ISLAND_WIDTH: f32 = 400.0;

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
        self.paint_applying(ui.ctx());
    }

    /// One docked slot: an `egui::Panel` on the side the slot names.
    fn draw_dock(&mut self, ui: &mut egui::Ui, slot: Slot, window: Rect) {
        let place = Place::Dock(slot);
        let Some(group) = self.preferences.workspace.group(place) else {
            return;
        };
        let size = group.size;
        let column = slot.is_column();
        let left = ui.available_rect_before_wrap();
        let max = if column {
            (window.width() * MAX_SHARE).min(left.width() - CENTRE_MIN_WIDTH)
        } else {
            (window.height() * MAX_SHARE).min(left.height() - CENTRE_MIN_HEIGHT)
        }
        .max(place.min_size());
        // Only a drag on an edge says what size the user wants. Otherwise egui's
        // remembered size is put back to the preference each frame, so a dock
        // squeezed by a small window grows back with it rather than keeping
        // the squeezed size for good.
        let dragging = ui.ctx().input(|input| input.pointer.primary_down());
        if !dragging {
            restore_size(
                ui.ctx(),
                slot.id(),
                column,
                size.clamp(place.min_size(), max),
            );
        }
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
        // Written back while an edge is dragged, so the arrangement that is saved
        // is the one the user chose, not whatever a small window squeezed it to.
        if dragging && (measured - size).abs() > 0.5 {
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
        // A fallback for the frame where the centre holds no viewport at all:
        // a console in the middle has no rendered rectangle to anchor to, and
        // the panel it was drawn in is the next best answer.
        if !self
            .preferences
            .workspace
            .group(Place::MAIN)
            .and_then(crate::dock::Group::selected)
            .is_some_and(DockPanel::is_viewport)
        {
            self.dock.canvas = response.response.rect;
        }
    }

    /// The overlays anchored to one corner of the scene, stacked in tab order.
    ///
    /// Stacked rather than placed, so two overlays sharing a corner cannot end
    /// up on top of each other. The stack grows inwards from the corner: down
    /// from the top ones, up from the bottom ones.
    /// The region overlays arrange themselves in.
    ///
    /// The canvas itself when the window's furniture is docked, because the
    /// bars have already taken their rows out of it. When the furniture floats,
    /// the canvas runs edge to edge under the bars — that is the point of it —
    /// so the corners overlays anchor to have to be inset past them by hand, or
    /// the hierarchy sits under the menus.
    /// The part of `rect` no floating bar covers, for anything drawn in a view
    /// that has to be read.
    pub(super) fn unobscured(&self, rect: Rect) -> Rect {
        rect.intersect(self.overlay_field())
    }

    fn overlay_field(&self) -> Rect {
        let canvas = self.dock.canvas;
        if self.preferences.workspace.chrome().floats() {
            Rect::from_min_max(
                egui::pos2(canvas.left(), canvas.top() + metric::TOP_BAR_HEIGHT),
                egui::pos2(canvas.right(), canvas.bottom() - metric::STATUS_HEIGHT),
            )
        } else {
            canvas
        }
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
        // The centre wears no strip of its own when the furniture floats: its
        // tabs are drawn in the top bar, so a full-width row here would be the
        // same tabs twice and another row of chrome ending the canvas.
        let in_the_bar = place == Place::MAIN && self.preferences.workspace.chrome().floats();
        let (strip, tabs) = if in_the_bar {
            (Rect::NOTHING, Vec::new())
        } else {
            self.dock_strip(ui, place, &panels, active)
        };
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
            self.dock.drawing_main = place == Place::MAIN;
            self.draw_panel(ui, panel);
            self.dock.drawing_main = false;
        }
    }

    /// One panel's contents, with nothing around them.
    ///
    /// Every arm is a call into the module that owns that region: what a panel
    /// contains is that module's business, and where it is drawn is this one's.
    fn draw_panel(&mut self, ui: &mut egui::Ui, panel: DockPanel) {
        match panel {
            DockPanel::Scene => {
                if self.preferences.workspace.chrome().floats() {
                    // The world first, into the whole of the space, and the
                    // tools over it afterwards. Drawn the other way round the
                    // toolbar claims a row and the scene starts below it, which
                    // is a docked editor however the panels are placed.
                    self.render_view(ui, WorkspaceTab::Scene);
                    self.tool_island(ui);
                } else {
                    self.scene_tools(ui, true);
                    self.render_view(ui, WorkspaceTab::Scene);
                }
            }
            DockPanel::Game => {
                if self.preferences.workspace.chrome().floats() {
                    // As the Scene view's tools: a strip at the top would be
                    // under the floating title bar, out of reach.
                    self.render_view(ui, WorkspaceTab::Game);
                    self.game_island(ui);
                } else {
                    self.game_tools(ui);
                    self.render_view(ui, WorkspaceTab::Game);
                }
            }
            DockPanel::Hierarchy => self.hierarchy_body(ui),
            DockPanel::Inspector => self.inspector_body(ui),
            DockPanel::Project => self.project_body(ui),
            DockPanel::Console => self.console_body(ui),
            DockPanel::History => self.history_body(ui),
            DockPanel::Assistant => self.assistant_body(ui),
        }
    }

    /// Points the centre's drop zone at the tabs drawn in the title bar.
    ///
    /// The zone was pushed with nothing in it while the centre was drawn,
    /// because its tabs had not been laid out yet — the bar draws before the
    /// workspace does on a docked arrangement and after it on a floating one.
    /// Filling it in here is what keeps "drop a panel on the Scene tab" working
    /// in both.
    pub(super) fn retarget_centre_zone(&mut self, strip: Rect, tabs: Vec<Rect>) {
        if let Some(zone) = self
            .dock
            .zones
            .iter_mut()
            .find(|zone| zone.place == Place::MAIN)
        {
            zone.strip = strip;
            zone.tabs = tabs;
        }
    }

    /// The scene tools, as an island floating over the world they act on.
    ///
    /// Anchored to the top of the canvas rather than following anything: these
    /// are the verbs that are always available, and a control that moves is a
    /// control that has to be looked for.
    fn tool_island(&mut self, ui: &mut egui::Ui) {
        self.island(
            ui,
            "scene-tool-island",
            ISLAND_WIDTH,
            Self::scene_tools_island,
        );
    }

    /// The Game view's tools, floating as the Scene view's do.
    fn game_island(&mut self, ui: &mut egui::Ui) {
        self.island(ui, "game-tool-island", GAME_ISLAND_WIDTH, |app, ui| {
            app.game_tool_row(ui, false);
        });
    }

    fn island(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        width: f32,
        contents: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let field = self.overlay_field();
        // Given an explicit rectangle rather than left to size itself. An
        // auto-sizing `Area` measured zero here and the island rendered
        // nothing at all — the same pattern the docked overlays already use
        // works, and the centring needs a known width anyway.
        let width = width.min(field.width() - OVERLAY_GUTTER * 2.0);
        let rect = Rect::from_min_size(
            egui::pos2(field.center().x - width / 2.0, field.top() + OVERLAY_GUTTER),
            egui::vec2(width, metric::TOOLBAR_HEIGHT),
        );
        egui::Area::new(egui::Id::new(id))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .interactable(true)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                panel::overlay_frame().show(ui, |ui| {
                    ui.set_clip_rect(rect.shrink(1.0));
                    contents(self, ui);
                });
            });
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

/// Puts egui's remembered size for a dock back to `size`.
///
/// egui keeps the panel's last rectangle and starts each frame from it, so once
/// a window squeezed a dock, that squeezed width was what every later frame
/// began with.
fn restore_size(context: &egui::Context, id: &'static str, column: bool, size: f32) {
    let id = egui::Id::new(id);
    let Some(mut state) = egui::containers::panel::PanelState::load(context, id) else {
        return;
    };
    let current = if column {
        state.outer_rect.width()
    } else {
        state.outer_rect.height()
    };
    if (current - size).abs() <= 0.5 {
        return;
    }
    let mut extent = state.outer_rect.size();
    if column {
        extent.x = size;
    } else {
        extent.y = size;
    }
    state.outer_rect = Rect::from_min_size(state.outer_rect.min, extent);
    context.data_mut(|data| data.insert_persisted(id, state));
}

#[cfg(test)]
mod tests {
    use eframe::egui::{self, Rect, containers::panel::PanelState, pos2, vec2};

    use super::restore_size;

    /// A dock egui remembers as squeezed is put back to the size the user
    /// chose, so it grows back with the window instead of staying small.
    #[test]
    fn a_squeezed_dock_is_restored_to_its_preference() {
        let context = egui::Context::default();
        let id = egui::Id::new("dock-left");
        context.data_mut(|data| {
            data.insert_persisted(
                id,
                PanelState {
                    outer_rect: Rect::from_min_size(pos2(0.0, 40.0), vec2(150.0, 600.0)),
                },
            );
        });
        restore_size(&context, "dock-left", true, 260.0);
        let state = PanelState::load(&context, id).expect("still remembered");
        assert!((state.size().x - 260.0).abs() < f32::EPSILON);
        assert!(
            (state.size().y - 600.0).abs() < f32::EPSILON,
            "only the axis the dock resizes along changes"
        );
    }
}
