//! Pointing: a script reading where the person is, whatever they point with.
//!
//! Decay could read the keyboard and nothing else, which is enough for a game
//! driven by arrow keys and no use at all to one that is mouse- and touch-first.
//! `Pointer` is one namespace for both, so a game written for a mouse works on a
//! phone without a second code path — and these tests are mostly about the
//! places where "both" has to decide which one wins.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{PrefabSources, ScriptComponent, ScriptReport, ScriptSources, Scripts};
use sindri_platform::{InputEvent, InputState};

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// A world with one scripted entity, and the script behind it.
fn world(script: &str) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        name: Some("Reader".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "reader.decay", "script": "Reader" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("reader.decay", script);
    (world, entity, sources)
}

fn advance(
    scripts: &mut Scripts,
    world: &mut World,
    sources: &ScriptSources,
    input: &InputState,
) -> ScriptReport {
    scripts.advance(
        world,
        &registry(),
        sources,
        &PrefabSources::new(),
        input,
        1.0 / 60.0,
    )
}

fn position(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .expect("still there")
        .transform_3d
        .expect("a transform")
        .position
}

/// Writes the pointer's position into the transform, so a test can read a
/// number a script saw.
const FOLLOW: &str = r"
script Reader {
    fn update(dt: f32) {
        this.transform.position.x = Pointer.x;
        this.transform.position.y = Pointer.y;
    }
}
";

#[test]
fn a_script_reads_where_the_mouse_is() {
    let (mut world, entity, sources) = world(FOLLOW);
    let mut input = InputState::default();
    input.apply(InputEvent::PointerMoved { x: 40.0, y: 25.0 });

    let report = advance(&mut Scripts::new(), &mut world, &sources, &input);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let at = position(&world, entity);
    assert!(
        (at[0] - 40.0).abs() < 1.0e-5 && (at[1] - 25.0).abs() < 1.0e-5,
        "{at:?}"
    );
}

/// The whole point of one namespace: the same script, driven by a finger.
#[test]
fn the_same_script_reads_where_a_finger_is() {
    let (mut world, entity, sources) = world(FOLLOW);
    let mut input = InputState::default();
    input.apply(InputEvent::TouchStarted {
        id: 1,
        x: 12.0,
        y: 8.0,
    });

    let report = advance(&mut Scripts::new(), &mut world, &sources, &input);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let at = position(&world, entity);
    assert!(
        (at[0] - 12.0).abs() < 1.0e-5 && (at[1] - 8.0).abs() < 1.0e-5,
        "{at:?}"
    );
}

