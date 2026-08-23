//! The windowed host: who owns the window, the event loop, and the frame.
//!
//! Both proof examples used to carry this themselves, and they carried the same
//! thing — a four-state startup, an async device request routed back through the
//! event loop, a redraw that measures its own delta, and a browser canvas lookup
//! behind a target conditional. Written twice, it was two things to keep in
//! agreement. Written here, an application supplies what is actually its own:
//! how to build itself, what to do with a frame of time, and how to draw.

use std::{future::Future, sync::Arc, time::Duration};

use sindri_gpu::{GpuContext, GpuError, GpuRequestOptions, WindowSurface};
use sindri_platform::{FrameTimer, InputEvent};
use thiserror::Error;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};

use crate::{WindowClock, input_event};

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
    gpu: &'a GpuContext,
    surface: &'a WindowSurface,
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

/// Runs an application until its window closes.
///
/// Returns on native targets once the event loop exits. In a browser the event
/// loop is handed to the page and this returns immediately, which is the one
/// place the two targets genuinely differ.
pub fn run<A: DesktopApp>(config: WindowConfig) -> Result<(), DesktopError<A::Error>> {
    let event_loop = EventLoop::with_user_event().build()?;
    let host = Host::<A>::new(&event_loop, config);

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;

        event_loop.spawn_app(host);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut host = host;
        event_loop.run_app(&mut host)?;
        host.failure.map_or(Ok(()), Err)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(future: impl Future<Output = ()> + 'static) {
    pollster::block_on(future);
}

#[cfg(target_arch = "wasm32")]
fn spawn(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

/// The device request, which is the only genuinely asynchronous part of startup.
async fn open_surface(
    display: winit::event_loop::OwnedDisplayHandle,
    window: Arc<Window>,
) -> Result<(GpuContext, WindowSurface), GpuError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
        Box::new(display),
    ));
    let target = Arc::clone(&window);
    let source = move |instance: &wgpu::Instance| instance.create_surface(Arc::clone(&target));
    let surface = source(&instance)?;
    let gpu = GpuContext::request(&instance, Some(&surface), &GpuRequestOptions::default()).await?;
    let size = window.inner_size();
    let surface = WindowSurface::new(instance, surface, source, &gpu, size.width, size.height)?;
    Ok((gpu, surface))
}

/// What asynchronous platform work sends back into the event loop.
enum Startup {
    Opened(Result<(GpuContext, WindowSurface), GpuError>),
    #[cfg(target_arch = "wasm32")]
    VisibilityChanged(bool),
}

#[cfg(target_arch = "wasm32")]
struct VisibilityListener {
    document: web_sys::Document,
    _callback: Closure<dyn FnMut()>,
}

#[cfg(target_arch = "wasm32")]
impl VisibilityListener {
    fn new(proxy: EventLoopProxy<Startup>) -> Option<Self> {
        let document = web_sys::window()?.document()?;
        let observed = document.clone();
        let callback = Closure::wrap(Box::new(move || {
            let visible = !observed.hidden();
            if proxy
                .send_event(Startup::VisibilityChanged(visible))
                .is_err()
            {
                log::debug!("visibility changed after the event loop closed");
            }
        }) as Box<dyn FnMut()>);
        document.set_onvisibilitychange(Some(callback.as_ref().unchecked_ref()));
        Some(Self {
            document,
            _callback: callback,
        })
    }

    fn visible(&self) -> bool {
        !self.document.hidden()
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for VisibilityListener {
    fn drop(&mut self) {
        self.document.set_onvisibilitychange(None);
    }
}

struct Running<A> {
    gpu: GpuContext,
    surface: WindowSurface,
    app: A,
    clock: WindowClock,
    timer: FrameTimer,
}

enum State<A> {
    Waiting,
    Opening,
    Running(Box<Running<A>>),
    Stopped,
}

struct Host<A: DesktopApp> {
    config: WindowConfig,
    proxy: EventLoopProxy<Startup>,
    window: Option<Arc<Window>>,
    state: State<A>,
    failure: Option<DesktopError<A::Error>>,
    page_visible: bool,
    #[cfg(target_arch = "wasm32")]
    _visibility_listener: Option<VisibilityListener>,
}

impl<A: DesktopApp> Host<A> {
    fn new(event_loop: &EventLoop<Startup>, config: WindowConfig) -> Self {
        let proxy = event_loop.create_proxy();
        #[cfg(target_arch = "wasm32")]
        let visibility_listener = VisibilityListener::new(proxy.clone());
        #[cfg(target_arch = "wasm32")]
        let page_visible = visibility_listener
            .as_ref()
            .is_none_or(VisibilityListener::visible);
        #[cfg(not(target_arch = "wasm32"))]
        let page_visible = true;

        Self {
            config,
            proxy,
            window: None,
            state: State::Waiting,
            failure: None,
            page_visible,
            #[cfg(target_arch = "wasm32")]
            _visibility_listener: visibility_listener,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: DesktopError<A::Error>) {
        log::error!("{error}");
        if self.failure.is_none() {
            self.failure = Some(error);
        }
        self.state = State::Stopped;
        event_loop.exit();
    }

    fn attributes(&self) -> winit::window::WindowAttributes {
        #[cfg_attr(
            not(target_arch = "wasm32"),
            expect(
                unused_mut,
                reason = "the \
             canvas is attached only in a browser"
            )
        )]
        let mut attributes = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));

