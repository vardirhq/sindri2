//! Where a click on a hierarchy row lands, and what a drag off one does.

use eframe::egui::{self, Rect, Vec2};
use egui_material_icons::icons::{ICON_ACCOUNT_TREE, ICON_LABEL};
use sindri_core::World;

use crate::ui::widgets::tree::{self, Children, RowStyle};

use super::super::editing::find_by_source_id;
use super::super::hierarchy::row::{HierarchyDrag, hierarchy_drop_target};
use super::support::*;

/// A hierarchy row as the panel draws one, so these tests exercise the widget
/// the editor actually uses rather than a copy of it.
fn hierarchy_row(
    ui: &mut egui::Ui,
    icon: egui_material_icons::MaterialIcon,
    name: &str,
    depth: usize,
    has_children: bool,
) -> tree::TreeRow {
    tree::row(
        ui,
        icon,
        name,
        RowStyle {
            selected: false,
            depth,
            children: Children::of(usize::from(has_children), false),
            dimmed: false,
        },
    )
}

/// Presses and releases the pointer at `target`, and reports whether a
/// hierarchy row drawn at the same place says it was clicked.
///
/// egui reports a click on the release, so the press and the release are
/// separate frames, as they are for a real pointer.
fn hierarchy_row_click_at(offset: Vec2, has_children: bool) -> (bool, bool) {
    let context = egui::Context::default();
    // The row draws a material icon, and the icon font is registered by the
    // same call the running editor makes.
    egui_material_icons::initialize(&context);
    let row = std::cell::Cell::new(Rect::NOTHING);
    let clicked = std::cell::Cell::new(false);
    let toggled = std::cell::Cell::new(false);
    let draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        context
            .run_ui(input, |ui| {
                let response =
                    hierarchy_row(ui, ICON_ACCOUNT_TREE, "Checker Cube", 0, has_children);
                row.set(response.select.rect);
                clicked.set(response.select.clicked());
                toggled.set(response.toggle.is_some_and(|response| response.clicked()));
            })
            .drop_without_applying_deltas();
    };

    draw(Vec::new());
    let target = row.get().left_center() + offset;
    let button = |pressed| egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    draw(vec![egui::Event::PointerMoved(target), button(true)]);
    draw(vec![button(false)]);
    (clicked.get(), toggled.get())
}

fn row_click_at(offset: Vec2) -> bool {
    hierarchy_row_click_at(offset, false).0
}

#[test]
fn a_hierarchy_drag_releases_onto_another_row() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let arm = find_by_source_id(&world, "arm").unwrap();
    let leg = find_by_source_id(&world, "leg").unwrap();
    let context = egui::Context::default();
    egui_material_icons::initialize(&context);
    let source_rect = std::cell::Cell::new(Rect::NOTHING);
    let target_rect = std::cell::Cell::new(Rect::NOTHING);
    let dropped = std::cell::Cell::new(None);
    let draw = |events: Vec<egui::Event>| {
        context
            .run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let source = hierarchy_row(ui, ICON_LABEL, "Arm", 0, false);
                    source.select.dnd_set_drag_payload(HierarchyDrag(arm));
                    source_rect.set(source.select.rect);

                    let target = hierarchy_row(ui, ICON_LABEL, "Leg", 0, false);
                    target_rect.set(target.select.rect);
                    if let Some(entity) = hierarchy_drop_target(ui, &target.drop, &world, Some(leg))
                    {
                        dropped.set(Some(entity));
                    }
                },
            )
            .drop_without_applying_deltas();
    };

    draw(Vec::new());
    let source = source_rect.get().center();
    let target = target_rect.get().center();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    draw(vec![
        egui::Event::PointerMoved(source),
        button(source, true),
    ]);
    draw(vec![egui::Event::PointerMoved(target)]);
    draw(vec![button(target, false)]);

    assert_eq!(dropped.get(), Some(arm));
}

/// The bug that made the editor read-only for a fortnight.
///
/// `hierarchy_row` returned the response of the `ui.horizontal` around the
/// button rather than the button's own. A layout is allocated with
/// `Sense::hover`, so it answers no to `clicked` forever, and selection —
/// which every edit in the editor is behind — could never happen. Reading
/// the code found nothing; driving the editor found it in one click.
#[test]
fn clicking_a_hierarchy_row_reports_the_click() {
    assert!(
        row_click_at(Vec2::new(60.0, 0.0)),
        "clicking a row's name must select it"
    );
}

/// A row answers everywhere, not only on its text.
///
/// The offsets walk across the indent, the icon, and the name. The middle
/// of that range is where the icon sits, and it was a dead patch until the
/// icon was given a sense of its own: a widget inside a click-sensing scope
/// takes precedence over the scope, so a hover-only label swallows the
/// click rather than passing it down.
#[test]
fn a_hierarchy_row_answers_across_its_whole_width() {
    for offset in [2.0_f32, 10.0, 16.0, 22.0, 30.0, 60.0, 90.0] {
        assert!(
            row_click_at(Vec2::new(offset, 0.0)),
            "a click {offset} points into the row was lost"
        );
    }
}

#[test]
fn a_hierarchy_chevron_folds_without_selecting() {
    let (selected, toggled) = hierarchy_row_click_at(Vec2::new(12.0, 0.0), true);
    assert!(toggled, "the child-bearing row's chevron must fold it");
    assert!(!selected, "folding a row must not also change selection");
}
