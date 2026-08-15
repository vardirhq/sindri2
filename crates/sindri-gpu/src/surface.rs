//! Presentation surfaces, and what to do when acquiring one does not work.
//!
//! Asking for the next swapchain texture has seven outcomes and only one of
//! them is a frame. The other six each want a different response: two mean the
//! configuration is stale and must be replaced, one means the surface itself is
//! gone, and three mean nothing is wrong and there is simply nothing to draw
//! into yet. Getting that wrong is not loud — reconfiguring on every occluded
//! frame rebuilds the swapchain in a loop behind a minimised window, and
//! treating a lost surface as a skipped frame renders nothing forever.
//!
//! So the decision lives here once, as [`SurfaceStatus::action`], rather than in
//! each host that presents a frame.

use std::fmt;

use wgpu::CurrentSurfaceTexture;

use crate::{GpuContext, GpuError};

/// The negotiated presentation configuration for a surface.
#[derive(Clone, Debug)]
pub struct SurfaceProfile {
    config: wgpu::SurfaceConfiguration,
}

impl SurfaceProfile {
    pub fn new(
        surface: &wgpu::Surface<'_>,
        adapter: &wgpu::Adapter,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let width = width.max(1);
        let height = height.max(1);
        let mut config = surface
            .get_default_config(adapter, width, height)
            .ok_or(GpuError::UnsupportedSurface)?;
        let capabilities = surface.get_capabilities(adapter);

        // The renderer works in linear and relies on the target encoding on
        // write. A non-sRGB swapchain would silently darken every colour, so it
        // is refused rather than accepted as a fallback.
        config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .ok_or(GpuError::NoSrgbSurfaceFormat)?;
        config.width = width;
        config.height = height;
        Ok(Self { config })
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub const fn width(&self) -> u32 {
        self.config.width
    }

    pub const fn height(&self) -> u32 {
        self.config.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
    }

    pub fn configure(&self, surface: &wgpu::Surface<'_>, device: &wgpu::Device) {
        surface.configure(device, &self.config);
    }
}

/// The outcome of asking a surface for its next texture.
///
/// This mirrors [`wgpu::CurrentSurfaceTexture`] without the texture its
/// successful variants carry. Those textures cannot be built without a GPU, so
/// mirroring the outcome is what lets the policy below be checked for every
/// case instead of only the ones a test can construct.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SurfaceStatus {
    /// A texture matching the current configuration.
    Ready,
    /// A texture that no longer matches the surface it came from.
    Suboptimal,
    /// The driver did not produce a texture in time.
    Timeout,
    /// The window is minimised or fully covered.
    Occluded,
    /// The surface changed and its configuration no longer describes it.
    Outdated,
    /// The surface no longer exists and must be built again.
    Lost,
    /// A validation error was raised inside the acquisition itself.
    Validation,
}

/// What a host should do about a [`SurfaceStatus`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SurfaceAction {
    /// Draw into the acquired texture and present it.
    Present,
    /// Skip the frame, leaving the surface as it is.
    Skip,
    /// Reconfigure the surface, then skip the frame.
    Reconfigure,
    /// Build the surface again, reconfigure it, then skip the frame.
    Recreate,
}

impl SurfaceStatus {
    /// The response this outcome calls for.
    ///
    /// The match is exhaustive on purpose: a new `wgpu` outcome cannot be
    /// classified by accident, because adding one stops this compiling.
    pub const fn action(self) -> SurfaceAction {
        match self {
            Self::Ready => SurfaceAction::Present,

            // None of these is fixed by touching the surface. A hidden window
            // and a slow driver both resolve themselves, and reconfiguring
            // would rebuild the swapchain every frame for as long as a window
            // stays minimised. A validation error is already reported through
            // the device's error scope, and skipping keeps the loop running so
            // that report is what the developer reads, rather than a panic
            // thrown from the presentation path.
            Self::Timeout | Self::Occluded | Self::Validation => SurfaceAction::Skip,

            // The texture could be presented, but it does not match the surface
            // it came from — mid-resize, this is a stretched frame. Replacing
            // the configuration now costs one frame and makes the next one
            // correct.
            Self::Suboptimal | Self::Outdated => SurfaceAction::Reconfigure,

            Self::Lost => SurfaceAction::Recreate,
        }
    }
}

