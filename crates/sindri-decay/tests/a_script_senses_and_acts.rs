//! The scripting host: a script that can sense the frame it is in and act on
//! more than its own transform.
//!
//! Before this, a Decay script could move its own transform and nothing else,
//! which is a proof rather than a feature. The surface here is the one
//! `docs/2d-inventory.md` recorded from the legacy engine's `player.lua` — real
//! gameplay rather than an imagined sample — so these tests are that list.

use serde_json::{Value, json};
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::{InputEvent, InputState, Key};

const SPRITE: &str = "sindri.sprite";

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// One entity with a transform, a sprite, and a script.
fn world(script: &str) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [
            (
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "s.decay", "script": "S" }),
            ),
            (
                SPRITE.to_owned(),
                json!({
                    "texture": "procedural:checkerboard",
                    "tint": [1.0, 1.0, 1.0, 1.0],
                    "layer": 3
                }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("s.decay", script);
    (world, entity, sources)
}

/// A keyboard with some keys held down.
fn holding(keys: &[Key]) -> InputState {
    let mut input = InputState::default();
    for key in keys {
        input.apply(InputEvent::KeyPressed(*key));
    }
    input
}

fn position(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("the entity kept its transform")
        .position
}

fn sprite(world: &World, entity: EntityId) -> &Value {
    &world.get(entity).expect("the entity exists").components[SPRITE]
}

/// The heart of it: a held key moves the entity, and a different key moves it
/// the other way.
#[test]
fn a_script_reads_the_keyboard_and_moves() {
    let source = r#"
        script S {
            @export
            let speed: f32 = 4.0;

            fn update(dt: f32) {
                let movement = Input.axis("ArrowLeft", "ArrowRight");
                this.transform.position.x += movement * speed * dt;
            }
        }
    "#;

    for (keys, expected) in [
        (vec![Key::ArrowRight], 2.0),
        (vec![Key::ArrowLeft], -2.0),
        // Both held is zero, so opposed keys cannot cancel into whichever
        // arrived last.
        (vec![Key::ArrowLeft, Key::ArrowRight], 0.0),
        (vec![], 0.0),
    ] {
        let (mut world, entity, sources) = world(source);
        let report = Scripts::new().advance(
            &mut world,
            &registry(),
            ScriptFrame::new(&sources, &holding(&keys), 0.5),
        );
        assert!(report.is_quiet(), "{report:?}");
        assert!(
            (position(&world, entity)[0] - expected).abs() < 1.0e-5,
            "holding {keys:?} should have moved to {expected}, and it went to {}",
            position(&world, entity)[0]
        );
    }
}

/// The edge questions are distinct from the held one, and a script that jumps
/// on a press must not jump every frame the key is down.
#[test]
fn a_script_can_tell_a_press_from_a_hold() {
    let source = r#"
        script S {
            var jumps: f32 = 0.0;
            fn update(dt: f32) {
                if Input.just_pressed("Space") {
                    jumps += 1.0;
                }
                this.transform.position.y = jumps;
            }
        }
    "#;
    let (mut world, entity, sources) = world(source);
    let mut scripts = Scripts::new();

    let mut input = InputState::default();
    input.apply(InputEvent::KeyPressed(Key::Space));
    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 0.5),
    );
    assert!((position(&world, entity)[1] - 1.0).abs() < 1.0e-5);

    // The next frame: still held, but no longer newly pressed.
    input.begin_frame();
    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 0.5),
    );
    assert!(
        (position(&world, entity)[1] - 1.0).abs() < 1.0e-5,
        "holding the key is not pressing it again"
    );
    assert!(
        input.key_down(Key::Space),
        "and the key really is still down"
    );
}

/// A key name nobody answers to is refused rather than read as never-held. A
/// control that silently does nothing is a bug report nobody can reproduce.
#[test]
fn a_key_name_that_names_no_key_is_refused() {
    let (mut world, _, sources) =
        world(r#"script S { fn update(dt: f32) { if Input.is_down("Jump") { } } }"#);
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );
    assert!(
        format!("{:?}", report.failures).contains("no key called `Jump`"),
        "{report:?}"
    );
}

/// Time is the frame's, and elapsed accumulates per instance.
#[test]
fn a_script_can_ask_how_long_it_has_been_running() {
    let (mut world, entity, sources) = world(
        r"script S { fn update(dt: f32) { this.transform.position.x = Time.elapsed; this.transform.position.y = Time.delta; } }",
    );
    let mut scripts = Scripts::new();
    let input = InputState::default();

    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 0.25),
    );
    assert!((position(&world, entity)[0] - 0.25).abs() < 1.0e-5);
    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 0.25),
    );
    assert!(
        (position(&world, entity)[0] - 0.5).abs() < 1.0e-5,
        "elapsed accumulates"
    );
    assert!(
        (position(&world, entity)[1] - 0.25).abs() < 1.0e-5,
        "and delta is this frame, not the total"
    );
}

/// A script can change its sprite, which is the acceptance list's `set_tint`
/// and the proof that reaching a component generalises past the transform.
#[test]
fn a_script_can_change_its_sprite() {
    let (mut world, entity, sources) = world(
        r"script S { fn update(dt: f32) { this.sprite.tint.r = 0.25; this.sprite.layer = 7.0; } }",
    );
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );
    assert!(report.is_quiet(), "{report:?}");

    let sprite = sprite(&world, entity);
    assert_eq!(sprite["tint"], json!([0.25, 1.0, 1.0, 1.0]));
    // Still an integer, because the payload held one: a scene that round-trips
    // byte for byte must not gain a `.0` because a script touched a layer.
    assert_eq!(sprite["layer"], json!(7));
    assert!(sprite["layer"].is_i64(), "{sprite}");
    // And nothing the script did not name was disturbed.
    assert_eq!(sprite["texture"], json!("procedural:checkerboard"));
}

