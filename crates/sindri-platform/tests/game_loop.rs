//! End-to-end coverage for the host loop, driven entirely by a manual clock.
//!
//! No window, no GPU, and no sleeping: the same code a desktop or browser host
//! runs is exercised here by feeding it exact frame deltas and input events.

use std::time::Duration;

use sindri_core::{EntityData, EntityId, FixedStepConfig, TimeScale, Transform3D};
use sindri_platform::{
    EngineHost, FrameContext, FramePhase, FrameTimer, Game, HostError, InputEvent, Key, ManualClock,
};
use thiserror::Error;

const STEP: Duration = Duration::from_millis(10);
/// Where the player sits along Z, which nothing in this test should move.
const PLAYER_DEPTH: f32 = -3.5;

const SPEED: f32 = 100.0;

fn config() -> FixedStepConfig {
    FixedStepConfig {
        step: STEP,
        max_frame_delta: Duration::from_millis(250),
        max_steps_per_frame: 8,
    }
}

#[derive(Debug, Error, PartialEq)]
enum PlayerError {
    #[error("the player entity is missing")]
    MissingEntity,
    #[error("the player was asked to fail")]
    Requested,
}

/// A minimal game: one entity, moved by the arrow keys at a fixed rate.
#[derive(Debug, Default)]
struct Player {
    entity: Option<EntityId>,
    fixed_updates: u32,
    updates: u32,
    started: bool,
    stopped: bool,
    fail_on_update: bool,
}

impl Player {
    fn position(&self, host: &EngineHost<Self>) -> [f32; 3] {
        host.world()
            .get(self.entity.expect("the player spawned"))
            .and_then(|data| data.transform_3d)
            .expect("the player has a transform")
            .position
    }
}

impl Game for Player {
    type Error = PlayerError;

    fn start(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.started = true;
        self.entity = Some(context.world.spawn(EntityData {
            name: Some("Player".into()),
            transform_3d: Some(Transform3D {
                // Off the play plane on purpose: gameplay that moves in two
                // dimensions must not quietly drag the third one with it.
                position: [0.0, 0.0, PLAYER_DEPTH],
                ..Transform3D::default()
            }),
            ..EntityData::default()
        }));
        Ok(())
    }

    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.fixed_updates += 1;
        let entity = self.entity.ok_or(PlayerError::MissingEntity)?;
        let horizontal = context.input.axis(Key::ArrowLeft, Key::ArrowRight);
        let vertical = context.input.axis(Key::ArrowDown, Key::ArrowUp);
        let seconds = context.time.delta.as_secs_f32();

        let data = context
            .world
            .get_mut(entity)
            .ok_or(PlayerError::MissingEntity)?;
        let mut transform = data.transform_3d.unwrap_or_default();
        transform.position[0] += horizontal * SPEED * seconds;
        transform.position[1] += vertical * SPEED * seconds;
        data.transform_3d = Some(transform);
        Ok(())
    }

    fn update(&mut self, _context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.updates += 1;
        if self.fail_on_update {
            return Err(PlayerError::Requested);
        }
        Ok(())
    }

    fn stop(&mut self, _context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        self.stopped = true;
        Ok(())
    }
}

/// Runs `frames` frames of exactly `frame` length through a manual clock.
fn run(
    host: &mut EngineHost<Player>,
    clock: &mut ManualClock,
    timer: &mut FrameTimer,
    frame: Duration,
    frames: u32,
) {
    for _ in 0..frames {
        clock.advance(frame);
        let delta = timer.tick(clock);
        host.advance(delta).expect("the frame advances");
    }
}

fn started_host() -> (EngineHost<Player>, ManualClock, FrameTimer) {
    let mut host = EngineHost::new(Player::default(), config()).expect("the host starts");
    host.start().expect("the game starts");
    let clock = ManualClock::new();
    let mut timer = FrameTimer::new();
    timer.tick(&clock);
    (host, clock, timer)
}

#[test]
fn a_game_spawns_on_start_and_stops_cleanly() {
    let (mut host, _, _) = started_host();
    assert!(host.game().started);
    assert_eq!(host.world().len(), 1);

    host.stop().expect("the game stops");
    assert!(host.game().stopped);
}

#[test]
fn the_keyboard_moves_an_entity() {
    let (mut host, mut clock, mut timer) = started_host();
    host.queue_input(InputEvent::KeyPressed(Key::ArrowRight));

    // One simulated second at 50 frames per second.
    run(
        &mut host,
        &mut clock,
        &mut timer,
        Duration::from_millis(20),
        50,
    );

    let player = host.game().position(&host);
    assert!(
        (player[0] - SPEED).abs() < 0.01,
        "expected roughly {SPEED} units of travel, got {}",
        player[0]
    );
    assert!(player[1].abs() < f32::EPSILON);
    assert!(
        (player[2] - PLAYER_DEPTH).abs() < f32::EPSILON,
        "moving in two dimensions must leave the third alone, but depth became {}",
        player[2]
    );
}

