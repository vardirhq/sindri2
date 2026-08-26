//! Drawing one hierarchy row, and reading what the pointer did to it.

use eframe::egui::{self, Response, RichText, Sense, Stroke, StrokeKind, Vec2};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_CAMERA_ALT, ICON_DEPLOYED_CODE, ICON_IMAGE, ICON_KEYBOARD_ARROW_DOWN,
        ICON_KEYBOARD_ARROW_RIGHT, ICON_TITLE, ICON_VIEW_IN_AR, ICON_WEB_ASSET,
    },
};
use sindri_core::{EntityData, EntityId, World};

use super::super::theme::{ACCENT, ACCENT_BRIGHT, PROBLEM, TEXT, TEXT_MUTED};

/// The two independent actions a hierarchy row can report.
pub(crate) struct HierarchyRowResponse {
    pub(crate) select: Response,
    pub(crate) drop: Response,
    pub(crate) toggle: Option<Response>,
}

/// The hierarchy owns its payload type so future drag-and-drop tools cannot be
/// mistaken for an entity move merely because they also carry an `EntityId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HierarchyDrag(pub(crate) EntityId);

/// One row of the hierarchy, reporting selection separately from folding.
///
/// The response has to be the button's, not the layout's. `ui.horizontal`
/// allocates its region with `Sense::hover`, so asking that value whether it was
/// clicked answers no forever — which is what it did from the first editor
/// commit until this was found by driving the editor rather than reading it. The
/// whole of selection, and therefore every edit the editor can make, hung on
/// this one word.
///
/// The row's rect is re-sensed as well, so the icon and the padding beside the
/// name select too. A row that answers only on its text is the same complaint in
/// miniature.
pub(crate) fn hierarchy_row(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    name: &str,
    selected: bool,
    depth: usize,
    has_children: bool,
    expanded: bool,
) -> HierarchyRowResponse {
    let width = ui.available_width();
    let builder = egui::UiBuilder::new().sense(Sense::click_and_drag());
    let row = ui.scope_builder(builder, |ui| {
        ui.set_min_width(width);
        ui.horizontal(|ui| {
            ui.add_space(9.0 + hierarchy_indent(depth, 14.0));
            let toggle = if has_children {
                Some(
                    ui.add(
                        egui::Button::new(
                            if expanded {
                                ICON_KEYBOARD_ARROW_DOWN
                            } else {
                                ICON_KEYBOARD_ARROW_RIGHT
                            }
                            .outlined()
                            .rich_text()
                            .size(15.0)
                            .color(TEXT_MUTED),
                        )
                        .frame(false)
                        .min_size(Vec2::new(16.0, 18.0)),
                    )
                    .on_hover_text(if expanded {
                        "Collapse children"
                    } else {
                        "Expand children"
                    }),
                )
            } else {
                ui.add_space(16.0);
                None
            };
            // The icon senses clicks so that it does not swallow them: a
            // widget inside the scope takes precedence over the scope's own
            // sense, so a hover-only label would be a dead patch in the middle
            // of the row.
            let icon = ui.add(
                egui::Label::new(icon.outlined().rich_text().size(15.0).color(if selected {
                    ACCENT_BRIGHT
                } else {
                    TEXT_MUTED
                }))
                .sense(Sense::click_and_drag()),
            );
            let label = ui.add(
                egui::Button::new(RichText::new(name).size(12.0).color(if selected {
                    TEXT
                } else {
                    TEXT_MUTED
                }))
                .selected(selected)
                .sense(Sense::click_and_drag())
                .frame(false),
            );
            (icon | label, toggle)
        })
        .inner
    });
    // A scope's sense sits below the widgets inside it, so the name still
    // answers for itself and the rest of the row answers for the scope.
    let select = row.response | row.inner.0;
    let toggle = row.inner.1;
    let drop = toggle
        .clone()
        .map_or_else(|| select.clone(), |toggle| select.clone() | toggle);
    HierarchyRowResponse {
        select,
        drop,
        toggle,
    }
}

/// Draws feedback for a hierarchy drop and returns a legal released payload.
pub(crate) fn hierarchy_drop_target(
    ui: &egui::Ui,
    response: &Response,
    world: &World,
    parent: Option<EntityId>,
) -> Option<EntityId> {
    let dragged = response.dnd_hover_payload::<HierarchyDrag>()?;
    let allowed = hierarchy_drop_allowed(world, dragged.0, parent);
    let colour = if allowed { ACCENT } else { PROBLEM };
    ui.painter().rect_stroke(
        response.rect,
        2.0,
        Stroke::new(1.5, colour),
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
///
/// The UI family gets icons of its own, because "this is on the screen rather
/// than in the world" is the most useful thing a row can say about an entity at
/// a glance — it is the difference between two entities that otherwise read
/// identically.
pub(crate) fn entity_icon(entity: &EntityData) -> MaterialIcon {
    if entity.components.contains_key("sindri.camera") {
        ICON_CAMERA_ALT
    } else if entity.components.contains_key("sindri.mesh") {
        ICON_VIEW_IN_AR
    } else if entity.components.contains_key("sindri.sprite") {
        ICON_IMAGE
    } else if entity.components.contains_key("sindri.ui.image") {
        ICON_WEB_ASSET
    } else if entity.components.contains_key("sindri.ui.text") {
        ICON_TITLE
    } else {
        ICON_DEPLOYED_CODE
    }
}

pub(crate) fn component_label(name: &str) -> String {
    humanize(name.strip_prefix("sindri.").unwrap_or(name))
}

pub(crate) fn humanize(value: &str) -> String {
    value
        .split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn hierarchy_indent(depth: usize, step: f32) -> f32 {
    f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * step
}
