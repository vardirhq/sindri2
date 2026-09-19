//! Gestures: a script reading what the person meant, not which button moved.
//!
//! A mouse has a second button to mean "remove" and a finger does not, so on a
//! phone removing has to be a hold and panning has to be a drag. Neither can be
//! told from `Pointer` alone: the difference between a tap and the start of a
//! drag is how the press ends and how far it wandered getting there, which is a
//! question about a press's whole life rather than about this instant.
//!
//! The recogniser for this already existed and nothing drove it, so every game
//! that wanted a tap had to write one in Decay. These tests are the proof that
//! a script can now ask.

use std::time::Duration;

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, GestureLimits, Gestures, SceneComponent,
    Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::{InputEvent, InputState, MouseButton};

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// A script that writes down what it was told, in the only place a test can
/// read without a host: its own transform.
const READER: &str = r"
script Reader {
    fn update(dt: f32) {
        var x: f32 = 0.0;
        var y: f32 = 0.0;
        var z: f32 = 0.0;
        if Gesture.tapped {
            x = 1.0;
        }
        if Gesture.held {
            y = 1.0;
        }
        if Gesture.dragging {
            z = Gesture.drag_x;
        }
        this.transform.position.x = x;
        this.transform.position.y = y;
        this.transform.position.z = z;
    }
}
";

fn world() -> (World, EntityId, ScriptSources) {
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
    sources.insert("reader.decay", READER);
    (world, entity, sources)
}

/// Whether the script wrote a flag into that axis.
///
/// It only ever writes a zero or a one there, but they arrive as floats, and
/// comparing floats exactly is a habit worth not having even where it is safe.
fn flagged(world: &World, entity: EntityId, axis: usize) -> bool {
    told(world, entity)[axis] > 0.5
}

fn told(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .expect("the reader is still there")
        .transform_3d
        .expect("it kept its transform")
        .position
}

/// Drives one frame: the events, then the recogniser, then the script.
fn frame(
    scripts: &mut Scripts,
    world: &mut World,
    sources: &ScriptSources,
    input: &mut InputState,
    gestures: &mut Gestures,
    events: &[InputEvent],
    elapsed: Duration,
) {
    input.begin_frame(elapsed);
    for event in events {
        input.apply(*event);
    }
    gestures.update(input.presses());
    let report = scripts.advance(
        world,
        &registry(),
        ScriptFrame::new(sources, input, elapsed.as_secs_f32()).with_gestures(gestures),
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
}

#[test]
fn a_press_that_arrives_and_leaves_where_it_landed_is_a_tap() {
    let (mut world, entity, sources) = world();
    let mut scripts = Scripts::new();
    let mut input = InputState::default();
    let mut gestures = Gestures::new(GestureLimits::default());
    let step = Duration::from_millis(16);

    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[
            InputEvent::PointerMoved { x: 40.0, y: 60.0 },
            InputEvent::ButtonPressed(MouseButton::Left),
        ],
        step,
    );
    assert!(
        !flagged(&world, entity, 0),
        "a press alone is not yet a tap"
    );

    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[InputEvent::ButtonReleased(MouseButton::Left)],
        step,
    );
    assert!(
        flagged(&world, entity, 0),
        "letting go where it landed is a tap"
    );
}

#[test]
fn a_press_that_stays_down_and_still_is_a_hold() {
    // What removing a block has to be on a phone, because a finger has no
    // second button.
    let (mut world, entity, sources) = world();
    let mut scripts = Scripts::new();
    let mut input = InputState::default();
    let mut gestures = Gestures::new(GestureLimits::default());

    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[
            InputEvent::PointerMoved { x: 10.0, y: 10.0 },
            InputEvent::ButtonPressed(MouseButton::Left),
        ],
        Duration::from_millis(16),
    );
    assert!(!flagged(&world, entity, 1), "not held yet");

    // Past the long-press limit, without moving.
    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[],
        Duration::from_millis(600),
    );
    assert!(
        flagged(&world, entity, 1),
        "a press that stayed still and stayed down is a hold, reported while \
         the finger is still there"
    );
}

#[test]
fn a_press_that_travels_is_a_drag_and_never_a_tap() {
    // The one that makes panning possible: a finger dragged across the world
    // must not also lay a block where it let go.
    let (mut world, entity, sources) = world();
    let mut scripts = Scripts::new();
    let mut input = InputState::default();
    let mut gestures = Gestures::new(GestureLimits::default());
    let step = Duration::from_millis(16);

    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[
            InputEvent::PointerMoved { x: 10.0, y: 10.0 },
            InputEvent::ButtonPressed(MouseButton::Left),
        ],
        step,
    );
    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[InputEvent::PointerMoved { x: 90.0, y: 10.0 }],
        step,
    );
    let moved = told(&world, entity);
    assert!(
        moved[2] > 0.0,
        "a moving press is a drag, and says how far it moved: {moved:?}"
    );

    frame(
        &mut scripts,
        &mut world,
        &sources,
        &mut input,
        &mut gestures,
        &[InputEvent::ButtonReleased(MouseButton::Left)],
        step,
    );
    assert!(
        !flagged(&world, entity, 0),
        "and letting go after travelling is not a tap, or every pan would \
         build a block where it ended"
    );
}
