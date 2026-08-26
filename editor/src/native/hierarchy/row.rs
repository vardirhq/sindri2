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
