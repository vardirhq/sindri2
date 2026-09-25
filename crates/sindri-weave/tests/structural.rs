//! Structural pseudo-classes, sibling combinators and `:not()`, `:is()` and
//! `:where()`, matched against a real scene in its own order.

use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

/// A menu of four buttons, the second one primary, then a label and a
/// slider that follows it. Written in scene order.
fn menu() -> World {
    let button = |id: &str, class: &str| {
        format!(
            r#"{{ "id": "{id}", "name": "{id}", "parent": "menu", "components": {{
                "sindri.ui.shape": {{ "kind": "rect" }},
                "sindri.ui.button": {{ "label": "{id}" }},
                "weave.style": {{ "classes": ["{class}"] }} }} }}"#
        )
    };
    let document = SceneDocument::from_json(&format!(
        r#"{{ "format_version": 10, "metadata": {{ "name": "menu" }},
             "entities": [
                {{ "id": "menu", "name": "menu", "components": {{
                    "sindri.ui.shape": {{ "kind": "rect" }} }} }},
                {button_a}, {button_b}, {button_c}, {button_d},
                {{ "id": "caption", "name": "caption", "parent": "menu", "components": {{
                    "sindri.ui.text": {{ "text": "Volume", "font": "f.ttf" }} }} }},
                {{ "id": "volume", "name": "volume", "parent": "menu", "components": {{
                    "sindri.ui.shape": {{ "kind": "rect" }},
                    "sindri.ui.slider": {{ "label": "volume" }} }} }}
             ] }}"#,
        button_a = button("a", "plain"),
        button_b = button("b", "primary"),
        button_c = button("c", "plain"),
        button_d = button("d", "plain"),
    ))
    .expect("the scene parses");
    World::from_scene(&document).expect("the scene loads").world
}

fn fills(sheet: &str) -> Vec<(String, f64)> {
    let styled = PresentationWorld::resolve(
        &menu(),
        &parse(sheet).expect("Weave parses"),
        Viewport {
            width: 800.0,
            height: 800.0,
        },
    )
    .expect("styles resolve");
    let mut fills: Vec<(String, f64)> = styled
        .world()
        .entities()
        .filter_map(|(_, data)| {
            let red = data
                .components
                .get("sindri.ui.shape")?
                .get("fill")?
                .get(0)?
                .as_f64()?;
            Some((data.name.clone()?, red))
        })
        .collect();
    fills.sort_by(|a, b| a.0.cmp(&b.0));
    fills
}

/// The names of the elements a rule painted white.
fn painted(sheet: &str) -> Vec<String> {
    fills(sheet)
        .into_iter()
        .filter(|(_, red)| *red > 0.99)
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn structural_pseudo_classes_pick_by_scene_order() {
    assert_eq!(painted("button:first-child { background: white; }"), ["a"]);
    // The last child of the menu is the slider, not a button.
    assert!(painted("button:last-child { background: white; }").is_empty());
    assert_eq!(
        painted("slider:last-child { background: white; }"),
        ["volume"]
    );
    assert_eq!(
        painted("button:nth-child(odd) { background: white; }"),
        ["a", "c"]
    );
    assert_eq!(
        painted("button:nth-child(-n+2) { background: white; }"),
        ["a", "b"]
    );
    assert_eq!(painted("#menu:only-child { background: white; }"), ["menu"]);
}

#[test]
fn not_is_and_sibling_combinators_match_as_css_does() {
    assert_eq!(
        painted("button:not(.primary) { background: white; }"),
        ["a", "c", "d"]
    );
    assert_eq!(
        painted("button:not(.primary, :first-child) { background: white; }"),
        ["c", "d"]
    );
    assert_eq!(painted(":is(#a, #d) { background: white; }"), ["a", "d"]);
    // The slider straight after the caption text, and every button after
    // the primary one.
    assert_eq!(painted("text + slider { background: white; }"), ["volume"]);
    assert_eq!(
        painted(".primary ~ button { background: white; }"),
        ["c", "d"]
    );
}

#[test]
fn is_counts_its_most_specific_argument_and_where_counts_nothing() {
    // `:is(#b)` is as specific as an ID, so it beats the class rule after it.
    assert_eq!(
        painted(":is(#b) { background: white; } .primary { background: black; }"),
        ["b"]
    );
    // `:where(#b)` counts for nothing, so the later class rule wins.
    assert!(
        painted(":where(#b) { background: white; } .primary { background: black; }").is_empty()
    );
    // `:not(#x)` is as specific as an ID too, whatever it excludes.
    assert_eq!(
        painted("button:not(#x) { background: white; } #b.primary { background: black; }"),
        ["a", "c", "d"]
    );
}
