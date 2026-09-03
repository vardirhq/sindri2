//! What a laid-out string covers, which nothing outside the renderer can say.
//!
//! An editor needs the answer to pick a string in a viewport, and a box that is
//! nearly right picks the wrong entity along its edges. So the measurement has
//! to come from the same shaping the frame is drawn with — which means a real
//! `TextRenderer`, which means an adapter, which is why this lives here beside
//! the other tests that need one.

use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{TextAlign, TextInstance, TextRenderer, Viewport, aligned_origin};

/// Set wherever a software adapter is installed on purpose. A GPU test that
/// skips on the machine that exists to run it is a check that quietly stopped
/// checking, so CI demands the adapter rather than hoping for it.
const REQUIRE_GPU: &str = "SINDRI_REQUIRE_GPU";

const FONT: &str = "fonts/Inter.ttf";
const VIEWPORT: Viewport = Viewport {
    x: 0,
    y: 0,
    width: 800,
    height: 600,
};

/// A renderer with the project's own face bound under a scene reference, or
/// `None` where there is no adapter to build one on.
fn renderer() -> Option<TextRenderer> {
    let instance = wgpu::Instance::default();
    let gpu = match pollster::block_on(GpuContext::request(
        &instance,
        None,
        &GpuRequestOptions::default(),
    )) {
        Ok(gpu) => gpu,
        Err(error) => {
            assert!(
                std::env::var_os(REQUIRE_GPU).is_none(),
                "{REQUIRE_GPU} is set but no adapter could be requested: {error}"
            );
            eprintln!("skipping: no GPU adapter ({error})");
            return None;
        }
    };
    let mut text = TextRenderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8UnormSrgb);
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../game/assets/fonts/Inter.ttf"
    ))
    .expect("the companion game's font is in the repository");
    text.bind_font(FONT, "Inter", bytes);
    Some(text)
}

fn instance(body: &str) -> TextInstance {
    TextInstance::new(
        body,
        FONT,
        [0.0, 0.0],
        24.0,
        30.0,
        [1.0; 4],
        [TextAlign::Start; 2],
    )
    .expect("a finite instance at the origin")
}

/// The three things a pick box has to get right: it is not empty, it grows with
/// the string, and a second line makes it taller rather than wider.
#[test]
fn a_measured_string_grows_with_what_it_says() {
    let Some(mut text) = renderer() else {
        return;
    };

    let short = text
        .measure(&instance("GO"), VIEWPORT)
        .expect("a bound font measures");
    let long = text
        .measure(&instance("GATHER THE ORBS"), VIEWPORT)
        .expect("a bound font measures");
    let stacked = text
        .measure(&instance("GO\nGO"), VIEWPORT)
        .expect("a bound font measures");

    assert!(short[0] > 0.0 && short[1] > 0.0, "got {short:?}");
    assert!(long[0] > short[0], "{long:?} is not wider than {short:?}");
    assert!(
        (short[1] - 30.0).abs() < 0.001,
        "one line is one line height, got {}",
        short[1]
    );
    assert!(
        (stacked[1] - 60.0).abs() < 0.001,
        "two lines are two, got {}",
        stacked[1]
    );
    assert!(
        (stacked[0] - short[0]).abs() < 0.001,
        "and a second line of the same word is no wider"
    );
    assert!(
        long[0] < 800.0,
        "a string that fits the viewport does not measure as the viewport"
    );
}

/// A face that never arrived measures to nothing, which is what the frame does
/// with it too: an unbound font is not drawn, so there is nothing there to
/// click.
#[test]
fn an_unbound_face_measures_nothing() {
    let Some(mut text) = renderer() else {
        return;
    };
    let missing = TextInstance::new(
        "GATHER",
        "fonts/Missing.ttf",
        [0.0, 0.0],
        24.0,
        30.0,
        [1.0; 4],
        [TextAlign::Start; 2],
    )
    .expect("a finite instance at the origin");
    assert!(text.measure(&missing, VIEWPORT).is_none());
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
    let Some(mut text) = renderer() else {
        return;
    };
    let at = [400.0_f32, 300.0_f32];
    let centred = TextInstance::new(
        "LAST STAND",
        FONT,
        at,
        24.0,
        30.0,
        [1.0; 4],
        [TextAlign::Middle; 2],
    )
    .expect("a finite instance");
    let size = text
        .measure(&centred, VIEWPORT)
        .expect("a bound face measures");
    assert!(size[0] > 0.0 && size[1] > 0.0, "{size:?}");

    let origin = aligned_origin(&centred, size);
    assert!(
        (origin[0] - (at[0] - size[0] * 0.5)).abs() < 0.01,
        "{origin:?}"
    );
    assert!(
        (origin[1] - (at[1] - size[1] * 0.5)).abs() < 0.01,
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
        24.0,
        30.0,
        [1.0; 4],
        [TextAlign::Start; 2],
    )
    .expect("a finite instance");
    let corner_size = text.measure(&from_corner, VIEWPORT).expect("measures");
    let corner = aligned_origin(&from_corner, corner_size);
    assert!((corner[0] - at[0]).abs() < f32::EPSILON, "{corner:?}");
    assert!((corner[1] - at[1]).abs() < f32::EPSILON, "{corner:?}");
}
