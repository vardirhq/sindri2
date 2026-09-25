//! Select in the lobby adds a bot, which readies itself and plays: it scores
//! against a player who stands still, and two bots play each other.

use scorchball::Run;
use sindri_platform::GamepadButton;

const STEP: f32 = 1.0 / 60.0;
const PAD: u32 = 10;

fn steps(run: &mut Run, count: usize) {
    for _ in 0..count {
        let notes = run.step(STEP);
        assert!(notes.is_empty(), "the game reported: {notes:?}");
    }
}

fn tap(run: &mut Run, button: GamepadButton) {
    run.button(PAD, button, true);
    steps(run, 1);
    run.button(PAD, button, false);
    steps(run, 1);
}

fn opened() -> Run {
    let mut run = Run::open().expect("the project opens");
    steps(&mut run, 5);
    run.connect(PAD);
    run
}

/// Plays until someone scores or the time runs out, and says who.
fn play(run: &mut Run, seconds: usize) -> (f32, f32) {
    for _ in 0..seconds * 60 {
        steps(run, 1);
        let score = (run.board("score_blue"), run.board("score_red"));
        if score.0 + score.1 > 0.0 {
            return score;
        }
    }
    (run.board("score_blue"), run.board("score_red"))
}

#[test]
fn a_bot_joins_the_other_side_readies_and_scores_on_a_player_who_stands_still() {
    let mut run = opened();
    tap(&mut run, GamepadButton::South);
    tap(&mut run, GamepadButton::Select);
    assert_eq!(run.tagged("player").len(), 2, "a person and a bot");
    tap(&mut run, GamepadButton::North);
    steps(&mut run, 60 * 4);
    assert!(
        (run.board("phase") - 2.0).abs() < f32::EPSILON,
        "the bot readied itself, so it kicked off"
    );
    let (blue, red) = play(&mut run, 60);
    assert!(red > 0.0, "the bot, on Red, scored: {blue} to {red}");
}

#[test]
fn two_bots_play_each_other_and_someone_scores() {
    let mut run = opened();
    tap(&mut run, GamepadButton::Select);
    tap(&mut run, GamepadButton::Select);
    tap(&mut run, GamepadButton::Select);
    assert_eq!(run.tagged("player").len(), 2, "one bot a side");
    steps(&mut run, 60 * 4);
    assert!((run.board("phase") - 2.0).abs() < f32::EPSILON);
    let (blue, red) = play(&mut run, 120);
    assert!(blue + red > 0.0, "a goal in two minutes");
}
