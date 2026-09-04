//! Buttons: a script acting on a click, and gameplay not acting on the same one.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::{InputEvent, InputState, MouseButton};
use sindri_scene::{SceneExtractor, ScreenExtent, ScreenUi, UiButtonComponent, UiImageComponent};
use std::time::Duration;

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

/// A mouse at the middle of the screen, driven the way a host drives one.
///
/// One state carried across frames, because that is what a host has. This used
/// to be a fresh state per frame with a hand-written pointer frame beside it,
/// and the interface was read from the hand-written one -- so a release could
/// be described on a frame where no press had ever happened, which is not a
/// thing any device can do. Against real presses it is not expressible: a
/// release with no press finishes nothing.
struct Pointer {
    input: InputState,
}

impl Pointer {
    fn new() -> Self {
        let mut input = InputState::default();
        input.apply(InputEvent::PointerMoved {
            x: WIDTH / 2.0,
            y: HEIGHT / 2.0,
        });
        Self { input }
    }

    fn press(&mut self) {
        self.input
            .apply(InputEvent::ButtonPressed(MouseButton::Left));
    }

    fn release(&mut self) {
        self.input
            .apply(InputEvent::ButtonReleased(MouseButton::Left));
    }

    /// Spends the frame's edges, as a host does between frames.
    fn frame(&mut self) {
        self.input.begin_frame(Duration::from_millis(16));
    }
}

fn run(
    scripts: &mut Scripts,
    world: &mut World,
    source: &str,
    screen: &mut ScreenUi,
    state: &InputState,
) -> ScriptReport {
    let components = registry();
    screen
        .update(
            world,
            &components,
            ScreenExtent::new(WIDTH, HEIGHT),
            state.presses(),
        )
        .expect("registered");
    let mut sources = ScriptSources::new();
    sources.insert("button.decay", source);
    scripts.advance(
        world,
        &components,
        ScriptFrame::new(&sources, state, 1.0 / 60.0).with_screen_ui(screen),
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

    let mut pointer = Pointer::new();
    pointer.press();
    let report = run(
        &mut scripts,
        &mut world,
        COUNTER,
        &mut screen,
        &pointer.input,
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(
        clicks(&world, entity).abs() < 1.0e-5,
        "a press is not a click"
    );

    pointer.frame();
    pointer.release();
    run(
        &mut scripts,
        &mut world,
        COUNTER,
        &mut screen,
        &pointer.input,
    );
    assert!((clicks(&world, entity) - 1.0).abs() < 1.0e-5, "no click");

    // The frame after: the click is over, and must not fire again.
    pointer.frame();
    run(
        &mut scripts,
        &mut world,
        COUNTER,
        &mut screen,
        &pointer.input,
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
        &Pointer::new().input,
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
