//! The scene hierarchy panel.

pub(super) mod row;
pub(super) mod rows;

use std::collections::BTreeSet;

use self::row::{HierarchyDrag, entity_icon, entity_name, hierarchy_drop_target, hierarchy_row};
use self::rows::{hierarchy_group, hierarchy_preference_key, visible_hierarchy_rows};
use eframe::egui::{self, Stroke};
use egui_material_icons::icons::{ICON_ACCOUNT_TREE, ICON_ADD, ICON_DELETE};
use sindri_core::EntityId;

use crate::preferences::Layout as WorkspaceLayout;

use super::EditorApp;
use super::editing::CreateGameObject;
use super::theme::{BORDER, PANEL_BG, panel_title, search_field};

impl EditorApp {
    pub(super) fn hierarchy_panel(&mut self, ui: &mut egui::Ui) {
        // Distinct ids per side deliberately: switching layouts should not
        // carry a width chosen for a different arrangement.
        let panel = match self.preferences.layout {
            WorkspaceLayout::TwoByThree => egui::Panel::right("hierarchy-column"),
            WorkspaceLayout::Wide => egui::Panel::left("hierarchy-dock"),
        };
        panel
            .default_size(248.0)
            .min_size(210.0)
            .max_size(340.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_title(ui, "Hierarchy");
                search_field(ui, &mut self.search, "Search");
                let (create, deleted) = self.hierarchy_toolbar(ui);
                ui.add_space(6.0);
                self.hierarchy_contents(ui);
                if let Some(create) = create {
                    self.create_entity(match create {
                        CreateGameObject::Root => None,
                        CreateGameObject::Child(parent) => Some(parent),
                    });
                }
                if let Some(entity) = deleted {
                    self.delete_entity(entity);
                }
            });
    }

    fn hierarchy_toolbar(&self, ui: &mut egui::Ui) -> (Option<CreateGameObject>, Option<EntityId>) {
        let mut create = None;
        let mut deleted = None;
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.menu_button(ICON_ADD.outlined().rich_text().size(14.0), |ui| {
                if ui.button("Create Empty").clicked() {
                    create = Some(CreateGameObject::Root);
                    ui.close();
                }
                if ui
                    .add_enabled(self.selection.is_some(), egui::Button::new("Create Child"))
                    .clicked()
                {
                    create = self.selection.map(CreateGameObject::Child);
                    ui.close();
                }
            })
            .response
            .on_hover_text("Create GameObject");
            // Offered only with something selected, because "delete" with
            // nothing chosen has no answer and a disabled button is a question
            // nobody asked.
            if let Some(entity) = self.selection
                && ui
                    .small_button(ICON_DELETE.outlined().rich_text().size(14.0))
                    .on_hover_text("Delete entity")
                    .clicked()
            {
                deleted = Some(entity);
            }
        });
        (create, deleted)
    }

    fn hierarchy_contents(&mut self, ui: &mut egui::Ui) {
        let mut reparenting = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let root = hierarchy_group(ui, "World", ICON_ACCOUNT_TREE);
                if let Some(entity) = hierarchy_drop_target(ui, &root, &self.world, None) {
                    reparenting = Some((entity, None));
                }
                let needle = self.search.trim().to_lowercase();
                let collapsed: BTreeSet<EntityId> = self
                    .world
                    .entities()
                    .filter_map(|(entity, _)| {
                        hierarchy_preference_key(self.file.path(), &self.world, entity)
                            .filter(|key| self.preferences.collapsed_hierarchy.contains(key))
                            .map(|_| entity)
                    })
                    .collect();
                let mut clicked: Option<Option<EntityId>> = None;
                let mut toggled = None;
                for (entity, depth) in visible_hierarchy_rows(&self.world, &collapsed, &needle) {
                    let Some(data) = self.world.get(entity) else {
                        continue;
                    };
                    let name = entity_name(data);
                    let row = hierarchy_row(
                        ui,
                        entity_icon(data),
                        &name,
                        self.selection == Some(entity),
                        depth + 1,
                        !data.children.is_empty(),
                        !collapsed.contains(&entity) || !needle.is_empty(),
                    );
                    row.select.dnd_set_drag_payload(HierarchyDrag(entity));
                    if row.select.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    if let Some(dragged) =
                        hierarchy_drop_target(ui, &row.drop, &self.world, Some(entity))
                    {
                        reparenting = Some((dragged, Some(entity)));
                    }
                    if row.toggle.is_some_and(|response| response.clicked()) {
                        toggled = Some(entity);
                    } else if row.select.clicked() {
                        clicked = Some(Some(entity));
                    }
                }
                if let Some(entity) = toggled
                    && let Some(key) =
                        hierarchy_preference_key(self.file.path(), &self.world, entity)
                    && !self.preferences.collapsed_hierarchy.remove(&key)
                {
                    self.preferences.collapsed_hierarchy.insert(key);
                }
                // Clicking past the last row clears the selection. Without
                // somewhere to click that means "nothing", a selection made by
                // accident can only be replaced.
                if ui
                    .allocate_response(ui.available_size(), egui::Sense::click())
                    .clicked()
                {
                    clicked = Some(None);
                }
                if let Some(entity) = clicked {
                    self.select(entity);
                }
            });
        if let Some((entity, parent)) = reparenting {
            self.reparent(entity, parent);
            self.select(Some(entity));
            if let Some(parent) = parent
                && let Some(key) = hierarchy_preference_key(self.file.path(), &self.world, parent)
            {
                self.preferences.collapsed_hierarchy.remove(&key);
            }
        }
    }
}
