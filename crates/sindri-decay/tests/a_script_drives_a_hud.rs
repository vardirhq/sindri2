//! Dynamic text and fill: a script changing what a screen element says.
//!
//! The audit's finding was that screen text and images render but "Decay cannot
//! change text content", which makes a HUD a picture of a HUD. What closes it is
//! not string building — Decay deliberately has none — but a split: the scene
//! authors the words and the script supplies the numbers.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::{UiImageComponent, UiTextComponent};

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
        .register::<UiTextComponent>("Text")
        .expect("sindri.ui.text registers");
    registry
        .register::<UiImageComponent>("Image")
        .expect("sindri.ui.image registers");
    registry
}

/// One entity carrying a script, a text template, and a bar.
fn hud(world: &mut World, template: &str) -> EntityId {
    world.spawn(EntityData {
        name: Some("Hud".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [
            (
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "hud.decay", "script": "Hud" }),
            ),
            (
                UiTextComponent::TYPE_NAME.to_owned(),
                json!({ "text": template, "font": "font.ttf" }),
            ),
            (
                UiImageComponent::TYPE_NAME.to_owned(),
                json!({ "texture": "bar.png" }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

fn run(world: &mut World, script: &str) -> ScriptReport {
    let mut sources = ScriptSources::new();
    sources.insert("hud.decay", script);
    let input = InputState::default();
    Scripts::new().advance(
        world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0),
    )
}

/// What the element would draw, which is the template with its slots filled.
fn shown(world: &World, entity: EntityId) -> String {
    let payload = world.get(entity).expect("there").components[UiTextComponent::TYPE_NAME].clone();
    serde_json::from_value::<UiTextComponent>(payload)
        .expect("a text component")
        .resolved()
}

fn fill(world: &World, entity: EntityId) -> f32 {
    let payload = world.get(entity).expect("there").components[UiImageComponent::TYPE_NAME].clone();
    serde_json::from_value::<UiImageComponent>(payload)
        .expect("an image component")
        .fill
        .fraction()
}

/// The case the whole design exists for: a score a script computes, in words a
/// designer wrote.
#[test]
fn a_script_puts_a_number_into_words_it_did_not_write() {
    let mut world = World::default();
    let entity = hud(&mut world, "Score: {}");
    let report = run(
        &mut world,
        r"
        script Hud {
            fn start() { Ui.set_number(this.entity, 1200.0); }
        }
        ",
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(shown(&world, entity), "Score: 1200");
}

#[test]
fn two_slots_take_two_numbers() {
    let mut world = World::default();
    let entity = hud(&mut world, "{}/{}");
    run(
        &mut world,
        r"
        script Hud {
            fn start() { Ui.set_numbers(this.entity, 45.0, 100.0); }
        }
        ",
    );
    assert_eq!(shown(&world, entity), "45/100");
}

/// The template survives being filled, so the next frame's number lands in the
/// same words. A script that wrote a finished string would consume it.
#[test]
fn the_words_survive_the_numbers() {
    let mut world = World::default();
    let entity = hud(&mut world, "Score: {}");
    for _ in 0..3 {
        run(
            &mut world,
            r"
            script Hud {
                fn update(dt: f32) { Ui.set_number(this.entity, 7.0); }
            }
            ",
        );
    }
    assert_eq!(shown(&world, entity), "Score: 7");
}

#[test]
fn a_script_can_swap_one_authored_string_for_another() {
    let mut world = World::default();
    let entity = hud(&mut world, "Playing");
    run(
        &mut world,
        r#"
        script Hud {
            fn start() { Ui.set_text(this.entity, "Game Over"); }
        }
        "#,
    );
    assert_eq!(shown(&world, entity), "Game Over");
}

/// A swapped-in string is a template like any other, so the numbers keep
/// working after the words change.
#[test]
fn a_swapped_string_is_a_template_too() {
    let mut world = World::default();
    let entity = hud(&mut world, "Playing");
    run(
        &mut world,
        r#"
        script Hud {
            fn start() {
                Ui.set_text(this.entity, "Wave {}");
                Ui.set_number(this.entity, 3.0);
            }
        }
        "#,
    );
    assert_eq!(shown(&world, entity), "Wave 3");
}

#[test]
fn a_script_drives_a_bar() {
    let mut world = World::default();
    let entity = hud(&mut world, "{}");
    run(
        &mut world,
        r"
        script Hud {
            fn start() { Ui.set_fill(this.entity, 0.25); }
        }
        ",
    );
    assert!((fill(&world, entity) - 0.25).abs() < 1.0e-5);
}

/// A bar driven past its ends is clamped rather than refused: health above the
/// maximum is a full bar, and that is not a bug worth stopping a frame for.
#[test]
fn a_bar_driven_past_its_ends_is_clamped() {
    for (given, expected) in [(2.0, 1.0), (-1.0, 0.0)] {
        let mut world = World::default();
        let entity = hud(&mut world, "{}");
        let script =
            format!("script Hud {{ fn start() {{ Ui.set_fill(this.entity, {given:.1}); }} }}");
        run(&mut world, &script);
        assert!((fill(&world, entity) - expected).abs() < 1.0e-5, "{given}");
    }
}

/// A HUD that stops updating because a script points at the wrong element is
/// the failure that survives a play-test.
#[test]
fn an_entity_that_is_not_the_kind_of_element_the_call_needs_is_named() {
    let mut world = World::default();
    let bare = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "hud.decay", "script": "Hud" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let report = run(
        &mut world,
        r"
        script Hud {
            fn start() { Ui.set_number(this.entity, 1.0); }
        }
        ",
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("not text"), "{message}");
    assert!(world.get(bare).is_some());
}