/// The property that makes fixed-step simulation worth having.
#[test]
fn simulation_is_frame_rate_independent() {
    let mut positions = Vec::new();
    let mut step_counts = Vec::new();

    for (frame, frames) in [
        (Duration::from_millis(20), 50),
        (Duration::from_millis(5), 200),
    ] {
        let (mut host, mut clock, mut timer) = started_host();
        host.queue_input(InputEvent::KeyPressed(Key::ArrowRight));
        run(&mut host, &mut clock, &mut timer, frame, frames);
        positions.push(host.game().position(&host)[0]);
        step_counts.push(host.game().fixed_updates);
    }

    // A simulated second is a simulated second at any frame rate.
    assert_eq!(step_counts[0], step_counts[1]);
    assert!(
        (positions[0] - positions[1]).abs() < 0.01,
        "50fps travelled {} but 200fps travelled {}",
        positions[0],
        positions[1]
    );
}

#[test]
fn an_input_edge_is_visible_for_exactly_one_frame() {
    let (mut host, mut clock, mut timer) = started_host();
    host.queue_input(InputEvent::KeyPressed(Key::Space));
    assert!(host.input().key_pressed(Key::Space));

    clock.advance(Duration::from_millis(20));
    let delta = timer.tick(&clock);
    host.advance(delta).expect("the frame advances");

    assert!(
        !host.input().key_pressed(Key::Space),
        "the edge should be consumed"
    );
    assert!(host.input().key_down(Key::Space), "the key is still held");
}

#[test]
fn losing_focus_releases_held_keys() {
    let (mut host, _, _) = started_host();
    host.queue_input(InputEvent::KeyPressed(Key::ArrowRight));
    assert!(host.input().key_down(Key::ArrowRight));

    // Key-up never arrives while unfocused, so the key would otherwise stick.
    host.queue_input(InputEvent::FocusChanged(false));
    assert!(!host.input().key_down(Key::ArrowRight));
    assert!(host.input().key_released(Key::ArrowRight));
    assert!(!host.input().is_focused());
}

#[test]
fn a_held_key_repeat_does_not_re_fire_the_press_edge() {
    let (mut host, _, _) = started_host();
    host.queue_input(InputEvent::KeyPressed(Key::A));
    assert!(host.input().key_pressed(Key::A));
    host.queue_input(InputEvent::KeyPressed(Key::A));
    assert!(host.input().key_pressed(Key::A));
    host.queue_input(InputEvent::KeyReleased(Key::A));
    assert!(!host.input().key_down(Key::A));
}

#[test]
fn time_scale_slows_the_simulation_but_not_the_frame() {
    let (mut host, mut clock, mut timer) = started_host();
    host.engine_mut()
        .set_time_scale(TimeScale::new(1, 2).expect("a valid scale"));
    host.queue_input(InputEvent::KeyPressed(Key::ArrowRight));

    run(
        &mut host,
        &mut clock,
        &mut timer,
        Duration::from_millis(20),
        50,
    );

    let travelled = host.game().position(&host)[0];
    assert!(
        (travelled - SPEED / 2.0).abs() < 0.01,
        "half speed should travel {} units, got {travelled}",
        SPEED / 2.0
    );
}

#[test]
fn a_frozen_time_scale_still_delivers_frames() {
    let (mut host, mut clock, mut timer) = started_host();
    host.engine_mut().set_time_scale(TimeScale::FROZEN);
    host.queue_input(InputEvent::KeyPressed(Key::ArrowRight));

    run(
        &mut host,
        &mut clock,
        &mut timer,
        Duration::from_millis(20),
        50,
    );

    assert_eq!(host.game().fixed_updates, 0);
    assert_eq!(host.game().updates, 50, "frames keep arriving while frozen");
    let resting = host.game().position(&host);
    assert!(resting[0].abs() < f32::EPSILON && resting[1].abs() < f32::EPSILON);
    assert!((resting[2] - PLAYER_DEPTH).abs() < f32::EPSILON);
}

#[test]
fn gameplay_failures_name_the_phase_they_came_from() {
    let (mut host, mut clock, mut timer) = started_host();
    host.game_mut().fail_on_update = true;

    clock.advance(Duration::from_millis(20));
    let delta = timer.tick(&clock);
    let error = host.advance(delta).expect_err("the update fails");

    match error {
        HostError::Game { phase, source } => {
            assert_eq!(phase, FramePhase::Update);
            assert_eq!(source, PlayerError::Requested);
        }
        other @ HostError::Engine(_) => panic!("expected a gameplay failure, got {other:?}"),
    }
}

#[test]
fn a_failed_frame_still_consumes_its_input_edges() {
    let (mut host, mut clock, mut timer) = started_host();
    host.game_mut().fail_on_update = true;
    host.queue_input(InputEvent::KeyPressed(Key::Space));

    clock.advance(Duration::from_millis(20));
    let delta = timer.tick(&clock);
    host.advance(delta).expect_err("the update fails");

    // Otherwise the next frame would see the same press a second time.
    assert!(!host.input().key_pressed(Key::Space));
}

#[test]
fn a_long_stall_cannot_spiral_the_simulation() {
    let (mut host, mut clock, mut timer) = started_host();
    host.queue_input(InputEvent::KeyPressed(Key::ArrowRight));

    clock.advance(Duration::from_secs(30));
    let delta = timer.tick(&clock);
    host.advance(delta).expect("the frame advances");

    // 30s of debugger pause must not become 3000 catch-up steps.
    assert_eq!(host.game().fixed_updates, config().max_steps_per_frame);
}
