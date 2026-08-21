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

/// What a surface will store, and what it will be drawn through.
///
/// The two differ only where a surface cannot offer sRGB directly, which in
/// practice means a browser canvas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChosenFormat {
    /// What the swapchain holds.
    pub storage: wgpu::TextureFormat,
    /// What the renderer draws through, always sRGB.
    pub view: wgpu::TextureFormat,
}

impl ChosenFormat {
    /// The view formats a surface configuration must declare, which is none at
    /// all when the storage format is already what is drawn through.
    fn view_formats(self) -> Vec<wgpu::TextureFormat> {
        if self.view == self.storage {
            Vec::new()
        } else {
            vec![self.view]
        }
    }
}

/// Picks what to store and what to draw through, from what a surface offers.
///
/// The renderer works in linear and relies on the target encoding on write, so
/// what must be sRGB is the *view*. A surface offering an sRGB format gives one
/// directly and nothing else is needed.
///
/// A browser canvas offers `bgra8unorm` and no sRGB format at all, and this
/// engine used to stop there — `NoSrgbSurfaceFormat`, at startup, on every page
/// load. That was not wrong to refuse: drawing into a non-sRGB target really
/// would darken every colour. It was wrong to conclude there was no way to
/// encode. WebGPU's answer is a view format: the swapchain stores non-sRGB
/// bytes, and the view the renderer draws through encodes on write exactly as
/// it always did. The invariant is untouched — every frame is written through
/// an sRGB view — and only where that view comes from has moved.
///
/// A surface offering neither is still refused, because then there is genuinely
/// nowhere to encode.
pub fn choose_format(offered: &[wgpu::TextureFormat]) -> Result<ChosenFormat, GpuError> {
    if let Some(format) = offered.iter().copied().find(wgpu::TextureFormat::is_srgb) {
        return Ok(ChosenFormat {
            storage: format,
            view: format,
        });
    }
    offered
        .iter()
        .copied()
        .find_map(|format| {
            let view = format.add_srgb_suffix();
            view.is_srgb().then_some(ChosenFormat {
                storage: format,
                view,
            })
        })
        .ok_or(GpuError::NoSrgbSurfaceFormat)
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

        let chosen = choose_format(&capabilities.formats)?;
        config.format = chosen.storage;
        config.view_formats = chosen.view_formats();
        config.width = width;
        config.height = height;
        Ok(Self { config })
    }

    /// The format the swapchain stores.
    pub const fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// The format a frame's view must be created with, which is the one the
    /// renderer's pipelines are built against.
    ///
    /// The same as [`Self::format`] whenever the surface offered sRGB directly,
    /// and its sRGB variant when the surface could only offer the storage
    /// format. Hosts ask for this rather than reaching for the config, so there
    /// is one answer to "what am I drawing into" rather than one per host.
    pub fn view_format(&self) -> wgpu::TextureFormat {
        self.config
            .view_formats
            .first()
            .copied()
            .unwrap_or(self.config.format)
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

    /// The format a renderer builds its pipelines against, and the one a
    /// frame's view is created with.
    ///
    /// Deliberately the *view* format and not the swapchain's storage format:
    /// the only thing a host does with this is describe what it draws into, and
    /// on a browser canvas those two differ. Handing back the storage format
    /// would build pipelines that do not match the view they render through,
    /// which wgpu rejects at draw time with a message about neither.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.profile.view_format()
    }

    /// What the swapchain stores, which only the configuration cares about.
    pub const fn storage_format(&self) -> wgpu::TextureFormat {
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

#[cfg(test)]
mod format_tests {
    use super::{ChosenFormat, choose_format};
    use crate::GpuError;
    use wgpu::TextureFormat;

    /// What a desktop swapchain offers. An sRGB format is taken directly and
    /// nothing about the old behaviour changes.
    #[test]
    fn an_srgb_surface_is_drawn_through_its_own_format() {
        let chosen = choose_format(&[
            TextureFormat::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Rgba8Unorm,
        ])
        .expect("an sRGB format is offered");
        assert_eq!(
            chosen,
            ChosenFormat {
                storage: TextureFormat::Bgra8UnormSrgb,
                view: TextureFormat::Bgra8UnormSrgb,
            }
        );
    }

    /// What a browser canvas offers, which is what stopped this engine at
    /// startup on every page load: no sRGB format at all.
    ///
    /// It is not a reason to draw in the wrong colour space, and it is not a
    /// reason to refuse. The swapchain stores what the canvas can hold and the
    /// renderer draws through an sRGB view of it.
    #[test]
    fn a_canvas_is_drawn_through_an_srgb_view_of_what_it_can_hold() {
        let chosen = choose_format(&[TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm])
            .expect("a canvas can still be encoded to");
        assert_eq!(
            chosen,
            ChosenFormat {
                storage: TextureFormat::Bgra8Unorm,
                view: TextureFormat::Bgra8UnormSrgb,
            }
        );
        assert!(
            chosen.view.is_srgb(),
            "the view encodes, whatever is stored"
        );
    }

    /// A surface that can hold neither is still refused, because then there is
    /// genuinely nowhere to encode and a frame really would come out dark.
    #[test]
    fn a_surface_that_cannot_encode_at_all_is_refused() {
        assert!(matches!(
            choose_format(&[TextureFormat::Rgba16Float, TextureFormat::R8Unorm]),
            Err(GpuError::NoSrgbSurfaceFormat)
        ));
        assert!(matches!(
            choose_format(&[]),
            Err(GpuError::NoSrgbSurfaceFormat)
        ));
    }
}
