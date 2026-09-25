//! What the devtools panel is told about one element.

use sindri_core::{SceneDocument, World};
use sindri_weave::{UiStates, inspect};
use weave::{Viewport, parse};

fn scene() -> World {
    let document = SceneDocument::from_json(
        r#"{ "format_version": 10, "metadata": { "name": "inspect" },
             "entities": [
                { "id": "card", "name": "card", "components": {
                    "sindri.ui.shape": { "kind": "rect" },
                    "weave.style": { "classes": ["card"] } } },
                { "id": "title", "name": "title", "parent": "card", "components": {
                    "sindri.ui.text": { "text": "Hi", "font": "f.ttf" },
                    "weave.style": { "classes": ["title"] } } }
             ] }"#,
    )
    .expect("scene parses");
    World::from_scene(&document).expect("scene loads").world
}

const VIEW: Viewport = Viewport {
    width: 1_280.0,
    height: 720.0,
};

#[test]
fn inspecting_an_element_lists_its_rules_strongest_first_and_what_applies() {
    let world = scene();
    let base = parse(
        ":root { --accent: #f00; }\n\
         .card { color: #fff; }\n\
         text { font-size: 12px; color: #aaa; }\n\
         .title { font-size: 20px; }",
    )
    .expect("base parses");
    let theme = parse("text { font-size: 24px; }").expect("theme parses");
    let (title, _) = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("title"))
        .expect("the title is there");

    let inspection = inspect(&world, &[base, theme], VIEW, &UiStates::new(), title)
        .expect("the title is inspected");

    let listed: Vec<(usize, &str, usize)> = inspection
        .rules
        .iter()
        .map(|rule| (rule.sheet, rule.origin.selector.as_str(), rule.origin.line))
        .collect();
    // The theme's rule first, then the base sheet's, class before element.
    assert_eq!(listed, [(1, "text", 1), (0, ".title", 4), (0, "text", 3)]);
    // The theme's font size wins over the base's class rule, as a later
    // stylesheet does; the base `text` colour applies, since the card's is
    // only inherited.
    let applies = |index: usize, property: &str| {
        inspection.rules[index]
            .declarations
            .iter()
            .find(|(name, _, _)| name == property)
            .map(|(_, _, won)| *won)
            .expect("the rule declares it")
    };
    assert!(applies(0, "font-size"));
    assert!(!applies(1, "font-size"));
    assert!(!applies(2, "font-size"));
    assert!(applies(2, "color"));

    let computed = |property: &str| {
        inspection
            .computed
            .iter()
            .find(|(name, _)| name == property)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(computed("font-size"), Some("24px"));
    assert_eq!(computed("color"), Some("#aaa"));
    assert_eq!(
        inspection.variables,
        [("--accent".to_owned(), "#f00".to_owned())]
    );
}
