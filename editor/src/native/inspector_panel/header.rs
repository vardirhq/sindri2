//! What every entity has: its name, where it lives, and its transform.

use eframe::egui::{self, Align, FontId, Layout, RichText, Stroke};
use egui_material_icons::MaterialIcon;
use glam::{EulerRot, Quat};
use sindri_core::{EntityId, Transform3D};

use crate::space::EntitySpace;
use crate::ui::icons;
use crate::ui::theme::{color, metric, radius, radius_tight, text};
use crate::ui::widgets::{property, section, vector};

use super::super::hierarchy::rows::ROOT_LABEL;
use super::draft::EntityDraft;

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
    property::Property::new("Parent")
        .tip("Which entity this one moves with")
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt(("parent", entity.index()))
                .selected_text(
                    RichText::new(current)
                        .size(text::LABEL)
                        .color(color::TEXT_MUTED),
                )
                .width(property::picker_width(ui))
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

/// The card at the top of the inspector: what this entity is called, and what
/// kind of thing it is.
///
/// Given a ground of its own so the name reads as the subject of the panel
/// rather than as the first of thirty rows. The rows below it are properties of
/// the thing this card names.
pub(super) fn inspector_identity(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    space: Option<EntitySpace>,
    draft: &mut EntityDraft,
) {
    egui::Frame::new()
        .fill(color::RAISED)
        .stroke(Stroke::new(1.0, color::LINE_SOFT))
        .corner_radius(radius())
        .inner_margin(egui::Margin::symmetric(8, 7))
        .outer_margin(egui::Margin::symmetric(metric::GUTTER_EDGE, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                ui.label(icon.outlined().rich_text().size(19.0).color(color::FORGE));
                ui.add_sized(
                    [ui.available_width(), metric::CONTROL_HEIGHT + 4.0],
                    egui::TextEdit::singleline(&mut draft.name)
                        .font(FontId::proportional(text::BODY + 1.0))
                        .hint_text("Unnamed entity"),
                );
            });
            ui.add_space(5.0);
            space_badge(ui, space);
        });
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
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(if known { color::EMBER } else { color::WELL })
            .stroke(Stroke::new(
                1.0,
                if known {
                    color::FORGE_DIM
                } else {
                    color::LINE_SOFT
                },
            ))
            .corner_radius(radius_tight())
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(RichText::new(label).size(text::NOTE).color(if known {
                    color::FORGE
                } else {
                    color::TEXT_FAINT
                }));
            })
            .response
            .on_hover_text(tip);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new("Entity")
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            );
        });
    });
}

pub(super) fn transform_3d_section(ui: &mut egui::Ui, transform: &mut Transform3D) {
    let open = section::component(
        ui,
        egui::Id::new("inspector-transform"),
        icons::TRANSFORM,
        "Transform",
        |ui| {
            // The lock is the one thing about a transform that is not a number,
            // so it is stated in the header rather than buried under nine drags.
            if transform.z_locked {
                crate::ui::widgets::toolbar::chip(ui, "Z locked", color::FORGE);
            }
        },
    );
    if !open {
        return;
    }
    ui.add_space(4.0);
    // The Z drag is taken away rather than left to fail: the command layer
    // would refuse the edit anyway, and a control that cannot do what it looks
    // like it does is the thing this editor is trying not to grow.
    vector::row(
        ui,
        "Position",
        &mut transform.position,
        &[false, false, transform.z_locked],
        0.05,
    );
    let rotation = Quat::from_array(transform.rotation);
    let rotation = if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    let mut degrees = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
    if vector::row(ui, "Rotation", &mut degrees, &[false; 3], 0.25) {
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            degrees[0].to_radians(),
            degrees[1].to_radians(),
            degrees[2].to_radians(),
        )
        .to_array();
    }
    vector::row(ui, "Scale", &mut transform.scale, &[false; 3], 0.01);
    property::toggle(ui, "Z lock", &mut transform.z_locked, "Locked", "Free");
    ui.add_space(4.0);
}
