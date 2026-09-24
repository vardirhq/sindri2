//! The platformer played by a test: it stands on the ground it was painted
//! on, and a player running right and jumping at each gap and wall reaches
//! the flag.

use platformer::Run;
use sindri_platform::Key;
use sindri_scene::TilemapComponent;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "the game reported: {notes:?}");
}

/// The level's tiles, read from the scene as a player would see them.
fn level(run: &Run) -> (TilemapComponent, [f32; 2]) {
    let entity = run.entity("level").expect("the level");
    let tilemap = run
        .components
        .get::<TilemapComponent>(&run.world, entity)
        .expect("a readable tilemap")
        .expect("the level is a tilemap");
    (tilemap, run.position(entity))
}

/// The height of the ground at a column, or `None` over a pit. Grass tufts
/// are decoration and do not count.
fn ground_at(tilemap: &TilemapComponent, corner: [f32; 2], column: i32) -> Option<f32> {
    let column = u32::try_from(column).ok()?;
    (0..tilemap.rows).find_map(|row| {
        let tile = tilemap.tile(column, row)?;
        let solid = tilemap.palette[tile as usize] != "tuft";
        #[allow(clippy::cast_precision_loss)]
        solid.then(|| corner[1] - row as f32)
    })
}

#[test]
fn the_hero_stands_on_the_painted_ground() {
    let mut run = Run::open().expect("the project opens");
    let hero = run.entity("hero").expect("the hero");
    for _ in 0..90 {
        step(&mut run);
    }
    let [x, y] = run.position(hero);
    // The first field's surface is at y = 3, and the hero's feet are half a
    // unit below its middle.
    assert!((y - 3.5).abs() < 0.08, "standing on the grass at {x}, {y}");
    assert!((run.board("coins_total") - 10.0).abs() < f32::EPSILON);
}

#[test]
fn a_player_running_and_jumping_reaches_the_flag() {
    let mut run = Run::open().expect("the project opens");
    let hero = run.entity("hero").expect("the hero");
    let (tilemap, corner) = level(&run);
    run.key(Key::ArrowRight, true);
    let mut holding_jump = 0;
    for frame in 0..60 * 40 {
        step(&mut run);
        if run.board("won") > 0.0 {
            assert!(run.board("falls") < 1.0, "it never fell");
            assert!(run.board("coins") >= 3.0, "and picked up coins on the way");
            eprintln!("reached the flag after {:.1} s", f64::from(frame) / 60.0);
            return;
        }
        if holding_jump > 0 {
            holding_jump -= 1;
            if holding_jump == 0 {
                run.key(Key::Space, false);
            }
            continue;
        }
        let [x, y] = run.position(hero);
        let settled = run
            .physics
            .world()
            .linear_velocity(hero)
            .is_ok_and(|velocity| velocity[1].abs() < 0.05);
        #[allow(clippy::cast_possible_truncation)]
        let column = (x - corner[0]).floor() as i32;
        let feet = y - 0.5;
        let underfoot = ground_at(&tilemap, corner, column);
        let blocked = (1..=2).any(|ahead| {
            ground_at(&tilemap, corner, column + ahead).is_none_or(|top| {
                top > feet + 0.5 || underfoot.is_some_and(|under| top < under - 0.5 && ahead == 1)
            })
        });
        if settled && blocked {
            run.key(Key::Space, true);
            holding_jump = 40;
        }
    }
    panic!(
        "the flag was not reached; the hero is at {:?} with {} coins",
        run.position(hero),
        run.board("coins")
    );
}

/// The camera follows the hero along the level, and stays inside its bounds.
#[test]
fn the_camera_follows_the_hero() {
    let mut run = Run::open().expect("the project opens");
    let camera = run.entity("camera").expect("the camera");
    let hero = run.entity("hero").expect("the hero");
    run.key(Key::ArrowRight, true);
    let mut furthest = 0.0_f32;
    for _ in 0..90 {
        step(&mut run);
        let [x, y] = run.position(camera);
        furthest = furthest.max(x);
        assert!(
            y >= 4.5 - 1.0e-3 && x >= 8.0 - 1.0e-3,
            "inside its bounds: {x}, {y}"
        );
    }
    let hero_x = run.position(hero)[0];
    assert!(
        furthest > 10.0 && (furthest - hero_x).abs() < 2.0,
        "it followed the hero to {hero_x}: {furthest}"
    );
}
