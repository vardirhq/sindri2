//! A script can ask to be somewhere else.
//!
//! The change cannot happen inside the call. The script asking is running *in*
//! the scene being left, from a world the change would rearrange underneath it,
//! so the call records an intention and the host performs it between frames —
//! the same shape as `Audio.play` recording a sound for whoever owns a speaker.

use serde_json::json;
use sindri_core::{ComponentSchemaRegistry, EntityData, SceneComponent, World};
use sindri_decay::{SceneChannel, ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// Runs one frame, with or without a scene channel.
fn run(source: &str, scenes: Option<&mut SceneChannel>) -> sindri_decay::ScriptReport {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Door".to_owned()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "door.decay", "script": "Door" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("door.decay", source);
    let input = InputState::default();
    let mut frame = ScriptFrame::new(&sources, &input, 1.0 / 60.0);
    if let Some(scenes) = scenes {
        frame = frame.with_scenes(scenes);
    }
    Scripts::new().advance(&mut world, &registry(), frame)
}

const GOES: &str = r#"
script Door {
    fn update(dt: f32) {
        Scene.go("house");
    }
}
"#;

#[test]
fn a_request_is_recorded_for_the_host_to_perform() {
    let mut scenes = SceneChannel::playing("farm");
    let report = run(GOES, Some(&mut scenes));
    assert!(report.failures.is_empty(), "{report:#?}");
    assert_eq!(scenes.take(), Some("house".to_owned()));
    assert_eq!(
        scenes.take(),
        None,
        "and taking it leaves the channel clear"
    );
}

/// Nothing about the world changes during the frame. The move is the host's to
/// make, once no script is mid-call inside the scene being left.
#[test]
fn asking_changes_nothing_yet() {
    let mut scenes = SceneChannel::playing("farm");
    run(GOES, Some(&mut scenes));
    assert_eq!(
        scenes.playing, "farm",
        "the frame is still being played in the scene it started in"
    );
}

/// Two scripts asking in one frame is a conflict with no right answer. First
/// asked wins, so a door beside a door does not depend on which script the pass
/// happened to reach first.
#[test]
fn the_first_request_in_a_frame_is_the_one_that_counts() {
    let mut world = World::default();
    for (name, target) in [("DoorA", "house"), ("DoorB", "barn")] {
        world.spawn(EntityData {
            name: Some(name.to_owned()),
            components: [(
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({
                    "source": "door.decay",
                    "script": "Door",
                    "properties": { "target": target },
                }),
            )]
            .into_iter()
            .collect(),
            ..EntityData::default()
        });
    }
    let mut sources = ScriptSources::new();
    sources.insert(
        "door.decay",
        r#"
        script Door {
            @export let target: String = "";
            fn update(dt: f32) { Scene.go(this.target); }
        }
        "#,
    );
    let mut scenes = SceneChannel::playing("farm");
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0).with_scenes(&mut scenes),
    );
    assert!(report.failures.is_empty(), "{report:#?}");
    assert_eq!(
        scenes.take(),
        Some("house".to_owned()),
        "the first asked, not the last"
    );
}

/// A host that plays exactly one scene says so, rather than accepting a request
/// nothing will ever perform. A game whose doors silently never open should be
/// heard about on the first frame.
#[test]
fn a_host_that_cannot_change_scene_says_so() {
    let report = run(GOES, None);
    assert!(
        !report.failures.is_empty(),
        "asking a host with no scenes should be reported"
    );
}

#[test]
fn a_script_can_read_the_scene_it_is_in() {
    let mut scenes = SceneChannel::playing("farm");
    let report = run(
        r#"
        script Door {
            fn update(dt: f32) {
                if Scene.current() == "farm" { print("outside"); }
            }
        }
        "#,
        Some(&mut scenes),
    );
    assert!(report.failures.is_empty(), "{report:#?}");
    assert_eq!(
        report.printed.len(),
        1,
        "the script should have recognised where it was: {report:#?}"
    );
}

/// Reading back its own request would show a script the move happening a frame
/// before it did.
#[test]
fn reading_the_scene_does_not_see_a_pending_request() {
    let mut scenes = SceneChannel::playing("farm");
    let report = run(
        r#"
        script Door {
            fn update(dt: f32) {
                Scene.go("house");
                if Scene.current() == "farm" { print("still outside"); }
            }
        }
        "#,
        Some(&mut scenes),
    );
    assert!(report.failures.is_empty(), "{report:#?}");
    assert_eq!(report.printed.len(), 1, "{report:#?}");
}