        #[cfg(target_arch = "wasm32")]
        if let Some(id) = &self.config.canvas_id {
            use winit::platform::web::WindowAttributesExtWebSys;

            let canvas = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id(id))
                .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            attributes = attributes.with_canvas(canvas);
        }

        attributes
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), A::Error> {
        let State::Running(running) = &mut self.state else {
            return Ok(());
        };
        running.surface.resize(&running.gpu.device, width, height);
        let context = AppContext {
            gpu: &running.gpu,
            surface: &running.surface,
        };
        running.app.resize(&context)?;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    fn set_visibility(&mut self, event_loop: &ActiveEventLoop, visible: bool) {
        self.page_visible = visible;
        let result = match &mut self.state {
            State::Running(running) => {
                if visible {
                    running.timer.reset();
                }
                running.app.visibility_changed(visible)
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, DesktopError::App(error));
            return;
        }
        if visible && let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn frame(&mut self) -> Result<Flow, DesktopError<A::Error>> {
        let State::Running(running) = &mut self.state else {
            return Ok(Flow::Continue);
        };

        let delta = running.timer.tick(&running.clock);
        let flow = running.app.update(delta).map_err(DesktopError::App)?;
        if flow == Flow::Exit {
            return Ok(flow);
        }

        let Some(frame) = running.surface.acquire(&running.gpu.device)? else {
            return Ok(flow);
        };

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(running.surface.format()),
            ..wgpu::TextureViewDescriptor::default()
        });
        let context = AppContext {
            gpu: &running.gpu,
            surface: &running.surface,
        };
        running
            .app
            .render(&context, &view)
            .map_err(DesktopError::App)?;

        if let Some(window) = &self.window {
            window.pre_present_notify();
        }
        running.gpu.queue.present(frame);
        Ok(flow)
    }
}

impl<A: DesktopApp> ApplicationHandler<Startup> for Host<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, State::Running(_)) {
            let result = match &mut self.state {
                State::Running(running) => {
                    running.timer.reset();
                    running.app.resume()
                }
                _ => unreachable!(),
            };
            if let Err(error) = result {
                self.fail(event_loop, DesktopError::App(error));
            } else if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        if !matches!(self.state, State::Waiting) {
            return;
        }
        self.state = State::Opening;

        let window = match event_loop.create_window(self.attributes()) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(event_loop, DesktopError::Window(error)),
        };
        self.window = Some(Arc::clone(&window));

        let display = event_loop.owned_display_handle();
        let proxy = self.proxy.clone();
        spawn(async move {
            let opened = open_surface(display, window).await;
            if proxy.send_event(Startup::Opened(opened)).is_err() {
                log::error!("the event loop closed before the GPU was ready");
            }
        });
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        let result = match &mut self.state {
            State::Running(running) => {
                running.timer.reset();
                running.app.suspend()
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, DesktopError::App(error));
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Startup) {
        #[cfg(target_arch = "wasm32")]
        if let Startup::VisibilityChanged(visible) = event {
            self.set_visibility(event_loop, visible);
            return;
        }

        let Startup::Opened(opened) = event else {
            return;
        };
        let (gpu, surface) = match opened {
            Ok(parts) => parts,
            Err(error) => return self.fail(event_loop, DesktopError::Gpu(error)),
        };

        log::info!(
            "using GPU adapter '{}' via {:?}",
            gpu.capabilities.adapter_name,
            gpu.capabilities.backend
        );

        let mut app = {
            let context = AppContext {
                gpu: &gpu,
                surface: &surface,
            };
            match A::create(&context) {
                Ok(app) => app,
                Err(error) => return self.fail(event_loop, DesktopError::App(error)),
            }
        };
        if let Err(error) = app.visibility_changed(self.page_visible) {
            return self.fail(event_loop, DesktopError::App(error));
        }

        self.state = State::Running(Box::new(Running {
            gpu,
            surface,
            app,
            clock: WindowClock::new(),
            timer: FrameTimer::new(),
        }));

        if let Some(window) = &self.window {
            let size = window.inner_size();
            if let Err(error) = self.resize(size.width, size.height) {
                self.fail(event_loop, DesktopError::App(error));
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }

        let scale_factor = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor());
        if let Some(input) = input_event(&event, scale_factor)
            && let State::Running(running) = &mut self.state
        {
            running.app.input(input);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Err(error) = self.resize(size.width, size.height) {
                    self.fail(event_loop, DesktopError::App(error));
                }
            }
            WindowEvent::RedrawRequested => match self.frame() {
                Ok(Flow::Exit) => event_loop.exit(),
                Ok(Flow::Continue) => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                Err(error) => self.fail(event_loop, error),
            },
            WindowEvent::Occluded(false) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
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
