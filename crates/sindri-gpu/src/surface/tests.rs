//! Configuring a surface, and what it refuses.

use super::*;

const EVERY_STATUS: [SurfaceStatus; 7] = [
    SurfaceStatus::Ready,
    SurfaceStatus::Suboptimal,
    SurfaceStatus::Timeout,
    SurfaceStatus::Occluded,
    SurfaceStatus::Outdated,
    SurfaceStatus::Lost,
    SurfaceStatus::Validation,
];

#[test]
fn only_a_ready_texture_is_presented() {
    for status in EVERY_STATUS {
        assert_eq!(
            status.action() == SurfaceAction::Present,
            status == SurfaceStatus::Ready,
            "{status:?} disagrees with itself about being a drawable frame"
        );
    }
}

#[test]
fn a_hidden_window_is_not_a_reason_to_rebuild_the_swapchain() {
    // Both fix themselves, and both arrive every frame while they last.
    assert_eq!(SurfaceStatus::Occluded.action(), SurfaceAction::Skip);
    assert_eq!(SurfaceStatus::Timeout.action(), SurfaceAction::Skip);
}

#[test]
fn a_stale_configuration_is_replaced_before_the_next_frame() {
    assert_eq!(SurfaceStatus::Outdated.action(), SurfaceAction::Reconfigure);
    assert_eq!(
        SurfaceStatus::Suboptimal.action(),
        SurfaceAction::Reconfigure
    );
}

#[test]
fn only_a_lost_surface_is_rebuilt() {
    // Rebuilding is the one response that throws away GPU state, so no
    // recoverable outcome may reach for it.
    for status in EVERY_STATUS {
        assert_eq!(
            status.action() == SurfaceAction::Recreate,
            status == SurfaceStatus::Lost,
            "{status:?} disagrees with itself about the surface still existing"
        );
    }
}

#[test]
fn a_validation_error_skips_the_frame_rather_than_ending_the_run() {
    // The error scope reports it. A panic here would replace that report
    // with a backtrace through the presentation path.
    assert_eq!(SurfaceStatus::Validation.action(), SurfaceAction::Skip);
}

#[test]
fn a_profile_never_configures_a_zero_sized_surface() {
    let mut profile = SurfaceProfile {
        config: wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 960,
            height: 540,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        },
    };

    profile.resize(0, 0);

    assert_eq!(profile.width(), 1);
    assert_eq!(profile.height(), 1);
}
