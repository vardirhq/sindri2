//! What a laid-out string covers, which nothing outside the renderer can say.
//!
//! An editor needs the answer to pick a string in a viewport, and a box that is
//! nearly right picks the wrong entity along its edges. So the measurement has
//! to come from the same shaping the frame is drawn with — which means a real
//! `TextRenderer`, which means an adapter, which is why this lives here beside
//! the other tests that need one.

use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{TextInstance, TextRenderer, Viewport};

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
    TextInstance::new(body, FONT, [0.0, 0.0], 24.0, 30.0, [1.0; 4])
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
    )
    .expect("a finite instance at the origin");
    assert!(text.measure(&missing, VIEWPORT).is_none());
}
