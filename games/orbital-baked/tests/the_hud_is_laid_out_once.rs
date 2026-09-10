//! Every HUD element is placed by the scene, in the scene's own coordinates.
//!
//! This is a narrow claim and worth stating plainly, because the obvious test is
//! not available. What went wrong here was two labels drawn over each other, and
//! that is a fact about pixels after the viewport has mapped everything -- two
//! elements can be far apart in scene coordinates and land on top of each other
//! anyway, which is exactly what happened: the health label sat at y=1.83 and
//! the sector name at y=-0.155, nearly two units apart and touching on screen.
//!
//! So this asserts the thing that actually went wrong instead: an element placed
//! outside the band the scene lays out in. Every authored HUD position is within
//! about a fifth of a unit of the origin; the overrides that caused the collision
//! were at 1.71, 1.76 and 1.83, which is not a different position so much as a
//! different coordinate system. Something that far out is a HUD element being
//! placed by hand rather than by the scene, which is the mistake worth catching.

use orbital_baked::Run;

const STEP: f32 = 1.0 / 60.0;
const BAND: f32 = 0.5;

#[test]
fn no_hud_element_is_placed_outside_the_scene_band() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        run.step(STEP);
    }
    run.click("TitleStart");
    for _ in 0..30 {
        run.step(STEP);
    }

    let mut strays: Vec<String> = Vec::new();
    for element in [
        "Health",
        "HealthBack",
        "HealthText",
        "Cores",
        "CoresBack",
        "Wave",
        "Level",
        "Score",
        "Clock",
    ] {
        let entity = run
            .find(element)
            .unwrap_or_else(|| panic!("the HUD has no {element}"));
        let at = run
            .world
            .get(entity)
            .and_then(|data| data.transform_3d.as_ref())
            .expect("the element has a transform")
            .position;
        if at[1].abs() > BAND {
            strays.push(format!("{element} at y={:.3}", at[1]));
        }
    }
    assert!(
        strays.is_empty(),
        "these are placed outside the band the scene lays out in, so something \
         is positioning them by hand: {strays:?}"
    );
}
