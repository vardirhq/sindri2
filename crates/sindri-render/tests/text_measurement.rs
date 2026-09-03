//! What a laid-out string covers, which nothing outside the renderer can say.
//!
//! An editor needs the answer to pick a string in a viewport, and a box that is
//! nearly right picks the wrong entity along its edges. So the measurement has
//! to come from the same shaping the frame is drawn with — which is why it is
//! asked of a real `TextRenderer` here rather than worked out from a font size
//! and a character count.
//!
//! No adapter, and that is worth noticing: a string is measured in the scene's
//! own units now, so what it covers no longer depends on a viewport, a
//! resolution, or a GPU. This test used to need all three.

use sindri_render::{TextAlign, TextInstance, TextRenderer, aligned_origin};

const FONT: &str = "fonts/Inter.ttf";

/// A share of the overlay, which is what a font size is. The overlay is two
/// units tall, so this is a label a fifteenth of the screen high.
const SIZE: f32 = 0.06;
const LINE: f32 = 0.075;

/// A renderer with the project's own face bound under a scene reference.
fn renderer() -> TextRenderer {
    let mut text = TextRenderer::new();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../game/assets/fonts/Inter.ttf"
    ))
    .expect("the companion game's font is in the repository");
    text.bind_font(FONT, "Inter", bytes);
    text
}

fn instance(body: &str) -> TextInstance {
    TextInstance::new(
        body,
        FONT,
        [0.0, 0.0],
        SIZE,
        LINE,
        [1.0; 4],
        [TextAlign::Start; 2],
    )
    .expect("a finite instance at the origin")
}

/// The three things a pick box has to get right: it is not empty, it grows with
/// the string, and a second line makes it taller rather than wider.
#[test]
fn a_measured_string_grows_with_what_it_says() {
    let mut text = renderer();

    let short = text
        .measure(&instance("GO"))
        .expect("a bound font measures");
    let long = text
        .measure(&instance("GATHER THE ORBS"))
        .expect("a bound font measures");
    let stacked = text
        .measure(&instance("GO\nGO"))
        .expect("a bound font measures");

    assert!(short[0] > 0.0 && short[1] > 0.0, "got {short:?}");
    assert!(long[0] > short[0], "{long:?} is not wider than {short:?}");
    assert!(
        (short[1] - LINE).abs() < 0.001,
        "one line is one line height, got {}",
        short[1]
    );
    assert!(
        (stacked[1] - LINE * 2.0).abs() < 0.001,
        "two lines are two, got {}",
        stacked[1]
    );
    assert!(
        (stacked[0] - short[0]).abs() < 0.001,
        "and a second line of the same word is no wider"
    );
    assert!(
        long[0] < 2.0,
        "a short label does not measure as tall as the whole overlay is"
    );
}

/// A face that never arrived measures to nothing, which is what the frame does
/// with it too: an unbound font is not drawn, so there is nothing there to
/// click.
#[test]
fn an_unbound_face_measures_nothing() {
    let mut text = renderer();
    let missing = TextInstance::new(
        "GATHER",
        "fonts/Missing.ttf",
        [0.0, 0.0],
        SIZE,
        LINE,
        [1.0; 4],
        [TextAlign::Start; 2],
    )
    .expect("a finite instance at the origin");
    assert!(text.measure(&missing).is_none());
}

/// A centred label is centred on the point, not started at it.
///
/// This is the whole of what a screen looked like without it: every title,
/// every button's word and every HUD reading had its top-left put at the point
/// the scene asked for, so all of them ran off to the right of where they were
/// meant to be. It is checked against a real measurement rather than a guessed
/// width, because the guess is what made it look nearly right.
#[test]
fn a_centred_string_sits_on_its_point_rather_than_starting_at_it() {
    let mut text = renderer();
    let at = [0.25_f32, -0.4_f32];
    let centred = TextInstance::new(
        "LAST STAND",
        FONT,
        at,
        SIZE,
        LINE,
        [1.0; 4],
        [TextAlign::Middle; 2],
    )
    .expect("a finite instance");
    let size = text.measure(&centred).expect("a bound face measures");
    assert!(size[0] > 0.0 && size[1] > 0.0, "{size:?}");

    let origin = aligned_origin(&centred, size);
    assert!(
        (origin[0] - (at[0] - size[0] * 0.5)).abs() < 0.01,
        "{origin:?}"
    );
    // Up, not down: overlay units run the other way from text layout, so a
    // centred string's *top* is half its height above the point it was given.
    assert!(
        (origin[1] - (at[1] + size[1] * 0.5)).abs() < 0.01,
        "{origin:?}"
    );
    // And the string's middle really is the point it was given.
    assert!((origin[0] + size[0] * 0.5 - at[0]).abs() < 0.01);

    // A label anchored into a corner still begins where it was put, so a HUD
    // reading in the top left does not walk off the edge.
    let from_corner = TextInstance::new(
        "Score 0",
        FONT,
        at,
        SIZE,
        LINE,
        [1.0; 4],
        [TextAlign::Start; 2],
    )
    .expect("a finite instance");
    let corner_size = text.measure(&from_corner).expect("measures");
    let corner = aligned_origin(&from_corner, corner_size);
    assert!((corner[0] - at[0]).abs() < f32::EPSILON, "{corner:?}");
    assert!((corner[1] - at[1]).abs() < f32::EPSILON, "{corner:?}");
}

/// A string is geometry, so twice the font size is twice the string.
///
/// The property that makes text behave like everything else drawn: what it
/// covers is a multiple of the size it was authored at and nothing else — not
/// the viewport, not the resolution, not how far the camera happens to be. It is
/// checked here because the atlas underneath bakes every glyph at one fixed em
/// and scales the quads, and a bake that leaked into the measurement would make
/// this ratio drift.
#[test]
fn a_string_measures_in_proportion_to_the_size_it_was_asked_for() {
    let mut text = renderer();
    let sized = |text: &mut TextRenderer, scale: f32| {
        let instance = TextInstance::new(
            "LAST STAND",
            FONT,
            [0.0, 0.0],
            SIZE * scale,
            LINE * scale,
            [1.0; 4],
            [TextAlign::Start; 2],
        )
        .expect("a finite instance");
        text.measure(&instance).expect("a bound face measures")
    };

    let single = sized(&mut text, 1.0);
    let double = sized(&mut text, 2.0);
    assert!(
        (double[0] - single[0] * 2.0).abs() < single[0] * 0.001,
        "{double:?} is not twice {single:?}"
    );
    assert!(
        (double[1] - single[1] * 2.0).abs() < 0.0001,
        "{double:?} is not twice {single:?}"
    );
}
