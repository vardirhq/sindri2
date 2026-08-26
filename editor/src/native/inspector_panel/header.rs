//! What every entity has: its name, its parent, and its transform.

use eframe::egui::{self, FontId, RichText, Stroke};
use egui_material_icons::{MaterialIcon, icons::ICON_OPEN_WITH};
use glam::{EulerRot, Quat};
use sindri_core::{EntityId, Transform3D};

use crate::space::EntitySpace;

use super::super::hierarchy::rows::ROOT_LABEL;
use super::super::{
    ACCENT, ACCENT_SOFT, BORDER, TEXT_FAINT, TEXT_MUTED,
    theme::{PANEL_RAISED, property_toggle, section_header},
};
use super::draft::EntityDraft;
use super::rows::vector_row;

/// What the parent menu came back with.
///
/// "Move to the root" and "nothing was chosen" are both an absence of a parent
/// and are not the same answer, so they are separate variants rather than two
/// layers of `Option` the caller has to remember the order of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ParentChoice {
    /// The menu offered no change: it is closed, or the current parent was
    /// picked again.
    Unchanged,
    /// Move out to the root.
    Root,
    /// Move under this entity.
    Under(EntityId),
}

/// The parent row, reporting a choice only when it is a change.
pub(super) fn inspector_parent(
    ui: &mut egui::Ui,
    entity: EntityId,
    parent: Option<EntityId>,
    choices: &[(EntityId, String)],
) -> ParentChoice {
    let mut chosen = parent;
    let current = parent
        .and_then(|parent| {
            choices
                .iter()
                .find(|(candidate, _)| *candidate == parent)
                .map(|(_, name)| name.clone())
        })
        .unwrap_or_else(|| ROOT_LABEL.to_owned());
    ui.horizontal(|ui| {
        ui.add_space(27.0);
        ui.label(RichText::new("Parent").size(11.0).color(TEXT_FAINT));
        egui::ComboBox::from_id_salt(("parent", entity.index()))
            .selected_text(RichText::new(current).size(11.0).color(TEXT_MUTED))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, ROOT_LABEL);
                for (candidate, name) in choices {
                    ui.selectable_value(&mut chosen, Some(*candidate), name);
                }
            });
    });
    if chosen == parent {
        return ParentChoice::Unchanged;
    }
    chosen.map_or(ParentChoice::Root, ParentChoice::Under)
}

pub(super) fn inspector_identity(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    space: Option<EntitySpace>,
    draft: &mut EntityDraft,
) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(icon.outlined().rich_text().size(19.0).color(TEXT_MUTED));
        ui.add_sized(
            [ui.available_width() - 18.0, 29.0],
            egui::TextEdit::singleline(&mut draft.name).font(FontId::proportional(13.0)),
        );
    });
    space_badge(ui, space);
    // "Tag  Untagged" and "Layer  Default" used to sit under the name. Neither
    // is a thing a Sindri entity has, so they were two lines of a different
    // engine's inspector printed over this one's.
}

/// Which space this entity is in, said once at the top.
///
/// It is a readout and not a control, because nothing here decides it: the
/// components below do, and a dropdown claiming otherwise would be a switch
/// wired to nothing. An entity carrying neither family reads as undecided,
/// which is the truth and also why Add Component still offers it both.
fn space_badge(ui: &mut egui::Ui, space: Option<EntitySpace>) {
    let (label, tip) = match space {
        Some(EntitySpace::World) => (
            "In the world",
            "Placed by its transform and drawn through the world camera",
        ),
        Some(EntitySpace::Ui) => (
            "On the viewport",
            "Anchored to the screen; no camera moves it and nothing in the world hides it",
        ),
        None => (
            "Nothing drawn yet",
            "Adding a component decides which space this belongs to",
        ),
    };
    let known = space.is_some();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(28.0);
        egui::Frame::new()
            .fill(if known { ACCENT_SOFT } else { PANEL_RAISED })
            .stroke(Stroke::new(1.0, if known { ACCENT } else { BORDER }))
            .corner_radius(3.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(RichText::new(label).size(10.0).color(if known {
                    TEXT_MUTED
                } else {
                    TEXT_FAINT
                }));
            })
            .response
            .on_hover_text(tip);
    });
}

pub(super) fn transform_3d_section(ui: &mut egui::Ui, transform: &mut Transform3D) {
    section_header(ui, ICON_OPEN_WITH, "Transform");
    // The Z drag is taken away rather than left to fail: the command layer
    // would refuse the edit anyway, and a control that cannot do what it looks
    // like it does is the thing this editor is trying not to grow.
    vector_row(ui, "Position", &mut transform.position, transform.z_locked);
    let rotation = Quat::from_array(transform.rotation);
    let rotation = if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    let mut degrees = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
    if vector_row(ui, "Rotation", &mut degrees, false) {
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            degrees[0].to_radians(),
            degrees[1].to_radians(),
            degrees[2].to_radians(),
        )
        .to_array();
    }
    vector_row(ui, "Scale", &mut transform.scale, false);
    property_toggle(ui, "Z lock", &mut transform.z_locked, "Locked", "Free");
}
