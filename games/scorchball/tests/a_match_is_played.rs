//! Scorchball played by a test with two pads: they join, ready up, kick off,
//! and Blue walks the ball into Red's goal.

use scorchball::Run;
use sindri_core::EntityId;
use sindri_platform::{GamepadAxis, GamepadButton};

const STEP: f32 = 1.0 / 60.0;
const BLUE: u32 = 10;
const RED: u32 = 20;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "the game reported: {notes:?}");
}

fn steps(run: &mut Run, count: usize) {
    for _ in 0..count {
        step(run);
    }
}

/// A press and a release, a frame apart, as a thumb does it.
fn tap(run: &mut Run, pad: u32, button: GamepadButton) {
    run.button(pad, button, true);
    step(run);
    run.button(pad, button, false);
    step(run);
}

fn at_centre([x, y]: [f32; 2]) -> bool {
    x.hypot(y) < 1e-4
}

fn slot_of(run: &Run, player: EntityId) -> f32 {
    run.world
        .get(player)
        .and_then(|data| data.components.get("sindri.script"))
        .and_then(|script| script["properties"]["slot"].as_f64())
        .map_or(0.0, |slot| {
            #[allow(clippy::cast_possible_truncation)]
            let slot = slot as f32;
            slot
        })
}

fn player(run: &Run, slot: f32) -> EntityId {
    run.tagged("player")
        .into_iter()
        .find(|player| (slot_of(run, *player) - slot).abs() < f32::EPSILON)
        .expect("that player is on the pitch")
}

/// Two pads join and ready up, and the countdown runs out.
fn kicked_off() -> Run {
    let mut run = Run::open().expect("the project opens");
    steps(&mut run, 5);
    run.connect(BLUE);
    run.connect(RED);
    tap(&mut run, BLUE, GamepadButton::South);
    tap(&mut run, RED, GamepadButton::South);
    tap(&mut run, BLUE, GamepadButton::North);
    tap(&mut run, RED, GamepadButton::North);
    steps(&mut run, 10);
    assert!(
        (run.board("phase") - 1.0).abs() < f32::EPSILON,
        "counting down"
    );
    steps(&mut run, 60 * 4);
    assert!(
        (run.board("phase") - 2.0).abs() < f32::EPSILON,
        "kicked off"
    );
    run
}

/// Steers a pad's player so its feet reach a point, a frame at a time.
fn walk_to(run: &mut Run, pad: u32, slot: f32, target: [f32; 2], frames: usize) -> bool {
    for _ in 0..frames {
        let at = run.position(player(run, slot));
        let feet = [at[0], at[1] - 1.1];
        let (dx, dy) = (target[0] - feet[0], target[1] - feet[1]);
        let length = dx.hypot(dy);
        if length < 0.3 {
            run.axis(pad, GamepadAxis::LeftX, 0.0);
            run.axis(pad, GamepadAxis::LeftY, 0.0);
            return true;
        }
        // The pad measures down as positive; the pitch measures up.
        run.axis(pad, GamepadAxis::LeftX, dx / length);
        run.axis(pad, GamepadAxis::LeftY, -dy / length);
        step(run);
    }
    run.axis(pad, GamepadAxis::LeftX, 0.0);
    run.axis(pad, GamepadAxis::LeftY, 0.0);
    false
}

#[test]
fn nobody_plays_until_a_pad_joins() {
    let mut run = Run::open().expect("the project opens");
    steps(&mut run, 60);
    assert!(run.tagged("player").is_empty());
    assert!(
        run.board("phase").abs() < f32::EPSILON,
        "waiting in the lobby"
    );
    let ball = run.entity("ball").expect("the ball");
    assert!(at_centre(run.position(ball)), "on the centre spot");
}

