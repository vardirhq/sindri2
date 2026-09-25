//! Couch play: scripts reading pads by player slot.
//!
//! A pad joins by pressing, and a script learns of it as an edge, so it can
//! make a player for it. Each player then reads only their own pad.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptReport, ScriptSources, Scripts};
use sindri_platform::{GamepadAxis, GamepadButton, InputEvent, InputState, PadId};

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

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
        ScriptFrame::new(sources, input, 1.0 / 60.0),
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

/// x counts joins, y is player 2's stick, z whether player 1 kicked.
const COUCH: &str = r#"
script Reader {
    fn update(dt: f32) {
        if Gamepad.joined() > 0.0 {
            this.transform.position.x = this.transform.position.x + Gamepad.joined();
        }
        this.transform.position.y = Gamepad.axis(2.0, "left_x");
        if Gamepad.just_pressed(1.0, "right_bumper") {
            this.transform.position.z = 1.0;
        }
    }
}
"#;

#[test]
fn players_join_by_pressing_and_each_reads_their_own_pad() {
    let (mut world, entity, sources) = world(COUCH);
    let mut scripts = Scripts::new();
    let mut input = InputState::default();
    let (first, second) = (PadId(7), PadId(3));
    input.apply(InputEvent::GamepadConnected(first));
    input.apply(InputEvent::GamepadConnected(second));
    input.apply(InputEvent::GamepadPressed {
        pad: first,
        button: GamepadButton::South,
    });
    input.apply(InputEvent::GamepadPressed {
        pad: second,
        button: GamepadButton::Start,
    });
    let report = advance(&mut scripts, &mut world, &sources, &input);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!((position(&world, entity)[0] - 1.0).abs() < 1e-5, "slot 1");

    input.begin_frame(std::time::Duration::from_millis(16));
    input.apply(InputEvent::GamepadAxisMoved {
        pad: second,
        axis: GamepadAxis::LeftX,
        value: -1.0,
    });
    input.apply(InputEvent::GamepadPressed {
        pad: second,
        button: GamepadButton::RightBumper,
    });
    advance(&mut scripts, &mut world, &sources, &input);
    let at = position(&world, entity);
    assert!(
        (at[0] - 3.0).abs() < 1e-5,
        "slot 2 joined the next frame: {at:?}"
    );
    assert!((at[1] + 1.0).abs() < 1e-5, "player 2's stick: {at:?}");
    assert!(at[2].abs() < 1e-5, "player 2's bumper is not player 1's");

    input.begin_frame(std::time::Duration::from_millis(16));
    input.apply(InputEvent::GamepadPressed {
        pad: first,
        button: GamepadButton::RightBumper,
    });
    advance(&mut scripts, &mut world, &sources, &input);
    assert!((position(&world, entity)[2] - 1.0).abs() < 1e-5);
}

#[test]
fn a_misspelled_button_or_a_fractional_slot_is_refused() {
    for script in [
        r#"script Reader { fn update(dt: f32) { if Gamepad.is_down(1.0, "a") { } } }"#,
        r#"script Reader { fn update(dt: f32) { if Gamepad.is_down(1.5, "south") { } } }"#,
        r#"script Reader { fn update(dt: f32) { this.transform.position.x = Gamepad.axis(9.0, "left_x"); } }"#,
    ] {
        let (mut world, _, sources) = world(script);
        let report = advance(
            &mut Scripts::new(),
            &mut world,
            &sources,
            &InputState::default(),
        );
        assert!(!report.failures.is_empty(), "{script} should fail");
    }
}
