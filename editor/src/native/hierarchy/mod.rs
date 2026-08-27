//! The scene hierarchy panel.

pub(super) mod row;
pub(super) mod rows;

use std::collections::BTreeSet;

use eframe::egui;
use sindri_core::EntityId;

use self::row::{RowLook, entity_name, entity_row, hierarchy_drop_target};
use self::rows::{hierarchy_group, hierarchy_preference_key, visible_hierarchy_rows};
use crate::ordering;
use crate::preferences::Layout as WorkspaceLayout;
use crate::selection::Pick;
use crate::space::EntitySpace;
use crate::ui::icons;
use crate::ui::theme::color;
use crate::ui::widgets::{
    button::{self, Intent},
    panel,
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
                if deleted {
                    self.delete_selection();
                }
            });
    }

    /// The panel's name, and the two things it can do to a scene.
    ///
    /// The actions live in the header rather than on a strip of their own: a
    /// create menu and a delete button are one row's worth of controls, and a
    /// second strip under the title spent eight vertical pixels saying so.
    fn hierarchy_header(&self, ui: &mut egui::Ui) -> (Option<CreateGameObject>, bool) {
        let mut create = None;
        let mut deleted = false;
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
                if !self.selection.is_empty()
                    && button::row_icon(
                        ui,
                        icons::REMOVE,
                        Intent::Danger,
                        if self.selection.len() == 1 {
                            "Delete the selected entity"
                        } else {
                            "Delete the selected entities"
                        },
                    )
                    .clicked()
                {
                    deleted = true;
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
                                !self.selection.is_empty(),
                                egui::Button::new("Create Child").shortcut_text("under selection"),
                            )
                            .clicked()
                        {
                            create =
                                self.selection
                                    .primary()
                                    .map(|parent| CreateGameObject::Empty {
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

    /// Everything one row needs to know about itself that is not in the world.
    ///
    /// Gathered here rather than inline, because the listing is a loop and a
    /// dozen lines of look inside it hides the loop.
    fn row_look(
        &self,
        entity: EntityId,
        depth: usize,
        collapsed: bool,
        authoring: bool,
    ) -> RowLook {
        RowLook {
            depth,
            selected: self.selection.contains(entity),
            switched_off: !self.world.is_active(entity),
            selected_count: self.selection.len(),
            can_move: {
                let beside = self.listed_beside(entity);
                (
                    ordering::can_move(&self.world, entity, -1, &beside),
                    ordering::can_move(&self.world, entity, 1, &beside),
                )
            },
            collapsed,
            authoring,
        }
    }

    fn hierarchy_contents(&mut self, ui: &mut egui::Ui) {
        let mut reparenting = None;
        let mut asked: Option<RowAction> = None;
        let authoring = self.authoring_enabled();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(2.0);
                let needle = self.search.trim().to_lowercase();
                let collapsed = self.collapsed_entities();
                let mut clicked: Option<(Option<EntityId>, Pick)> = None;
                let mut toggled = None;
                // The rows in the order they are drawn, which is the order a
                // Shift-click's range runs along.
                let mut listed: Vec<EntityId> = Vec::new();
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
                        if self.world.get(entity).is_none() {
                            continue;
                        }
                        listed.push(entity);
                        let folded = collapsed.contains(&entity) && needle.is_empty();
                        let renaming = self.renaming == Some(entity);
                        let look = self.row_look(entity, depth, folded, authoring);
                        let report = entity_row(
                            ui,
                            &self.world,
                            entity,
                            &look,
                            renaming.then_some(&mut self.rename_draft),
                        );
                        if report.asked.is_some() {
                            asked = report.asked;
                        }
                        if report.reparent.is_some() {
                            reparenting = report.reparent;
                        }
                        if report.toggled {
                            toggled = Some(entity);
                        }
                        if let Some(how) = report.clicked {
                            clicked = Some((Some(entity), how));
                        }
                    }
                }
                if listed.is_empty() {
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
                    clicked = Some((None, Pick::Only));
                }
                if let Some((entity, how)) = clicked {
                    self.pick(entity, how, &listed);
                }
            });
        if let Some(action) = asked {
            self.act_on_row(action);
        }
        if let Some((entity, parent)) = reparenting {
            self.reparent_dragged(entity, parent);
            if !self.selection.contains(entity) {
                self.select(Some(entity));
            }
            if let Some(parent) = parent
                && let Some(key) = hierarchy_preference_key(self.file.path(), &self.world, parent)
            {
                self.preferences.collapsed_hierarchy.remove(&key);
            }
        }
    }
}

/// What a row's menu, its keys, or a double click asked for.
///
/// Gathered as a value and acted on after the listing has finished drawing,
/// because every one of them borrows the world the rows are being read from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RowAction {
    BeginRename(EntityId),
    CommitRename,
    CancelRename,
    Duplicate(EntityId),
    DuplicateSelection,
    CreateChild(EntityId),
    Delete(EntityId),
    DeleteSelection,
    Focus(EntityId),
    FocusSelection,
    /// Switch this entity on or off.
    Switch(EntityId, bool),
    /// Switch everything selected on or off.
    SwitchSelection(bool),
    /// Move this entity that many places among its siblings.
    MoveBy(EntityId, isize),
}

impl EditorApp {
    /// Carries out what a row asked for.
    pub(super) fn act_on_row(&mut self, action: RowAction) {
        match action {
            RowAction::BeginRename(entity) => self.begin_rename(entity),
            RowAction::CommitRename => {
                if let Some(entity) = self.renaming.take() {
                    let draft = std::mem::take(&mut self.rename_draft);
                    self.rename_entity(entity, &draft);
                }
            }
            RowAction::CancelRename => {
                self.renaming = None;
                self.rename_draft.clear();
            }
            RowAction::Duplicate(entity) => self.duplicate_entity(entity),
            RowAction::DuplicateSelection => self.duplicate_selection(),
            RowAction::CreateChild(entity) => self.create_game_object(CreateGameObject::Empty {
                parent: Some(entity),
            }),
            RowAction::Delete(entity) => self.delete_entity(entity),
            RowAction::DeleteSelection => self.delete_selection(),
            RowAction::Focus(entity) => {
                self.select(Some(entity));
                self.focus_selection();
            }
            RowAction::FocusSelection => self.focus_selection(),
            RowAction::Switch(entity, on) => self.switch_entities(&[entity], on),
            RowAction::SwitchSelection(on) => {
                let selected = self.selection.clone();
                self.switch_entities(selected.all(), on);
            }
            RowAction::MoveBy(entity, offset) => self.move_among_siblings(entity, offset),
        }
    }

    /// Starts renaming one entity, with its current name as the draft.
    pub(super) fn begin_rename(&mut self, entity: EntityId) {
        self.select(Some(entity));
        self.rename_draft = self.world.get(entity).map(entity_name).unwrap_or_default();
        self.renaming = Some(entity);
    }
}
