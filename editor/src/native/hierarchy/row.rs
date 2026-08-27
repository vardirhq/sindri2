//! Drawing one hierarchy row, and reading what the pointer did to it.
//!
//! The drawing itself is `ui::widgets::tree`, which the project browser's rows
//! also sit on. What is here is what a *scene* row is: which icon an entity
//! earns from what it carries, what it is called when it has no name, and which
//! drops the world will accept.

use eframe::egui::{self, Response, Stroke, StrokeKind};
use egui_material_icons::MaterialIcon;
use sindri_core::{EntityData, EntityId, World};

use crate::ui::icons;
use crate::ui::theme::{color, metric};
use crate::ui::widgets::{
    menu,
    tree::{self, Children, Rename, RowStyle},
};

use crate::selection::Pick;

use super::RowAction;

/// How one row is drawn, and whether the scene can be edited behind it.
pub(super) struct RowLook {
    pub(super) depth: usize,
    pub(super) selected: bool,
    /// Whether this row has anywhere to go up, and anywhere to go down.
    ///
    /// Read here rather than in the menu, because the menu is built while the
    /// world is already borrowed to draw the row.
    pub(super) can_move: (bool, bool),
    /// How many entities are selected in all.
    ///
    /// The row's menu needs it: right-clicking one of five selected rows and
    /// choosing Delete should delete the five, and the entry has to say so
    /// before it is chosen rather than after.
    pub(super) selected_count: usize,
    pub(super) collapsed: bool,
    /// Whether the verbs that write to the world are offered at all. A running
    /// scene is not the document, and Stop puts back what Play started with.
    pub(super) authoring: bool,
}

/// What one row reported this frame.
///
/// Gathered as a value rather than acted on where it is read, because every
/// one of these writes to the world the listing is being drawn from.
#[derive(Default)]
pub(super) struct RowReport {
    pub(super) asked: Option<RowAction>,
    pub(super) reparent: Option<(EntityId, Option<EntityId>)>,
    /// Whether the fold triangle was pressed.
    pub(super) toggled: bool,
    /// How the row was pressed, if it was: a plain click, a Ctrl-click, or a
    /// Shift-click, which mean three different things to a selection.
    pub(super) clicked: Option<Pick>,
}

/// One entity as a row: what it looks like, what a drag does to it, and every
/// verb that acts on it alone.
pub(super) fn entity_row(
    ui: &mut egui::Ui,
    world: &World,
    entity: EntityId,
    look: &RowLook,
    draft: Option<&mut String>,
) -> RowReport {
    let mut report = RowReport::default();
    let Some(data) = world.get(entity) else {
        return report;
    };
    let name = entity_name(data);
    let renaming = draft.is_some();
    let row = tree::row_named(
        ui,
        entity_icon(data),
        &name,
        RowStyle {
            selected: look.selected,
            depth: look.depth + 1,
            children: Children::of(data.children.len(), look.collapsed),
            dimmed: entity_is_bare(data),
        },
        draft,
    );
    match row.rename {
        Some(Rename::Committed) => report.asked = Some(RowAction::CommitRename),
        Some(Rename::Cancelled) => report.asked = Some(RowAction::CancelRename),
        None => {}
    }
    // The row's own menu, which is where every verb that acts on one entity
    // lives. Without it there was nowhere to put duplicate, rename, or delete,
    // and so none of the three existed.
    // A menu on a row inside the selection is a menu about the selection.
    // Anywhere else it is about the one row, and right-clicking outside a
    // selection does not silently act on entities somewhere else in the tree.
    let group = if look.selected {
        look.selected_count.max(1)
    } else {
        1
    };
    row_menu(
        &row.select,
        &name,
        entity,
        Group {
            size: group,
            authoring: look.authoring,
            can_move: look.can_move,
        },
        &mut report.asked,
    );
    // A rename is a text field, and dragging a text field out of the hierarchy
    // is not a reparent.
    if !renaming {
        row.select.dnd_set_drag_payload(HierarchyDrag(entity));
    }
    if row.select.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if let Some(dragged) = hierarchy_drop_target(ui, &row.drop, world, Some(entity)) {
        report.reparent = Some((dragged, Some(entity)));
    }
    if row.toggle.is_some_and(|response| response.clicked()) {
        report.toggled = true;
    } else if row.select.double_clicked() && look.authoring {
        // A double click renames, which is where every other tree in every
        // other tool puts it.
        report.asked = Some(RowAction::BeginRename(entity));
    } else if row.select.clicked() && !renaming {
        report.clicked = Some(picked(ui));
    }
    report
}

/// How many entities a row's menu is about, and whether it may write.
#[derive(Clone, Copy)]
struct Group {
    size: usize,
    authoring: bool,
    can_move: (bool, bool),
}

