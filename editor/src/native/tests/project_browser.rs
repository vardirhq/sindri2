//! What a row in the project browser answers to.

use super::super::*;

/// Whether an asset row reports a double click `offset` points into it.
///
/// Driven through real frames for the same reason the hierarchy row is: the
/// row is a sensing scope wrapped around labels, and whether a label
/// swallows the click is not something reading the code answers.
fn asset_row_double_click_at(kind: AssetKind, offset: Vec2) -> bool {
    let context = egui::Context::default();
    egui_material_icons::initialize(&context);
    let entry = ProjectEntry {
        sprites: Vec::new(),
        path: PathBuf::from("/project/level.scene.json"),
        name: "level.scene.json".to_owned(),
        relative: "level.scene.json".to_owned(),
        kind,
        depth: 0,
    };
    let row = std::cell::Cell::new(Rect::NOTHING);
    let opened = std::cell::Cell::new(false);
    let draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        context
            .run_ui(input, |ui| {
                let response = asset_row(ui, &entry, 0, false, None, None);
                row.set(response.rect);
                opened.set(response.double_clicked());
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

/// A row with nothing behind it is still a listing. A script row that
/// responded would be offering something the editor cannot do — it lists
/// `.decay` files and cannot open one.
#[test]
fn a_row_with_nothing_to_open_is_a_listing() {
    for kind in [AssetKind::Script, AssetKind::Mesh, AssetKind::Other] {
        assert!(
            !asset_row_double_click_at(kind, Vec2::new(40.0, 0.0)),
            "{kind:?} has nothing behind it and should not respond"
        );
    }
}
