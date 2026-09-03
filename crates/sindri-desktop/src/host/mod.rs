//! The windowed host: who owns the window, the event loop, and the frame.
//!
//! Both proof examples used to carry this themselves, and they carried the
//! same thing — a four-state startup, an async device request routed back
//! through the event loop, a redraw that measures its own delta, and a browser
//! canvas lookup behind a target conditional. Written twice, it was two things
//! to keep in agreement. Written here, an application supplies what is
//! actually its own: how to build itself, what to do with a frame of time, and
//! how to draw.
//!
//! `app` is that contract, `startup` is getting a window and a device, and
//! this file is the loop between them.

mod app;
mod page_size;
mod startup;
mod visibility;

use self::startup::{Startup, open_surface};

pub use app::{AppContext, DesktopApp, DesktopError, Flow, WindowConfig};
pub use startup::run;

#[cfg(target_arch = "wasm32")]
use self::page_size::PageSizeListener;
#[cfg(target_arch = "wasm32")]
use self::visibility::VisibilityListener;

use std::sync::Arc;

use sindri_gpu::{GpuContext, WindowSurface};
use sindri_platform::FrameTimer;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::{WindowClock, input_event};

use self::startup::spawn;

#[cfg(target_arch = "wasm32")]
use self::startup::announce_failure;

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
    #[cfg(target_arch = "wasm32")]
    _page_size_listener: Option<PageSizeListener>,
}

impl<A: DesktopApp> Host<A> {
    fn new(event_loop: &EventLoop<Startup>, config: WindowConfig) -> Self {
        let proxy = event_loop.create_proxy();
        #[cfg(target_arch = "wasm32")]
        let visibility_listener = VisibilityListener::new(proxy.clone());
        #[cfg(target_arch = "wasm32")]
        let page_size_listener = PageSizeListener::new(proxy.clone());
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
            #[cfg(target_arch = "wasm32")]
            _page_size_listener: page_size_listener,
        }
    }

    /// Records the first failure and stops. A host that kept drawing after
    /// gameplay failed would replace the error with whatever happened next.
    ///
    /// It is *logged* as well as recorded, and that is not belt and braces. On
    /// a desktop the recorded error is returned by [`run`], which prints it. In
    /// a browser there is nobody to return to — `spawn_app` hands the loop to
    /// the page and `run` has already returned `Ok` — so a failure recorded and
    /// not logged is a failure that happens in silence. The first time this
    /// engine was loaded in a browser it stopped at the device request and said
    /// nothing at all, which is what this line is for.
    ///
    /// A log line is still only half of it, because a player has no console. In
    /// a browser the failure is also announced as a DOM event, so the page can
    /// show it. The engine deliberately does not know what that page looks
    /// like: it names the failure and the page decides what to do with it,
    /// which is the same arrangement as the canvas it is handed rather than
    /// creates.
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: DesktopError<A::Error>) {
        log::error!("{error}");
        #[cfg(target_arch = "wasm32")]
        announce_failure(&error.to_string());
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

        // In a browser the page *is* the window, so a project's requested size
        // is the wrong answer to a question the page already answers. Asking
        // for one there left a phone showing a 960 by 540 letterbox in the
        // middle of an empty screen.
        #[cfg(target_arch = "wasm32")]
        if let Some((width, height)) = page_size::page_size() {
            attributes = attributes.with_inner_size(winit::dpi::LogicalSize::new(width, height));
        }

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

    #[cfg(target_arch = "wasm32")]
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

    /// One frame: advance by real elapsed time, then draw if a texture arrives.
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
            // Skipped. The surface has already recovered if it needed to.
            return Ok(flow);
        };

        // Made with the surface's view format rather than the texture's own,
        // which is what lets a canvas that stores non-sRGB bytes still be drawn
        // through an sRGB view. They are the same format wherever the surface
        // offered sRGB directly.
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
        let opened = match event {
            Startup::VisibilityChanged(visible) => {
                self.set_visibility(event_loop, visible);
                return;
            }
            Startup::PageResized(width, height) => {
                if let Some(window) = &self.window {
                    // Requested rather than set: the browser decides what a
                    // canvas actually becomes, and winit reports the result
                    // back through the ordinary resize path.
                    let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
                }
                return;
            }
            Startup::Opened(opened) => opened,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let Startup::Opened(opened) = event;

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

        // The window may have been resized while the device was being requested.
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
