//! What a row in the project browser answers to.

use std::path::PathBuf;

use eframe::egui::{self, Rect, Vec2};

use crate::project::{AssetKind, ProjectEntry};

use super::super::project_panel::BrowserAction;
use super::super::project_panel::row::{asset_row, listing_row};
use super::super::project_panel::state::BrowserState;

/// Whether an asset row reports a click `offset` points into it.
fn asset_row_click_at(kind: AssetKind, offset: Vec2) -> bool {
    driven_row(kind, offset, Reported::Clicked)
}

/// One file for a row test to draw.
fn entry_of(kind: AssetKind) -> ProjectEntry {
    ProjectEntry {
        sprites: Vec::new(),
        path: PathBuf::from("/project/level.scene.json"),
        name: "level.scene.json".to_owned(),
        relative: "level.scene.json".to_owned(),
        kind,
        depth: 0,
    }
}

/// Which answer a driven row is asked for.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Reported {
    Clicked,
    DoubleClicked,
}

/// Whether an asset row reports a double click `offset` points into it.
///
/// Driven through real frames for the same reason the hierarchy row is: the
/// row is a sensing scope wrapped around labels, and whether a label
/// swallows the click is not something reading the code answers.
fn asset_row_double_click_at(kind: AssetKind, offset: Vec2) -> bool {
    driven_row(kind, offset, Reported::DoubleClicked)
}

/// Drives a whole listing row and reports what the panel decided.
fn driven_listing_row(kind: AssetKind, wanted: Reported) -> Option<BrowserAction> {
    let context = egui::Context::default();
    egui_material_icons::initialize(&context);
    let entry = entry_of(kind);
    let mut state = BrowserState::default();
    let row = std::cell::Cell::new(Rect::NOTHING);
    let acted = std::cell::RefCell::new(None);
    let mut draw = |events: Vec<egui::Event>| {
        context
            .run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let before = ui.cursor().min;
                    let chosen = listing_row(ui, &entry, 0, false, None, &mut state);
                    row.set(Rect::from_min_max(before, ui.cursor().min));
                    if let Some(chosen) = chosen {
                        *acted.borrow_mut() = Some(chosen);
                    }
                },
            )
            .drop_without_applying_deltas();
    };

    draw(Vec::new());
    let target = row.get().left_center() + Vec2::new(40.0, 0.0);
    let button = |pressed| egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    draw(vec![egui::Event::PointerMoved(target), button(true)]);
    draw(vec![button(false)]);
    if wanted == Reported::DoubleClicked {
        draw(vec![button(true)]);
        draw(vec![button(false)]);
    }
    acted.into_inner()
}

/// Drives a row through real frames and reports what it said.
fn driven_row(kind: AssetKind, offset: Vec2, wanted: Reported) -> bool {
    let context = egui::Context::default();
    egui_material_icons::initialize(&context);
    let entry = entry_of(kind);
    let state = BrowserState::default();
    let row = std::cell::Cell::new(Rect::NOTHING);
    let opened = std::cell::Cell::new(false);
    let draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        context
            .run_ui(input, |ui| {
                let response = asset_row(ui, &entry, 0, false, None, &state, None);
                row.set(response.rect);
                opened.set(match wanted {
                    Reported::Clicked => response.clicked(),
                    Reported::DoubleClicked => response.double_clicked(),
                });
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
    draw(vec![button(true)]);
    draw(vec![button(false)]);
    opened.get()
}

/// A scene row opens the scene, and answers everywhere rather than only on
/// its text — the same complaint as the hierarchy row, in the other panel.
///
/// The labels have to carry the row's sense: a widget inside a sensing
/// scope takes precedence over the scope, and an ordinary egui label is
/// selectable text, so it answered the double click by selecting the word
/// "json" and the row never heard about it.
#[test]
fn double_clicking_a_scene_row_opens_it() {
    for offset in [2.0_f32, 10.0, 20.0, 40.0, 80.0] {
        assert!(
            asset_row_double_click_at(AssetKind::Scene, Vec2::new(offset, 0.0)),
            "a double click {offset} points into a scene row was lost"
        );
    }
}

/// A texture row responds, because there is now something to do with one:
/// selecting it opens the slicer. It did not until an image had a slice.
#[test]
fn a_texture_row_responds_because_it_can_be_sliced() {
    assert!(asset_row_double_click_at(
        AssetKind::Texture,
        Vec2::new(40.0, 0.0)
    ));
}

/// Every row answers a click, whatever the editor can or cannot do with it.
///
/// This used to be the opposite claim: a row the editor could do nothing with
/// was given `Sense::hover()`, as the signal that it was a listing rather than
/// a control. That reasoning held while the only verb was "open". It stopped
/// holding when the browser gained a selection, because a row that cannot be
/// clicked cannot be selected — and one that cannot be selected has nowhere to
/// put a rename or a delete.
///
/// What a row *means* moved with it: the row is a control now, and the panel
/// decides what pressing one does. See
/// [`only_a_scene_opens_on_a_double_click`].
#[test]
fn every_row_can_be_selected_even_with_nothing_to_open() {
    for kind in [AssetKind::Script, AssetKind::Mesh, AssetKind::Other] {
        assert!(
            asset_row_click_at(kind, Vec2::new(40.0, 0.0)),
            "{kind:?} must still be selectable"
        );
    }
}

/// Selecting marks; only a scene opens.
///
/// The rule the row's sense used to enforce, asserted where the decision now
/// lives. A script row that opened something would be offering a thing the
/// editor cannot do: it lists `.decay` files and cannot open one.
#[test]
fn only_a_scene_opens_on_a_double_click() {
    for kind in [
        AssetKind::Scene,
        AssetKind::Script,
        AssetKind::Mesh,
        AssetKind::Other,
    ] {
        let opened = matches!(
            driven_listing_row(kind, Reported::DoubleClicked),
            Some(BrowserAction::Open(_))
        );
        assert_eq!(
            opened,
            kind == AssetKind::Scene,
            "{kind:?} answered a double click with the wrong thing"
        );
        assert!(
            matches!(
                driven_listing_row(kind, Reported::Clicked),
                Some(BrowserAction::Select(_))
            ),
            "{kind:?} must be selectable by a single click"
        );
    }
}
