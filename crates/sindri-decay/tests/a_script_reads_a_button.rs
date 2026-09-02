//! Buttons: a script acting on a click, and gameplay not acting on the same one.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::{InputEvent, InputState, MouseButton};
use sindri_scene::{
    PointerFrame, SceneExtractor, ScreenExtent, ScreenUi, UiButtonComponent, UiImageComponent,
};

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = SceneExtractor::new()
        .expect("the builtin components register")
        .components()
        .clone();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// A scripted button filling the middle of the screen.
fn scripted_button(world: &mut World, script: &str) -> EntityId {
    world.spawn(EntityData {
        name: Some("Start".to_owned()),
        transform_3d: Some(Transform3D {
            scale: [0.6, 0.3, 1.0],
            ..Transform3D::default()
        }),
        components: [
            (
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "button.decay", "script": script }),
            ),
            (
                UiButtonComponent::TYPE_NAME.to_owned(),
                json!({ "label": "Start" }),
            ),
            (
                UiImageComponent::TYPE_NAME.to_owned(),
                json!({ "texture": "panel.png" }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

/// The pointer at the middle of the screen, in whatever state.
fn pointing(pressed: bool, released: bool, down: bool) -> (InputState, PointerFrame) {
    let mut input = InputState::default();
    input.apply(InputEvent::PointerMoved {
        x: WIDTH / 2.0,
        y: HEIGHT / 2.0,
    });
    if pressed {
        input.apply(InputEvent::ButtonPressed(MouseButton::Left));
    }
    if released {
        input.apply(InputEvent::ButtonReleased(MouseButton::Left));
    }
    let frame = PointerFrame {
        position: Some([WIDTH / 2.0, HEIGHT / 2.0]),
        pressed,
        released,
        down,
    };
    (input, frame)
}

fn run(
    scripts: &mut Scripts,
    world: &mut World,
    source: &str,
    screen: &mut ScreenUi,
    state: &(InputState, PointerFrame),
) -> ScriptReport {
    let components = registry();
    screen
        .update(
            world,
            &components,
            ScreenExtent::new(WIDTH, HEIGHT),
            state.1,
        )
        .expect("registered");
    let mut sources = ScriptSources::new();
    sources.insert("button.decay", source);
    scripts.advance(
        world,
        &components,
        ScriptFrame::new(&sources, &state.0, 1.0 / 60.0).with_screen_ui(screen),
    )
}

const COUNTER: &str = r"
script Start {
    fn update(dt: f32) {
        if Ui.is_pressed(this.entity) {
            this.transform.position.z = this.transform.position.z + 1.0;
        }
    }
}
";

fn clicks(world: &World, entity: EntityId) -> f32 {
    world
        .get(entity)
        .expect("there")
        .transform_3d
        .expect("a transform")
        .position[2]
}

#[test]
fn a_script_acts_on_a_click_and_only_once() {
    let mut world = World::default();
    let entity = scripted_button(&mut world, "Start");
    let mut screen = ScreenUi::new();
    let mut scripts = Scripts::new();

    let report = run(
        &mut scripts,
        &mut world,
        COUNTER,
        &mut screen,
        &pointing(true, false, true),
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(
        clicks(&world, entity).abs() < 1.0e-5,
        "a press is not a click"
    );

    run(
        &mut scripts,
        &mut world,
        COUNTER,
        &mut screen,
        &pointing(false, true, false),
    );
    assert!((clicks(&world, entity) - 1.0).abs() < 1.0e-5, "no click");

    // The frame after: the click is over, and must not fire again.
    run(
        &mut scripts,
        &mut world,
        COUNTER,
        &mut screen,
        &pointing(false, false, false),
    );
    assert!(
        (clicks(&world, entity) - 1.0).abs() < 1.0e-5,
        "one click started two games"
    );
}

/// The line that keeps a click on a pause button from also firing the gun.
#[test]
fn gameplay_can_tell_that_a_screen_element_took_the_pointer() {
    let source = r"
    script Start {
        fn update(dt: f32) {
            if Pointer.over_ui {
                this.transform.position.z = 1.0;
            }
        }
    }
    ";
    let mut world = World::default();
    let entity = scripted_button(&mut world, "Start");
    let mut screen = ScreenUi::new();
    run(
        &mut Scripts::new(),
        &mut world,
        source,
        &mut screen,
        &pointing(false, false, false),
    );
    assert!((clicks(&world, entity) - 1.0).abs() < 1.0e-5);
}

/// A menu whose buttons never respond because nothing is laying them out should
/// be heard about on the first frame, not mistaken for a person who has not
/// clicked yet.
#[test]
fn a_host_laying_out_no_screen_ui_says_so() {
    let mut world = World::default();
    scripted_button(&mut world, "Start");
    let mut sources = ScriptSources::new();
    sources.insert("button.decay", COUNTER);
    let input = InputState::default();
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0),
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("not laying out"), "{message}");
}

/// With no screen UI there is no element to take the pointer, which is a true
/// answer rather than a missing one.
#[test]
fn over_ui_is_false_rather_than_an_error_without_a_screen() {
    let source = r"
    script Start {
        fn update(dt: f32) {
            if Pointer.over_ui { this.transform.position.z = 1.0; }
        }
    }
    ";
    let mut world = World::default();
    let entity = scripted_button(&mut world, "Start");
    let mut sources = ScriptSources::new();
    sources.insert("button.decay", source);
    let input = InputState::default();
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 1.0 / 60.0),
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(clicks(&world, entity).abs() < 1.0e-5);
}