#[test]
fn a_tap_and_a_click_are_the_same_line_of_gameplay() {
    let source = r#"
    script Reader {
        fn update(dt: f32) {
            if Pointer.is_down("Left") {
                this.transform.position.z = 1.0;
            }
        }
    }
    "#;
    for event in [
        InputEvent::ButtonPressed(sindri_platform::MouseButton::Left),
        InputEvent::TouchStarted {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
    ] {
        let (mut world, entity, sources) = world(source);
        let mut input = InputState::default();
        input.apply(event);
        let report = advance(&mut Scripts::new(), &mut world, &sources, &input);
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        assert!(
            (position(&world, entity)[2] - 1.0).abs() < 1.0e-5,
            "{event:?}"
        );
    }
}

/// A script that cares has to ask before it believes a position, and the
/// answer has to be honest when nothing is pointing.
#[test]
fn a_script_can_ask_whether_there_is_a_pointer_at_all() {
    let (mut world, entity, sources) = world(
        r"
        script Reader {
            fn update(dt: f32) {
                if Pointer.inside {
                    this.transform.position.z = 1.0;
                } else {
                    this.transform.position.z = -1.0;
                }
            }
        }
        ",
    );
    let mut scripts = Scripts::new();
    let mut input = InputState::default();

    let report = advance(&mut scripts, &mut world, &sources, &input);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(position(&world, entity)[2] < 0.0, "nothing is pointing yet");

    input.apply(InputEvent::PointerMoved { x: 1.0, y: 1.0 });
    advance(&mut scripts, &mut world, &sources, &input);
    assert!(position(&world, entity)[2] > 0.0, "the mouse arrived");

    input.apply(InputEvent::PointerLeft);
    advance(&mut scripts, &mut world, &sources, &input);
    assert!(position(&world, entity)[2] < 0.0, "and left again");
}

#[test]
fn a_script_reads_a_second_finger_through_touch() {
    let (mut world, entity, sources) = world(
        r"
        script Reader {
            fn update(dt: f32) {
                this.transform.position.z = Touch.count;
                if Touch.count > 1.0 {
                    this.transform.position.x = Touch.x(1.0);
                }
            }
        }
        ",
    );
    let mut input = InputState::default();
    input.apply(InputEvent::TouchStarted {
        id: 1,
        x: 5.0,
        y: 0.0,
    });
    input.apply(InputEvent::TouchStarted {
        id: 2,
        x: 90.0,
        y: 0.0,
    });

    let report = advance(&mut Scripts::new(), &mut world, &sources, &input);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let at = position(&world, entity);
    assert!((at[2] - 2.0).abs() < 1.0e-5, "two fingers: {at:?}");
    assert!((at[0] - 90.0).abs() < 1.0e-5, "the second one: {at:?}");
}

/// A button name that names nothing is a control that silently does nothing,
/// which is the failure this whole surface is arranged to avoid.
#[test]
fn a_button_name_nothing_answers_to_is_refused() {
    let (mut world, _, sources) = world(
        r#"
        script Reader {
            fn update(dt: f32) {
                if Pointer.is_down("Thumb") {
                    this.transform.position.z = 1.0;
                }
            }
        }
        "#,
    );
    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources,
        &InputState::default(),
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("no pointer button called"), "{message}");
}

/// Asking for a finger that is not down is a bound that is wrong. Answering
/// zero would read as a finger in the corner of the screen.
#[test]
fn asking_for_a_finger_that_is_not_down_is_refused() {
    let (mut world, _, sources) = world(
        r"
        script Reader {
            fn update(dt: f32) {
                this.transform.position.x = Touch.x(2.0);
            }
        }
        ",
    );
    let report = advance(
        &mut Scripts::new(),
        &mut world,
        &sources,
        &InputState::default(),
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("are down"), "{message}");
}

/// The drag the reference game is built on, written the way a game writes it:
/// where the pointer went down, subtracted from where it is now. There is no
/// engine abstraction for this, and this test is the argument that none is
/// needed.
#[test]
fn a_drag_is_four_lines_of_gameplay() {
    let (mut world, entity, sources) = world(
        r#"
        script Reader {
            var from_x: f32 = 0.0;
            var dragging: bool = false;
            fn update(dt: f32) {
                if Pointer.just_pressed("Left") {
                    this.from_x = Pointer.x;
                    this.dragging = true;
                }
                if this.dragging && Pointer.is_down("Left") {
                    this.transform.position.x = Pointer.x - this.from_x;
                }
                if Pointer.just_released("Left") {
                    this.dragging = false;
                }
            }
        }
        "#,
    );
    let mut scripts = Scripts::new();
    let mut input = InputState::default();

    input.apply(InputEvent::TouchStarted {
        id: 1,
        x: 100.0,
        y: 0.0,
    });
    advance(&mut scripts, &mut world, &sources, &input);
    assert!(position(&world, entity)[0].abs() < 1.0e-5, "no drag yet");

    input.begin_frame();
    input.apply(InputEvent::TouchMoved {
        id: 1,
        x: 130.0,
        y: 0.0,
    });
    advance(&mut scripts, &mut world, &sources, &input);
    assert!(
        (position(&world, entity)[0] - 30.0).abs() < 1.0e-5,
        "dragged thirty pixels"
    );
}
