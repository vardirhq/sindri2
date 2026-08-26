//! What an application supplies, and what the host hands it back.
//!
//! Both proof examples used to carry the host themselves, and carried
//! the same thing. An application now supplies only what is actually
//! its own: how to build itself, what to do with a frame of time, and
//! how to draw.

use std::time::Duration;

use sindri_gpu::{GpuContext, GpuError, WindowSurface};
use sindri_platform::InputEvent;
use thiserror::Error;

/// How the host should open its window.
#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// The id of the canvas element to render into in a browser.
    ///
    /// This is not behind a target conditional on purpose: a project describes
    /// its window once, and the host ignores what does not apply to the target
    /// it was compiled for.
    pub canvas_id: Option<String>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Sindri".into(),
            width: 960,
            height: 540,
            canvas_id: Some("sindri-canvas".into()),
        }
    }
}

impl WindowConfig {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

/// Whether the host should keep running after an update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flow {
    Continue,
    Exit,
}

/// The GPU and surface an application draws with.
///
/// Handed to an application rather than owned by it, because the host has to
/// reconfigure and rebuild the surface underneath it.
#[derive(Clone, Copy)]
pub struct AppContext<'a> {
    pub(super) gpu: &'a GpuContext,
    pub(super) surface: &'a WindowSurface,
}

impl<'a> AppContext<'a> {
    pub const fn gpu(&self) -> &'a GpuContext {
        self.gpu
    }

    pub const fn surface(&self) -> &'a WindowSurface {
        self.surface
    }

    pub const fn device(&self) -> &'a wgpu::Device {
        &self.gpu.device
    }

    pub const fn queue(&self) -> &'a wgpu::Queue {
        &self.gpu.queue
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.surface.format()
    }

    pub const fn width(&self) -> u32 {
        self.surface.width()
    }

    pub const fn height(&self) -> u32 {
        self.surface.height()
    }
}

/// An application the windowed host runs.
///
/// The host owns the window, the surface, the clock, and input. What is left is
/// what only the application knows.
pub trait DesktopApp: Sized + 'static {
    type Error: std::error::Error + 'static;

    /// Builds the application once a device and a configured surface exist.
    ///
    /// This is not asynchronous: the host has already awaited the parts that
    /// genuinely are, so an application that loads no assets does not have to
    /// pretend to be async to say so.
    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error>;

    /// Applies one translated input event.
    ///
    /// Events are handed over rather than accumulated here, because whoever
    /// runs the simulation already accumulates them and clears their per-frame
    /// edges. Two `InputState`s would mean two answers to whether a key is
    /// down.
    fn input(&mut self, event: InputEvent) {
        let _ = event;
    }

    /// Advances by the real time since the previous frame.
    fn update(&mut self, delta: Duration) -> Result<Flow, Self::Error> {
        let _ = delta;
        Ok(Flow::Continue)
    }

    /// Rebuilds anything sized to the surface, which is already reconfigured.
    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> {
        let _ = context;
        Ok(())
    }

    /// Called when the platform suspends the application, such as a browser
    /// page entering the back-forward cache. Ordinary tab visibility is a
    /// separate signal because browsers distinguish the two lifecycles.
    fn suspend(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Called after a platform suspension. The host resets its frame timer
    /// before invoking this hook, so time spent suspended is not delivered as
    /// one catch-up frame.
    fn resume(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Reports whether the browser document is visible. Native hosts leave this
    /// at `true`; browser hosts update it from `document.visibilitychange`.
    fn visibility_changed(&mut self, visible: bool) -> Result<(), Self::Error> {
        let _ = visible;
        Ok(())
    }

    /// Encodes and submits one frame. The host presents what this drew into.
    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Error)]
pub enum DesktopError<E: std::error::Error + 'static> {
    #[error("could not create an event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error("could not create a window: {0}")]
    Window(#[from] winit::error::OsError),
    #[error(transparent)]
    Gpu(#[from] GpuError),
    #[error("the application failed")]
    App(#[source] E),
}
