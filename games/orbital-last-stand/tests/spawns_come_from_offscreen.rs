//! Nothing the campaign spawns appears in front of the player.
//!
//! Enemies used to be placed on a circle of radius 7.5 around the origin,
//! which `it_fits_a_phone` requires the camera to frame whole — so on every
//! supported screen some part of that ring was visible and enemies arrived out
//! of nothing in the middle of the arena. Placement follows the viewport now,
//! and this is the check that says so on each screen shape rather than on the
//! one the author happened to be looking at.
//!
//! Drops are deliberately exempt: a core or a pickup is *supposed* to appear
//! where the thing that dropped it died.

use orbital_last_stand::Run;
use sindri_core::{EntityId, TagsComponent};

const STEP: f32 = 1.0 / 60.0;

/// The scene camera's `sindri.camera.vertical_size`, which every gameplay
/// script carries as its own `view_size` export.
const VIEW_SIZE: f32 = 11.0;

/// Every viewport worth being sure about, widest to narrowest.
const SCREENS: [(&str, f32, f32); 5] = [
    ("desktop 16:9", 1920.0, 1080.0),
    ("laptop 16:10", 1440.0, 900.0),
    ("tablet portrait", 1536.0, 2048.0),
    ("phone portrait", 1080.0, 2400.0),
    ("phone landscape", 2400.0, 1080.0),
];

/// What the camera frames, worked out the way the extractor does: the authored
/// size lands on the shorter axis.
fn visible_half(width: f32, height: f32) -> (f32, f32) {
    let aspect = width / height;
    let half = VIEW_SIZE * 0.5;
    if aspect >= 1.0 {
        (half * aspect, half)
    } else {
        (half, half / aspect)
    }
}

fn tagged(run: &Run, tag: &str) -> Vec<EntityId> {
    run.world
        .entities()
        .filter_map(|(entity, _)| {
            run.components
                .get::<TagsComponent>(&run.world, entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has(tag))
                .then_some(entity)
        })
        .collect()
}

fn position(run: &Run, entity: EntityId) -> [f32; 3] {
    run.world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("a spawned enemy has a transform")
        .position
}

/// Every enemy the director places arrives outside the frame, on every screen.
#[test]
fn enemies_arrive_from_outside_the_frame() {
    for (name, width, height) in SCREENS {
        let (half_x, half_y) = visible_half(width, height);

        let mut run = Run::open().expect("the project opens");
        run.viewport = (width, height);
        for _ in 0..6 {
            let notes = run.step(STEP);
            assert!(notes.is_empty(), "{name}: {notes:#?}");
        }
        run.click("TitleStart");
        // A hull that survives the whole sample, so the run does not end early
        // and stop spawning.
        run.set_board("hp", 10_000.0);

        let mut seen: Vec<EntityId> = Vec::new();
        let mut checked = 0_usize;
        // Twenty seconds is many spawns at the Normal rate, and short of the
        // first boss at sixty.
        for _ in 0..1_200 {
            let notes = run.step(STEP);
            assert!(notes.is_empty(), "{name}: {notes:#?}");

            for enemy in tagged(&run, "enemy") {
                if seen.contains(&enemy) {
                    continue;
                }
                seen.push(enemy);

                let [x, y, _] = position(&run, enemy);
                assert!(
                    x.abs() > half_x || y.abs() > half_y,
                    "{name}: an enemy appeared at ({x:.2}, {y:.2}), inside the \
                     {half_x:.2} by {half_y:.2} the camera frames"
                );
                checked += 1;
            }
        }

        assert!(
            checked >= 5,
            "{name}: only {checked} enemies spawned in twenty seconds, which is \
             too few for this to have proved anything"
        );
    }
}