#[test]
fn pads_join_on_their_own_sides_and_one_ready_player_is_not_a_match() {
    let mut run = Run::open().expect("the project opens");
    run.connect(BLUE);
    run.connect(RED);
    tap(&mut run, RED, GamepadButton::Start);
    tap(&mut run, BLUE, GamepadButton::South);
    // Slots follow the order pads pressed in, whichever pad it is.
    let first = player(&run, 1.0);
    let second = player(&run, 2.0);
    assert!(
        run.position(first)[0] < 0.0,
        "slot 1 plays for Blue, on the left"
    );
    assert!(
        run.position(second)[0] > 0.0,
        "slot 2 plays for Red, on the right"
    );
    // Unready players stand tall in the lobby.
    steps(&mut run, 60);
    assert!(run.scale(first) > 5.0 && run.scale(second) > 5.0);

    // The second pad to press is slot 2, so this readies the Red player.
    tap(&mut run, BLUE, GamepadButton::North);
    steps(&mut run, 60);
    assert!(run.board("phase").abs() < f32::EPSILON, "one is not enough");
    assert!(run.scale(second) < 4.0, "a ready player shrinks to size");
    assert!(run.scale(first) > 5.0, "the other is still waiting");
}

#[test]
fn blue_takes_the_ball_and_scores_in_reds_goal() {
    let mut run = kicked_off();
    // Red steps out of the lane: touching a held ball steals it.
    assert!(
        walk_to(&mut run, RED, 2.0, [7.0, -5.0], 60 * 3),
        "Red stood aside"
    );
    assert!(
        walk_to(&mut run, BLUE, 1.0, [0.0, 0.0], 60 * 4),
        "reached the ball"
    );
    steps(&mut run, 2);
    assert!(
        (run.board("holder") - 1.0).abs() < f32::EPSILON,
        "Blue has it"
    );

    // Dribble towards Red's goal, then kick it in.
    run.axis(BLUE, GamepadAxis::LeftX, 1.0);
    let ball = run.entity("ball").expect("the ball");
    for _ in 0..60 * 3 {
        step(&mut run);
        if run.position(ball)[0] > 9.0 {
            break;
        }
    }
    run.axis(BLUE, GamepadAxis::LeftX, 0.0);
    assert!(run.position(ball)[0] > 9.0, "dribbled up the pitch");
    run.axis(BLUE, GamepadAxis::RightX, 1.0);
    tap(&mut run, BLUE, GamepadButton::RightBumper);
    for _ in 0..60 * 2 {
        step(&mut run);
        if run.board("score_blue") > 0.0 {
            break;
        }
    }
    assert!(
        (run.board("score_blue") - 1.0).abs() < f32::EPSILON,
        "a goal for Blue"
    );
    assert!(run.board("score_red").abs() < f32::EPSILON);
    step(&mut run);
    assert!(at_centre(run.position(ball)), "back on the centre spot");
}

