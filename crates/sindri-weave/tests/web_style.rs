//! Styling a scene the way a web page is styled: selectors that reach into
//! the hierarchy, text that inherits from its container, theme variables,
//! and rules that follow what the pointer is doing.

use sindri_core::{SceneDocument, World};
use sindri_weave::{PresentationWorld, UiStates};
use weave::{States, Viewport, parse};

const VIEW: Viewport = Viewport {
    width: 1_280.0,
    height: 720.0,
};

/// A menu panel holding a play button with a label, and a disabled slider.
fn menu() -> World {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 10,
            "metadata": { "name": "menu" },
            "entities": [
                {
                    "id": "menu",
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" },
                        "weave.style": { "classes": ["menu"] }
                    }
                },
                {
                    "id": "play",
                    "parent": "menu",
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" },
                        "sindri.ui.button": {},
                        "weave.style": { "classes": ["primary"] }
                    }
                },
                {
                    "id": "play-label",
                    "parent": "play",
                    "components": {
                        "sindri.ui.text": { "text": "Play", "font": "fonts/ui.ttf" }
                    }
                },
                {
                    "id": "volume",
                    "parent": "menu",
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" },
                        "sindri.ui.slider": { "disabled": true }
                    }
                }
            ]
        }"#,
    )
    .expect("scene parses");
    World::from_scene(&document).expect("scene loads").world
}

fn field(world: &World, id: &str, component: &str, name: &str) -> serde_json::Value {
    let entity = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|source| source.as_str() == id)
        })
        .map(|(entity, _)| entity)
        .expect("the entity");
    world.get(entity).expect("live").components[component][name].clone()
}

fn colour(world: &World, id: &str, component: &str, name: &str) -> [f64; 4] {
    let value = field(world, id, component, name);
    let channels: Vec<f64> = value
        .as_array()
        .expect("a colour")
        .iter()
        .map(|channel| channel.as_f64().expect("a number"))
        .collect();
    [channels[0], channels[1], channels[2], channels[3]]
}

#[test]
fn text_inside_a_panel_takes_the_panel_colour() {
    let sheet = parse(".menu { color: #ffffff; font-size: 24px; }").expect("parses");
    let styled = PresentationWorld::resolve(&menu(), &sheet, VIEW).expect("resolves");
    let world = styled.world();
    assert_eq!(
        colour(world, "play-label", "sindri.ui.text", "color"),
        [1.0; 4]
    );
    let size = field(world, "play-label", "sindri.ui.text", "font_size")
        .as_f64()
        .expect("a size");
    assert!((size - 24.0 * 2.0 / 720.0).abs() < 1.0e-6, "{size}");
}

#[test]
fn a_descendant_selector_reaches_into_the_hierarchy() {
    let sheet = parse(".menu button.primary text { color: #000000; } text { color: #ffffff; }")
        .expect("parses");
    let styled = PresentationWorld::resolve(&menu(), &sheet, VIEW).expect("resolves");
    assert_eq!(
        colour(styled.world(), "play-label", "sindri.ui.text", "color"),
        [0.0, 0.0, 0.0, 1.0],
        "the more specific descendant rule wins"
    );
}

#[test]
fn theme_variables_are_written_once_and_used_anywhere() {
    let sheet = parse(
        "
        .menu { --accent: #000000; }
        .primary { background: var(--accent); }
        slider { background: var(--track, #ffffff); }
        ",
    )
    .expect("parses");
    let styled = PresentationWorld::resolve(&menu(), &sheet, VIEW).expect("resolves");
    let world = styled.world();
    assert_eq!(
        colour(world, "play", "sindri.ui.shape", "fill"),
        [0.0, 0.0, 0.0, 1.0]
    );
    assert_eq!(colour(world, "volume", "sindri.ui.shape", "fill"), [1.0; 4]);
}

#[test]
fn hover_and_press_follow_the_pointer() {
    let sheet = parse(
        "
        button { background: #000000; }
        button:hover { background: #ffffff; }
        button:active { background: #ff0000; }
        ",
    )
    .expect("parses");
    let world = menu();
    let play = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "play")
        })
        .map(|(entity, _)| entity)
        .expect("the button");
    let fill = |states: States| {
        let styled = PresentationWorld::resolve_with_states(
            &world,
            &sheet,
            VIEW,
            &UiStates::from([(play, states)]),
        )
        .expect("resolves");
        colour(styled.world(), "play", "sindri.ui.shape", "fill")
    };
    assert_eq!(fill(States::NONE), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(fill(States::HOVER), [1.0; 4]);
    assert_eq!(
        fill(States::HOVER.with(States::ACTIVE)),
        [1.0, 0.0, 0.0, 1.0]
    );
}

#[test]
fn a_disabled_control_is_disabled_without_being_told() {
    let sheet = parse("slider { background: #ffffff; } slider:disabled { background: #000000; }")
        .expect("parses");
    let styled = PresentationWorld::resolve(&menu(), &sheet, VIEW).expect("resolves");
    assert_eq!(
        colour(styled.world(), "volume", "sindri.ui.shape", "fill"),
        [0.0, 0.0, 0.0, 1.0]
    );
}
