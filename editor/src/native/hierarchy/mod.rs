//! The scene hierarchy panel.

pub(super) mod row;
pub(super) mod rows;

use std::collections::BTreeSet;

use eframe::egui;
use sindri_core::EntityId;

use self::row::{HierarchyDrag, entity_icon, entity_is_bare, entity_name, hierarchy_drop_target};
use self::rows::{hierarchy_group, hierarchy_preference_key, visible_hierarchy_rows};
use crate::preferences::Layout as WorkspaceLayout;
use crate::space::EntitySpace;
use crate::ui::icons;
use crate::ui::theme::color;
use crate::ui::widgets::{
    button::{self, Intent},
    panel,
    tree::{self, Children, RowStyle},
};

use super::EditorApp;
use super::editing::CreateGameObject;
use super::runtime::PLAYING_TIP;

impl EditorApp {
    pub(super) fn hierarchy_panel(&mut self, ui: &mut egui::Ui) {
        // Distinct ids per side deliberately: switching layouts should not
        // carry a width chosen for a different arrangement.
        let panel_side = match self.preferences.layout {
            WorkspaceLayout::TwoByThree => egui::Panel::right("hierarchy-column"),
            WorkspaceLayout::Wide => egui::Panel::left("hierarchy-dock"),
        };
        panel_side
            .default_size(248.0)
            .min_size(200.0)
            .max_size(340.0)
            .resizable(true)
            .frame(panel::frame())
            .show(ui, |ui| {
                let (create, deleted) = self.hierarchy_header(ui);
                panel::body(ui, |ui| {
                    panel::search(ui, &mut self.search, "Filter entities");
                });
                self.hierarchy_contents(ui);
                if let Some(create) = create {
                    self.create_game_object(create);
                }
                if let Some(entity) = deleted {
                    self.delete_entity(entity);
                }
            });
    }

    /// The panel's name, and the two things it can do to a scene.
    ///
    /// The actions live in the header rather than on a strip of their own: a
    /// create menu and a delete button are one row's worth of controls, and a
    /// second strip under the title spent eight vertical pixels saying so.
    fn hierarchy_header(&self, ui: &mut egui::Ui) -> (Option<CreateGameObject>, Option<EntityId>) {
        let mut create = None;
        let mut deleted = None;
        // Spawning and despawning are world writes, and a running scene is not
        // the document: Stop puts back the world as it was when Play was
        // pressed, so anything made here while playing would vanish without
        // being mentioned.
        let authoring = self.authoring_enabled();
        panel::header(ui, icons::HIERARCHY, "Hierarchy", |ui| {
            ui.add_enabled_ui(authoring, |ui| {
                // Offered only with something selected, because "delete" with
                // nothing chosen has no answer and a disabled button is a
                // question nobody asked.
                if let Some(entity) = self.selection
                    && button::row_icon(
                        ui,
                        icons::REMOVE,
                        Intent::Danger,
                        "Delete the selected entity",
                    )
                    .clicked()
                {
                    deleted = Some(entity);
                }
                ui.menu_button(
                    icons::ADD
                        .outlined()
                        .rich_text()
                        .size(15.0)
                        .color(color::TEXT_MUTED),
                    |ui| {
                        ui.set_min_width(200.0);
                        // Which space a new object is in is a choice made here
                        // rather than a component hunted for afterwards,
                        // because it is the first thing an author knows about
                        // the thing they are making.
                        if ui.button("Create Empty").clicked() {
                            create = Some(CreateGameObject::Empty { parent: None });
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.selection.is_some(),
                                egui::Button::new("Create Child").shortcut_text("under selection"),
                            )
                            .clicked()
                        {
                            create = self.selection.map(|parent| CreateGameObject::Empty {
                                parent: Some(parent),
                            });
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Create UI Image").clicked() {
                            create = Some(CreateGameObject::UiImage);
                            ui.close();
                        }
                    },
                )
                .response
                .on_hover_text(if authoring {
                    "Create GameObject"
                } else {
                    PLAYING_TIP
                });
            });
        });
        (create, deleted)
    }

    /// Which entities the hierarchy is currently folded closed.
    fn collapsed_entities(&self) -> BTreeSet<EntityId> {
        self.world
            .entities()
            .filter_map(|(entity, _)| {
                hierarchy_preference_key(self.file.path(), &self.world, entity)
                    .filter(|key| self.preferences.collapsed_hierarchy.contains(key))
                    .map(|_| entity)
            })
            .collect()
    }

    fn hierarchy_contents(&mut self, ui: &mut egui::Ui) {
        let mut reparenting = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(2.0);
                let needle = self.search.trim().to_lowercase();
                let collapsed = self.collapsed_entities();
                let mut clicked: Option<Option<EntityId>> = None;
                let mut toggled = None;
                let mut listed = 0_usize;
                // Two groups, because a scene holds two kinds of thing: what is
                // in the world and what is drawn on top of it. Which group an
                // entity is listed under is read from what it carries, so a
                // drop on either header means the same thing — move to the top
                // level — and the entity lands where its components say.
                for (space, label, icon) in [
                    (EntitySpace::World, "World", icons::WORLD),
                    (EntitySpace::Ui, "UI", icons::UI_ELEMENT),
                ] {
                    ui.add_space(if space == EntitySpace::Ui { 8.0 } else { 0.0 });
                    let root = hierarchy_group(ui, label, icon);
                    if let Some(entity) = hierarchy_drop_target(ui, &root, &self.world, None) {
                        reparenting = Some((entity, None));
                    }
                    for (entity, depth) in
                        visible_hierarchy_rows(&self.world, &collapsed, &needle, space)
                    {
                        let Some(data) = self.world.get(entity) else {
                            continue;
                        };
                        listed += 1;
                        let name = entity_name(data);
                        let row = tree::row(
                            ui,
                            entity_icon(data),
                            &name,
                            RowStyle {
                                selected: self.selection == Some(entity),
                                depth: depth + 1,
                                children: Children::of(
                                    data.children.len(),
                                    collapsed.contains(&entity) && needle.is_empty(),
                                ),
                                dimmed: entity_is_bare(data),
                            },
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
                }
                if listed == 0 {
                    ui.add_space(6.0);
                    panel::note(
                        ui,
                        if needle.is_empty() {
                            "This scene has no entities yet. Use + to create one."
                        } else {
                            "No entity here matches that filter."
                        },
                    );
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
