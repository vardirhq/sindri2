//! The headings that divide the inspector into things.
//!
//! Two kinds, and the difference matters. A *component* is a thing an entity
//! carries: it has a name, it can be folded away, and it can be taken off. A
//! *group* is a heading inside one — Clips, Frames, Preview — which is a label
//! and nothing more. They used to be drawn by the same function, so a tilemap's
//! Palette heading looked exactly like the Tilemap component it was inside.

use eframe::egui::{self, Align, Id, Layout, RichText, Sense, Stroke, UiBuilder, Vec2};
use egui_material_icons::MaterialIcon;

use crate::ui::icons;
use crate::ui::theme::{color, hairline, metric, text};

/// A component's heading: fold state, name, and the component's own actions.
///
/// Returns whether the component's fields should be drawn. Folding is editor
/// state and deliberately not a preference: which components an author had open
/// is about the minute, not the project.
pub fn component(
    ui: &mut egui::Ui,
    id: Id,
    icon: MaterialIcon,
    title: &str,
    actions: impl FnOnce(&mut egui::Ui),
) -> bool {
    let mut open = ui.data_mut(|data| *data.get_temp_mut_or(id, true));
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, metric::HEADER_HEIGHT), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        0.0,
        if hovered {
            color::RAISED
        } else {
            color::HEADER
        },
    );
    painter.hline(rect.x_range(), rect.top() + 0.5, hairline());
    painter.hline(rect.x_range(), rect.bottom() - 0.5, hairline());

    let mut content = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(5.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    content.spacing_mut().item_spacing.x = 4.0;
    content.label(
        if open {
            icons::EXPANDED
        } else {
            icons::COLLAPSED
        }
        .outlined()
        .rich_text()
        .size(15.0)
        .color(color::TEXT_FAINT),
    );
    content.label(icon.outlined().rich_text().size(15.0).color(color::FORGE));
    content.label(
        RichText::new(title)
            .strong()
            .size(text::HEADING)
            .color(color::TEXT),
    );
    let actions_rect = rect.shrink2(Vec2::new(5.0, 2.0));
    let mut trailing = content.new_child(
        UiBuilder::new()
            .max_rect(actions_rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    trailing.spacing_mut().item_spacing.x = 2.0;
    actions(&mut trailing);
    // The strip folds the component, except at the end where its own controls
    // are: a click that removed a component must not also record that the
    // component someone just deleted was folded.
    let on_actions = trailing.min_rect().width() > 0.0
        && response
            .interact_pointer_pos()
            .is_some_and(|pointer| pointer.x >= trailing.min_rect().left() - 4.0);
    if response.clicked() && !on_actions {
        open = !open;
        ui.data_mut(|data| data.insert_temp(id, open));
    }
    response.on_hover_text(if open {
        "Collapse this component"
    } else {
        "Expand this component"
    });
    open
}

/// A heading inside a component: a label for the rows under it.
pub fn group(ui: &mut egui::Ui, icon: MaterialIcon, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
        ui.label(
            icon.outlined()
                .rich_text()
                .size(13.0)
                .color(color::TEXT_FAINT),
        );
        ui.label(
            RichText::new(title.to_uppercase())
                .size(text::NOTE)
                .color(color::TEXT_FAINT),
        );
        let width = ui.available_width() - metric::GUTTER;
        if width > 12.0 {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke::new(1.0, color::LINE_SOFT),
            );
        }
    });
    ui.add_space(3.0);
}

/// A short caption under a heading, for something the panel has to explain.
pub fn caption(ui: &mut egui::Ui, message: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(metric::GUTTER + 4.0);
        ui.add(
            egui::Label::new(
                RichText::new(message)
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            )
            .wrap(),
        );
    });
}