/// How a host builds the surface it presents to.
///
/// A lost surface has to be built again from whatever it was attached to, and
/// only the host knows what that is. Taking a closure rather than a window
/// keeps this crate's promise that windows and event loops belong to platform
/// hosts: the same type serves a `winit` window and a browser canvas.
pub type SurfaceSource =
    dyn Fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>;

/// A surface a host presents to, and the policy that keeps it presentable.
///
/// Hosts acquire through [`acquire`](Self::acquire) and never see the outcomes
/// it recovers from.
pub struct WindowSurface {
    instance: wgpu::Instance,
    source: Box<SurfaceSource>,
    surface: wgpu::Surface<'static>,
    profile: SurfaceProfile,
}

impl WindowSurface {
    /// Adopts a surface and configures it.
    ///
    /// The caller passes a surface it has already built because one must exist
    /// before [`GpuContext::request`](crate::GpuContext::request) can choose an
    /// adapter able to present to it. `source` builds another the same way, and
    /// is used only if the surface is ever lost.
    pub fn new(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        source: impl Fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>
        + 'static,
        gpu: &GpuContext,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let profile = SurfaceProfile::new(&surface, &gpu.adapter, width, height)?;
        profile.configure(&surface, &gpu.device);
        Ok(Self {
            instance,
            source: Box::new(source),
            surface,
            profile,
        })
    }

    pub const fn profile(&self) -> &SurfaceProfile {
        &self.profile
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.profile.format()
    }

    pub const fn width(&self) -> u32 {
        self.profile.width()
    }

    pub const fn height(&self) -> u32 {
        self.profile.height()
    }

    /// Resizes and reconfigures. Zero dimensions are clamped, so a minimised
    /// window does not produce an invalid configuration.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.profile.resize(width, height);
        self.profile.configure(&self.surface, device);
    }

    /// Acquires the next texture, recovering from anything that is recoverable.
    ///
    /// `Ok(None)` means this frame is skipped: the surface has already been
    /// reconfigured or rebuilt if the outcome called for it, and the host should
    /// ask for another frame. An error means the surface was lost and could not
    /// be built again, which no amount of retrying fixes.
    pub fn acquire(
        &mut self,
        device: &wgpu::Device,
    ) -> Result<Option<wgpu::SurfaceTexture>, GpuError> {
        let (texture, status) = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture) => (Some(texture), SurfaceStatus::Ready),
            CurrentSurfaceTexture::Suboptimal(texture) => {
                (Some(texture), SurfaceStatus::Suboptimal)
            }
            CurrentSurfaceTexture::Timeout => (None, SurfaceStatus::Timeout),
            CurrentSurfaceTexture::Occluded => (None, SurfaceStatus::Occluded),
            CurrentSurfaceTexture::Outdated => (None, SurfaceStatus::Outdated),
            CurrentSurfaceTexture::Lost => (None, SurfaceStatus::Lost),
            CurrentSurfaceTexture::Validation => (None, SurfaceStatus::Validation),
        };

        match status.action() {
            SurfaceAction::Present => Ok(texture),
            SurfaceAction::Skip => Ok(None),
            SurfaceAction::Reconfigure => {
                // Dropping discards the texture rather than presenting it.
                drop(texture);
                self.profile.configure(&self.surface, device);
                Ok(None)
            }
            SurfaceAction::Recreate => {
                drop(texture);
                self.surface = (self.source)(&self.instance)?;
                self.profile.configure(&self.surface, device);
                Ok(None)
            }
        }
    }
}

impl fmt::Debug for WindowSurface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowSurface")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
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
}
