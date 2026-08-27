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
use super::draft::{EntityDraft, IdentityRefusal};

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

/// Whether this entity takes part in the scene, as its own switch.
///
/// A row rather than a checkbox tucked beside the name, because it is the
/// largest single fact about an entity: off means nothing it carries is drawn,
/// stepped, scripted or picked, and neither is anything under it. Reports the
/// wanted state only when it changed, like the parent picker above it, because
/// it is a discrete choice and merging it into the draft would make one undo
/// step that both renamed and switched off.
///
/// `inherited` is a child whose parent is the one switched off. It is shown as
/// off and cannot be switched here: what would it mean to enable it? The switch
/// that governs it is on the parent, and offering a second one that does
/// nothing is the kind of control this editor is trying not to grow.
pub(super) fn active_row(ui: &mut egui::Ui, disabled: bool, inherited: bool) -> Option<bool> {
    let mut wanted = !disabled && !inherited;
    let mut changed = false;
    property::Property::new("Active")
        .tip(if inherited {
            "Switched off by a parent. The switch that governs this is on the parent."
        } else {
            "Off takes this entity and everything under it out of the scene: not drawn, not stepped, not scripted, not picked"
        })
        .show(ui, |ui| {
            ui.add_enabled_ui(!inherited, |ui| {
                changed = property::switch(ui, &mut wanted, "On", "Off");
            });
        });
    changed.then_some(wanted)
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
    identity: Identity<'_>,
    draft: &mut EntityDraft,
) -> IdentityEdit {
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
            let edit = stable_id(ui, identity);
            ui.add_space(5.0);
            space_badge(ui, space);
            edit
        })
        .inner
    // "Tag  Untagged" and "Layer  Default" used to sit under the name. Neither
    // is a thing a Sindri entity has, so they were two lines of a different
    // engine's inspector printed over this one's.
}

/// The stable ID as the panel hands it over: the text being edited, and why it
/// cannot be used if it cannot.
pub(super) struct Identity<'a> {
    pub(super) text: &'a mut String,
    pub(super) refused: Option<IdentityRefusal>,
}

/// What happened to the ID field this frame.
///
/// Two answers rather than one, because a stable ID is not written as it is
/// typed. Renaming `orb-1` to `player` passes through `p`, `pl`, `pla` — each
/// of which would be a real rename of a real identity, rewriting every
/// component that points at it, and one of which might collide with something.
/// So the panel holds the text until the edit is finished and writes once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct IdentityEdit {
    pub(super) changed: bool,
    pub(super) finished: bool,
}

/// The identity the scene file keys this entity under.
///
/// Under the name because it is the quieter of the two and most authors will
/// leave it alone — but visible and editable, because it is what a parent link
/// names, what sibling order is derived from, and what `sindri.grid.occupant`
/// points at. It was neither, so the editor could produce `game-object-1` and
/// nothing else, and a scene of `player`, `floor` and `orb-1` was unreachable.
///
/// A value that cannot be used is shown in the colour the editor uses for a
/// refusal and says why on hover, rather than being written and rejected: the
/// draft is committed every frame, so a refused command would be refused again
/// on the next one and the console would fill with the same line.
fn stable_id(ui: &mut egui::Ui, identity: Identity<'_>) -> IdentityEdit {
    let Identity { text, refused } = identity;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.add_space(26.0);
        ui.label(
            RichText::new("ID")
                .size(text::NOTE)
                .color(color::TEXT_FAINT),
        );
        let field = ui.add_sized(
            [ui.available_width(), metric::CONTROL_HEIGHT],
            egui::TextEdit::singleline(text)
                .font(FontId::monospace(text::LABEL))
                .text_color(if refused.is_some() {
                    color::DANGER_TEXT
                } else {
                    color::TEXT_MUTED
                })
                .hint_text("game-object"),
        );
        let edit = IdentityEdit {
            changed: field.changed(),
            // Enter and clicking away are both "done", and both arrive here.
            finished: field.lost_focus(),
        };
        match refused {
            Some(refusal) => field.on_hover_text(refusal.reason()),
            None => field.on_hover_text(
                "What the scene file keys this entity by, and what a component naming it points at",
            ),
        };
        edit
    })
    .inner
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