/// Reaching for a component the entity does not have says so, rather than
/// failing as an unknown path that looks like a typo.
#[test]
fn writing_a_sprite_an_entity_does_not_have_says_so() {
    let mut world = World::default();
    world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "s.decay", "script": "S" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "s.decay",
        r"script S { fn update(dt: f32) { this.sprite.tint.r = 0.5; } }",
    );

    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );
    assert!(
        format!("{:?}", report.failures).contains("has none"),
        "{report:?}"
    );
}

/// A script has no other way to say anything, and an author debugging one
/// needs it before they need anything else.
#[test]
fn a_script_can_print() {
    let (mut world, entity, sources) =
        world(r#"script S { fn update(dt: f32) { print("moving"); print(dt); print(true); } }"#);
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );

    assert!(report.failures.is_empty(), "{report:?}");
    let said: Vec<&str> = report
        .printed
        .iter()
        .map(|message| message.message.as_str())
        .collect();
    assert_eq!(said, ["moving", "0.5", "true"]);
    assert!(
        report
            .printed
            .iter()
            .all(|message| message.entity == entity),
        "and each line names the script that said it"
    );
}

/// The capability that justified a typed language: a panel can find out what a
/// script wants authored, and what each field starts as, without running it
/// against an entity.
#[test]
fn a_script_declares_what_it_wants_authored() {
    let (mut world, _, sources) = world(
        r#"
        script S {
            @export let speed: f32 = 6.0;
            @export let label: String = "player";
            @export var enabled: bool = true;
            // Not exported: instance state, which is nobody's business but the
            // script's, and must not appear in a property panel.
            var elapsed: f32 = 0.0;
            fn update(dt: f32) { elapsed += dt; }
        }
        "#,
    );
    let mut scripts = Scripts::new();
    // Compiled as a side effect of running, which is also how the editor gets
    // there: a script is compiled because the scene names it.
    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );

    let exports = scripts
        .exports("s.decay", "S")
        .expect("the script compiled");
    let named: Vec<(&str, Option<&str>)> = exports
        .iter()
        .map(|export| (export.name.as_str(), export.type_name.as_deref()))
        .collect();
    assert_eq!(
        named,
        [
            ("speed", Some("f32")),
            ("label", Some("String")),
            ("enabled", Some("bool")),
        ],
        "declaration order, and nothing that is not @export"
    );

    assert_eq!(exports[0].default, sindri_decay::ScriptValue::Number(6.0));
    assert_eq!(
        exports[1].default,
        sindri_decay::ScriptValue::String("player".to_owned())
    );
    assert_eq!(exports[2].default, sindri_decay::ScriptValue::Bool(true));
}

/// A source that has not compiled has no exports to report, which is different
/// from having none — a panel must be able to tell those apart.
#[test]
fn a_script_that_has_not_compiled_reports_nothing_rather_than_no_properties() {
    let scripts = Scripts::new();
    assert_eq!(scripts.exports("never-loaded.decay", "S"), None);
}

/// Two scripts cooperating, which is the smallest thing a game needs and the
/// one thing the language cannot do on its own: Decay has no value that holds
/// an entity, so a script cannot name another. It can leave a number under a
/// name, and another can read it.
#[test]
fn one_script_can_leave_a_number_for_another() {
    let mut world = World::default();
    let mut spawn = |script: &str| {
        world.spawn(EntityData {
            transform_3d: Some(Transform3D::default()),
            components: [(
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "s.decay", "script": script }),
            )]
            .into_iter()
            .collect(),
            ..EntityData::default()
        })
    };
    // Named so the writer runs first, since a world is queried in handle order
    // and a reader that ran first would see the fallback.
    let writer = spawn("Writer");
    let reader = spawn("Reader");

    let mut sources = ScriptSources::new();
    sources.insert(
        "s.decay",
        r#"
        script Writer {
            fn update(dt: f32) {
                this.transform.position.x = 7.0;
                Game.set("player_x", this.transform.position.x);
            }
        }
        script Reader {
            fn update(dt: f32) {
                this.transform.position.x = Game.get("player_x", -1.0);
            }
        }
        "#,
    );

    let mut scripts = Scripts::new();
    let report = scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );
    assert!(report.is_quiet(), "{report:?}");
    assert!((position(&world, writer)[0] - 7.0).abs() < 1.0e-5);
    assert!(
        (position(&world, reader)[0] - 7.0).abs() < 1.0e-5,
        "the reader saw what the writer left"
    );
}

/// A note nobody has left reads as the fallback the script named, not as zero.
/// A `get` that silently answered zero would make a typo look like a value.
#[test]
fn an_unwritten_note_reads_as_the_fallback() {
    let (mut world, entity, sources) = world(
        r#"script S { fn update(dt: f32) { this.transform.position.x = Game.get("nobody", 42.0); } }"#,
    );
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 0.5),
    );
    assert!(report.is_quiet(), "{report:?}");
    assert!((position(&world, entity)[0] - 42.0).abs() < 1.0e-5);
}

/// The board is what a run counted, so it belongs to the run. Stopping and
/// starting again must not begin with the last game's score.
#[test]
fn the_board_is_cleared_with_the_instances() {
    let (mut world, _, sources) = world(
        r#"script S { fn update(dt: f32) { Game.set("score", Game.get("score", 0.0) + 1.0); } }"#,
    );
    let mut scripts = Scripts::new();
    let input = InputState::default();
    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 0.5),
    );
    scripts.advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &input, 0.5),
    );
    assert!((scripts.blackboard().get("score", 0.0) - 2.0).abs() < 1.0e-9);

    scripts.clear();
    assert!(!scripts.blackboard().has("score"), "a new run starts fresh");
}
