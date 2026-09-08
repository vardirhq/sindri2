use std::collections::BTreeMap;

use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Stylesheet, Viewport, compose};

const SCENE: &str = include_str!("../../../games/orbital-last-stand/assets/orbital.scene.json");
const STYLE_ENTRY: &str = include_str!("../../../games/orbital-last-stand/assets/ui.weave");
const STYLE_HUD: &str = include_str!("../../../games/orbital-last-stand/assets/ui/hud.weave");
const STYLE_OVERLAYS: &str =
    include_str!("../../../games/orbital-last-stand/assets/ui/overlays.weave");
const STYLE_SCREENS: &str =
    include_str!("../../../games/orbital-last-stand/assets/ui/screens.weave");

fn stylesheet() -> Stylesheet {
    let sources = BTreeMap::from([
        ("ui.weave".to_owned(), STYLE_ENTRY.to_owned()),
        ("ui/hud.weave".to_owned(), STYLE_HUD.to_owned()),
        ("ui/overlays.weave".to_owned(), STYLE_OVERLAYS.to_owned()),
        ("ui/screens.weave".to_owned(), STYLE_SCREENS.to_owned()),
    ]);
    compose("ui.weave", &sources).expect("Last Stand Weave composes")
}

fn entity<'a>(world: &'a World, id: &str) -> &'a sindri_core::EntityData {
    world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|source_id| source_id.as_str() == id)
        })
        .map(|(_, data)| data)
        .expect("Last Stand UI entity")
}

fn size(world: &World, id: &str) -> [f32; 2] {
    entity(world, id)
        .transform_3d
        .expect("UI transform")
        .scale_2d()
}

fn number(world: &World, id: &str, component: &str, field: &str) -> f64 {
    entity(world, id).components[component][field]
        .as_f64()
        .expect("numeric component field")
}

#[test]
fn last_stand_hud_resolves_for_desktop_and_phone_without_mutating_the_scene() {
    let document = SceneDocument::from_json(SCENE).expect("Last Stand scene parses");
    let authored = World::from_scene(&document)
        .expect("Last Stand scene loads")
        .world;
    let authored_health_width = size(&authored, "hud-hp")[0];
    let stylesheet = stylesheet();

    let desktop = PresentationWorld::resolve(
        &authored,
        &stylesheet,
        Viewport {
            width: 1280.0,
            height: 720.0,
        },
    )
    .expect("desktop HUD resolves");
    let phone = PresentationWorld::resolve(
        &authored,
        &stylesheet,
        Viewport {
            width: 390.0,
            height: 844.0,
        },
    )
    .expect("phone HUD resolves");

    let desktop_health_width = size(desktop.world(), "hud-hp")[0];
    let phone_health_width = size(phone.world(), "hud-hp")[0];
    assert!((desktop_health_width - 1.0).abs() < 1.0e-6);
    assert!((phone_health_width - (2.0 * 0.70 * 390.0 / 844.0)).abs() < 1.0e-6);
    assert_ne!(desktop_health_width, phone_health_width);

    let desktop_clock_size = number(desktop.world(), "hud-clock", "sindri.ui.text", "font_size");
    let phone_clock_size = number(phone.world(), "hud-clock", "sindri.ui.text", "font_size");
    assert!((desktop_clock_size - (2.0 * 40.0 / 720.0)).abs() < 1.0e-6);
    assert!((phone_clock_size - (2.0 * 0.07 * 390.0 / 844.0)).abs() < 1.0e-6);
    assert_ne!(desktop_clock_size, phone_clock_size);

    assert_eq!(size(&authored, "hud-hp")[0], authored_health_width);
}

#[test]
fn last_stand_dynamic_hud_components_survive_presentation_resolution() {
    let document = SceneDocument::from_json(SCENE).expect("Last Stand scene parses");
    let authored = World::from_scene(&document)
        .expect("Last Stand scene loads")
        .world;
    let stylesheet = stylesheet();
    let styled = PresentationWorld::resolve(
        &authored,
        &stylesheet,
        Viewport {
            width: 1280.0,
            height: 720.0,
        },
    )
    .expect("HUD resolves");

    assert_eq!(
        entity(styled.world(), "hud-score").components["sindri.ui.text"]["text"],
        "Score {}"
    );
    assert_eq!(
        entity(styled.world(), "hud-hp").components["sindri.ui.image"]["fill"]["amount"],
        1.0
    );
    assert_eq!(
        entity(styled.world(), "hud-cores").components["sindri.ui.image"]["fill"]["amount"],
        0.0
    );
}

fn string<'a>(world: &'a World, id: &str, component: &str, field: &str) -> &'a str {
    entity(world, id).components[component][field]
        .as_str()
        .expect("string component field")
}

#[test]
fn last_stand_upgrade_choices_recompose_on_phone() {
    let document = SceneDocument::from_json(SCENE).expect("Last Stand scene parses");
    let authored = World::from_scene(&document)
        .expect("Last Stand scene loads")
        .world;
    let stylesheet = stylesheet();

    let desktop = PresentationWorld::resolve(
        &authored,
        &stylesheet,
        Viewport {
            width: 1280.0,
            height: 720.0,
        },
    )
    .expect("desktop upgrades resolve");
    let phone = PresentationWorld::resolve(
        &authored,
        &stylesheet,
        Viewport {
            width: 390.0,
            height: 844.0,
        },
    )
    .expect("phone upgrades resolve");

    assert_eq!(
        string(
            desktop.world(),
            "upgrade-row",
            "sindri.ui.layout",
            "direction"
        ),
        "row"
    );
    assert_eq!(
        string(
            phone.world(),
            "upgrade-row",
            "sindri.ui.layout",
            "direction"
        ),
        "column"
    );
    assert_eq!(
        string(phone.world(), "upgrade-row", "sindri.ui.layout", "align"),
        "center"
    );
    assert_eq!(
        string(phone.world(), "title-hint", "sindri.ui.text", "wrap"),
        "word"
    );
}
