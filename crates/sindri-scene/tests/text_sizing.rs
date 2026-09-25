//! Text that sizes what holds it: a label measured by its font, and a badge
//! that fits the label and its own padding, as `width: auto` does in CSS.

use sindri_core::{SceneDocument, World};
use sindri_render::TextRenderer;
use sindri_scene::{SceneExtractor, ScreenExtent, ScreenUi, UiHierarchy, measure_ui_text};

const FONT: &str = "fonts/Inter.ttf";

fn renderer() -> TextRenderer {
    let mut text = TextRenderer::new();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../game/assets/fonts/Inter.ttf"
    ))
    .expect("the companion game's font is in the repository");
    text.bind_font(FONT, "Inter", bytes);
    text
}

/// A badge laid out as a row that fits its content, holding one label that
/// fits its words. The badge is authored far too big, so fitting is visible.
fn badge(label: &str) -> World {
    let document = SceneDocument::from_json(&format!(
        r#"{{
            "format_version": 10,
            "metadata": {{ "name": "badge" }},
            "entities": [
                {{ "id": "badge", "name": "badge",
                   "transform_3d": {{ "scale": [5.0, 5.0, 1.0] }},
                   "components": {{
                       "sindri.ui.shape": {{ "kind": "rect" }},
                       "sindri.ui.button": {{ "label": "badge" }},
                       "sindri.ui.layout": {{ "direction": "row", "spacing": 0.0,
                                              "fit_content": [true, true] }},
                       "sindri.ui.box": {{ "padding": [0.02, 0.05, 0.02, 0.05] }} }} }},
                {{ "id": "label", "name": "label", "parent": "badge",
                   "transform_3d": {{ "scale": [3.0, 3.0, 1.0] }},
                   "components": {{
                       "sindri.ui.text": {{ "text": "{label}", "font": "{FONT}",
                                            "font_size": 0.06, "line_height": 0.075 }},
                       "sindri.ui.box": {{ "fit_content": [true, true] }} }} }}
            ]
        }}"#
    ))
    .expect("the scene parses");
    World::from_scene(&document).expect("the scene loads").world
}

fn sizes(world: &World, extractor: &SceneExtractor, text: &mut TextRenderer) -> [[f32; 2]; 2] {
    let measured = measure_ui_text(world, extractor.components(), text).expect("measures");
    let hierarchy =
        UiHierarchy::measured(world, extractor.components(), &measured).expect("lays out");
    let size = |name: &str| {
        let (entity, _) = world
            .entities()
            .find(|(_, data)| data.name.as_deref() == Some(name))
            .expect("the entity is there");
        hierarchy
            .placement(entity)
            .expect("a UI element")
            .size_or([0.0; 2])
    };
    [size("badge"), size("label")]
}

#[test]
fn a_badge_fits_its_measured_label_and_its_padding() {
    let extractor = SceneExtractor::new().expect("the builtin components register");
    let mut text = renderer();
    let [badge_size, label] = sizes(&badge("LIVE"), &extractor, &mut text);

    // The label is its words: one line high, and nowhere near its authored 3.
    assert!((label[1] - 0.075).abs() < 1.0e-3, "{label:?}");
    assert!(label[0] > 0.05 && label[0] < 0.5, "{label:?}");
    // The badge is the label and its padding, all round.
    assert!(
        (badge_size[0] - (label[0] + 0.1)).abs() < 1.0e-4,
        "{badge_size:?}"
    );
    assert!(
        (badge_size[1] - (label[1] + 0.04)).abs() < 1.0e-4,
        "{badge_size:?}"
    );

    // Longer words, a wider badge, and no taller.
    let [longer, _] = sizes(&badge("LIVE LAYOUT"), &extractor, &mut text);
    assert!(
        longer[0] > badge_size[0] + 0.1,
        "{longer:?} vs {badge_size:?}"
    );
    assert!((longer[1] - badge_size[1]).abs() < 1.0e-4);
}

#[test]
fn a_fitted_button_is_clicked_at_the_size_it_is_drawn() {
    let extractor = SceneExtractor::new().expect("the builtin components register");
    let mut text = renderer();
    let world = badge("LIVE");
    let measured = measure_ui_text(&world, extractor.components(), &mut text).expect("measures");
    let mut screen = ScreenUi::new();
    screen
        .lay_out(
            &world,
            extractor.components(),
            ScreenExtent::new(800.0, 600.0),
            &measured,
        )
        .expect("lays out");
    let (badge_entity, _) = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("badge"))
        .expect("the badge is there");
    let rect = screen.rect(badge_entity).expect("the badge is laid out");
    assert!(rect.size[0] < 0.5 && rect.size[1] < 0.2, "{rect:?}");

    // Without measuring, the badge keeps what layout alone can say.
    screen
        .lay_out(
            &world,
            extractor.components(),
            ScreenExtent::new(800.0, 600.0),
            &sindri_scene::UiTextSizes::new(),
        )
        .expect("lays out");
    let unmeasured = screen.rect(badge_entity).expect("laid out");
    assert!(unmeasured.size[0] > 1.0, "{unmeasured:?}");
}