/// The menu a right-click opens on a row.
fn row_menu(
    response: &Response,
    name: &str,
    entity: EntityId,
    group: Group,
    asked: &mut Option<RowAction>,
) {
    let many = group.size > 1;
    let subject = if many {
        format!("{} entities", group.size)
    } else {
        name.to_owned()
    };
    menu::on_right_click(response, |ui| {
        menu::subject(ui, &subject);
        ui.add_enabled_ui(group.authoring, |ui| {
            // Renaming stays about the one row even inside a selection: five
            // rows cannot become one field, and giving five entities the same
            // name is not what anyone meant.
            if menu::item_with_key(ui, "Rename", "F2").clicked() {
                *asked = Some(RowAction::BeginRename(entity));
                ui.close();
            }
            if menu::item_with_key(ui, "Duplicate", "Ctrl+D").clicked() {
                *asked = Some(if many {
                    RowAction::DuplicateSelection
                } else {
                    RowAction::Duplicate(entity)
                });
                ui.close();
            }
            if !many && menu::item(ui, "Create child").clicked() {
                *asked = Some(RowAction::CreateChild(entity));
                ui.close();
            }
        });
        ui.separator();
        // Greyed out at the ends of a list rather than offered and refused: a
        // control that does nothing is how an interface teaches people to stop
        // trusting it. Order is the one verb here that stays about the row
        // even inside a selection, because moving five rows one place at once
        // has no single answer.
        ui.add_enabled_ui(group.authoring, |ui| {
            let (up, down) = group.can_move;
            if ui
                .add_enabled(up, menu::entry("Move up", "Alt+Up"))
                .clicked()
            {
                *asked = Some(RowAction::MoveBy(entity, -1));
                ui.close();
            }
            if ui
                .add_enabled(down, menu::entry("Move down", "Alt+Down"))
                .clicked()
            {
                *asked = Some(RowAction::MoveBy(entity, 1));
                ui.close();
            }
        });
        ui.separator();
        if menu::item_with_key(ui, "Frame in the Scene view", "F").clicked() {
            *asked = Some(if many {
                RowAction::FocusSelection
            } else {
                RowAction::Focus(entity)
            });
            ui.close();
        }
        ui.separator();
        ui.add_enabled_ui(group.authoring, |ui| {
            if menu::danger(ui, "Delete", "Del").clicked() {
                *asked = Some(if many {
                    RowAction::DeleteSelection
                } else {
                    RowAction::Delete(entity)
                });
                ui.close();
            }
        });
    });
}

/// The hierarchy owns its payload type so future drag-and-drop tools cannot be
/// mistaken for an entity move merely because they also carry an `EntityId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HierarchyDrag(pub(crate) EntityId);

/// Draws feedback for a hierarchy drop and returns a legal released payload.
pub(crate) fn hierarchy_drop_target(
    ui: &egui::Ui,
    response: &Response,
    world: &World,
    parent: Option<EntityId>,
) -> Option<EntityId> {
    let dragged = response.dnd_hover_payload::<HierarchyDrag>()?;
    let allowed = hierarchy_drop_allowed(world, dragged.0, parent);
    let tint = if allowed { color::FORGE } else { color::DANGER };
    // A filled wash as well as the outline: an outline alone on a row that is
    // already banded reads as the row being selected rather than as a target.
    ui.painter()
        .rect_filled(response.rect, metric::RADIUS, tint.gamma_multiply(0.14));
    ui.painter().rect_stroke(
        response.rect,
        metric::RADIUS,
        Stroke::new(1.5, tint),
        StrokeKind::Inside,
    );
    ui.ctx().set_cursor_icon(if allowed {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::NotAllowed
    });
    if allowed {
        response
            .dnd_release_payload::<HierarchyDrag>()
            .map(|dragged| dragged.0)
    } else {
        None
    }
}

pub(crate) fn hierarchy_drop_allowed(
    world: &World,
    entity: EntityId,
    parent: Option<EntityId>,
) -> bool {
    world.get(entity).is_some_and(|data| data.parent != parent)
        && world.check_set_parent(entity, parent).is_ok()
}

pub(crate) fn entity_name(entity: &EntityData) -> String {
    entity.name.clone().unwrap_or_else(|| {
        entity
            .source_id
            .as_ref()
            .map_or_else(|| "Entity".to_owned(), |id| humanize(id.as_str()))
    })
}

/// What an entity looks like in a list, from the first thing it carries that
/// says what it is.
pub(crate) fn entity_icon(entity: &EntityData) -> MaterialIcon {
    icons::for_entity(|type_name| entity.components.contains_key(type_name))
}

/// Whether this entity carries nothing but its place in the world.
///
/// An entity with a transform and no components is a perfectly good thing to
/// make — a pivot, a spawn point, a parent for four others — but it is also
/// what a half-finished one looks like, and the hierarchy is where the
/// difference should be visible rather than in the inspector one click later.
/// The row is drawn dimmed, not marked as wrong, because it is not wrong.
pub(crate) fn entity_is_bare(entity: &EntityData) -> bool {
    entity.components.is_empty()
}

pub(crate) fn component_label(name: &str) -> String {
    humanize(name.strip_prefix("sindri.").unwrap_or(name))
}

pub(crate) fn humanize(value: &str) -> String {
    value
        .split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            // "UI" is a word people write in capitals, and "Ui Image" over a
            // component reads as a typo rather than as a name.
            if part.eq_ignore_ascii_case("ui") {
                return "UI".to_owned();
            }
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the modifiers held during a click mean to the selection.
///
/// Shift wins over Ctrl when both are down, because a range is the more
/// specific request and answering half of each would be answering neither.
/// `command` rather than `ctrl` on the Ctrl branch, because egui reports a
/// Mac's Command key there and Ctrl-click on a Mac is a right-click.
pub(crate) fn picked(ui: &egui::Ui) -> Pick {
    ui.input(|input| {
        if input.modifiers.shift {
            Pick::Through
        } else if input.modifiers.command {
            Pick::Also
        } else {
            Pick::Only
        }
    })
}