#[test]
fn unplugging_a_pad_takes_its_player_off() {
    let mut run = kicked_off();
    assert_eq!(run.tagged("player").len(), 2);
    run.disconnect(RED);
    steps(&mut run, 2);
    assert_eq!(run.tagged("player").len(), 1);
    assert!((slot_of(&run, run.tagged("player")[0]) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_sign_drops_from_the_sky_and_cannot_be_taken_until_it_lands() {
    let mut run = kicked_off();
    let mut sign = None;
    for _ in 0..60 * 16 {
        step(&mut run);
        sign = run.tagged("powerup").first().copied();
        if sign.is_some() {
            break;
        }
    }
    let sign = sign.expect("a sign dropped");
    // Blue waits exactly where it will land, and it is only taken once there.
    let at = run.position(sign);
    run.world
        .get_mut(player(&run, 1.0))
        .and_then(|data| data.transform_3d.as_mut())
        .expect("Blue")
        .position = [at[0], at[1] + 1.1, 1.0];
    step(&mut run);
    assert!(run.world.get(sign).is_some(), "still falling");
    steps(&mut run, 60 * 2);
    assert!(run.world.get(sign).is_none(), "taken once it landed");
}

#[test]
fn power_ups_appear_during_play() {
    let mut run = kicked_off();
    for _ in 0..60 * 16 {
        step(&mut run);
        if !run.tagged("powerup").is_empty() {
            return;
        }
    }
    panic!("no power-up appeared within the longest wait");
}

/// The kind a sign was made with, as the match authored it.
fn kind_of(run: &Run, sign: EntityId) -> u32 {
    run.world
        .get(sign)
        .and_then(|data| data.components.get("sindri.script"))
        .and_then(|script| script["properties"]["kind"].as_f64())
        .map_or(0, |kind| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let kind = kind as u32;
            kind
        })
}

/// Blue walks into sign after sign until it has had every power, and each
/// does what it says.
#[test]
fn every_power_does_what_it_says() {
    let mut run = kicked_off();
    // Red waits out of the way, so Blue is the only one collecting.
    assert!(walk_to(&mut run, RED, 2.0, [12.0, -5.5], 60 * 3));
    let mut seen = [false; 5];
    for _ in 0..16 {
        let mut sign = None;
        for _ in 0..60 * 16 {
            step(&mut run);
            if let Some(found) = run.tagged("powerup").first() {
                sign = Some(*found);
                break;
            }
        }
        let sign = sign.expect("a sign dropped");
        let kind = kind_of(&run, sign);
        // It is taken at the player's feet, once it has landed on its shadow.
        steps(&mut run, 60);
        let at = run.position(sign);
        walk_to(&mut run, BLUE, 1.0, at, 60 * 4);
        steps(&mut run, 3);
        assert!(run.world.get(sign).is_none(), "the sign was taken");
        steps(&mut run, 60);
        let blue = player(&run, 1.0);
        match kind {
            1 => assert!(run.scale(blue) > 4.5, "the Enlarger grew Blue"),
            3 => assert!(run.board("wind") > 0.0, "Blue's wind blows towards Red"),
            4 => assert!(run.effects.live() > 0, "the ball is burning"),
            _ => {}
        }
        seen[kind as usize] = true;
        if seen[1..].iter().all(|seen| *seen) {
            return;
        }
    }
    panic!("not every power came up: {seen:?}");
}

/// A burning ball sets alight an opponent of the player who lit it, and they
/// run wild until it burns out.
#[test]
fn a_fireball_sets_an_opponent_running_wild() {
    let mut run = kicked_off();
    // Collect until Blue has lit the ball.
    for _ in 0..16 {
        let mut sign = None;
        for _ in 0..60 * 16 {
            step(&mut run);
            sign = run.tagged("powerup").first().copied();
            if sign.is_some() {
                break;
            }
        }
        let sign = sign.expect("a sign dropped");
        let kind = kind_of(&run, sign);
        steps(&mut run, 60);
        let at = run.position(sign);
        walk_to(&mut run, BLUE, 1.0, at, 60 * 4);
        steps(&mut run, 3);
        if kind == 4 {
            break;
        }
    }
    assert!(run.effects.live() > 0, "the ball is burning");
    // Red walks into the burning ball, and is set alight: with its stick at
    // rest it keeps moving, which is running wild.
    let ball = run.entity("ball").expect("the ball");
    let at = run.position(ball);
    walk_to(&mut run, RED, 2.0, at, 60 * 4);
    steps(&mut run, 20);
    let red = player(&run, 2.0);
    let before = run.position(red);
    steps(&mut run, 60);
    let after = run.position(red);
    let moved = (after[0] - before[0]).hypot(after[1] - before[1]);
    assert!(
        moved > 2.0,
        "Red ran wild with its stick at rest: moved {moved}"
    );
    // Five seconds later it is out, and Red stands still again.
    steps(&mut run, 60 * 5);
    let settled = run.position(red);
    steps(&mut run, 30);
    let still = run.position(red);
    assert!(
        (still[0] - settled[0]).hypot(still[1] - settled[1]) < 0.2,
        "the fire went out"
    );
}
