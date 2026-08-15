use std::{future::Future, sync::Arc};

use sindri_gpu::{GpuContext, GpuRequestOptions, WindowSurface};
use sindri_render::TriangleRenderer;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

#[cfg(not(target_arch = "wasm32"))]
fn spawn(future: impl Future<Output = ()> + 'static) {
    pollster::block_on(future);
}

#[cfg(target_arch = "wasm32")]
fn spawn(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

struct RenderState {
    window: Arc<Window>,
    gpu: GpuContext,
    surface: WindowSurface,
    renderer: TriangleRenderer,
}

impl RenderState {
    async fn new(
        display: winit::event_loop::OwnedDisplayHandle,
        window: Arc<Window>,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)),
        );
        // How the surface is built, so `WindowSurface` can build it again if it
        // is ever lost. The first one is needed here to pick an adapter that can
        // present to it.
        let target = window.clone();
        let source = move |instance: &wgpu::Instance| instance.create_surface(Arc::clone(&target));
        let surface = source(&instance).map_err(|error| error.to_string())?;
        let gpu = GpuContext::request(&instance, Some(&surface), &GpuRequestOptions::default())
            .await
            .map_err(|error| error.to_string())?;
        let size = window.inner_size();
        let surface = WindowSurface::new(instance, surface, source, &gpu, size.width, size.height)
            .map_err(|error| error.to_string())?;
        let renderer = TriangleRenderer::new(&gpu.device, surface.format());

        Ok(Self {
            window,
            gpu,
            surface,
            renderer,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(&self.gpu.device, width, height);
        self.window.request_redraw();
    }

    fn render(&mut self) {
        let frame = match self.surface.acquire(&self.gpu.device) {
            Ok(Some(frame)) => frame,
            // The surface handled whatever went wrong; the redraw request at
            // the end of this event asks for the frame it cost.
            Ok(None) => return,
            Err(error) => {
                log::error!("could not acquire a surface texture: {error}");
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Sindri triangle encoder"),
            });
        self.renderer.encode(&mut encoder, &view);
        self.gpu.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.gpu.queue.present(frame);
    }
}

enum AppAction {
    Initialized(Result<RenderState, String>),
}

enum AppState {
    Uninitialized,
    Loading,
    Running(Box<RenderState>),
    Failed,
}

struct App {
    proxy: EventLoopProxy<AppAction>,
    window: Option<Arc<Window>>,
    state: AppState,
}

impl App {
    fn new(event_loop: &EventLoop<AppAction>) -> Self {
        Self {
            proxy: event_loop.create_proxy(),
            window: None,
            state: AppState::Uninitialized,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if let AppState::Running(state) = &mut self.state {
            state.resize(width, height);
        }
    }
}

impl ApplicationHandler<AppAction> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !matches!(self.state, AppState::Uninitialized) {
            return;
        }
        self.state = AppState::Loading;

        #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
        let mut attributes = Window::default_attributes()
            .with_title("Sindri — shared native/web triangle")
            .with_inner_size(winit::dpi::LogicalSize::new(960, 540));

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            let canvas = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id("sindri-canvas"))
                .expect("#sindri-canvas must exist")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("#sindri-canvas must be a canvas element");
            attributes = attributes.with_canvas(Some(canvas));
        }

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                log::error!("failed to create window: {error}");
                self.state = AppState::Failed;
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());
        let display = event_loop.owned_display_handle();
        let proxy = self.proxy.clone();

        spawn(async move {
            let result = RenderState::new(display, window).await;
            if proxy.send_event(AppAction::Initialized(result)).is_err() {
                log::error!("event loop closed before GPU initialization completed");
            }
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppAction) {
        match event {
            AppAction::Initialized(Ok(state)) => {
                log::info!(
                    "using GPU adapter '{}' via {:?}",
                    state.gpu.capabilities.adapter_name,
                    state.gpu.capabilities.backend
                );
                self.state = AppState::Running(Box::new(state));
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    self.resize(size.width, size.height);
                }
            }
            AppAction::Initialized(Err(error)) => {
                log::error!("GPU initialization failed: {error}");
                self.state = AppState::Failed;
                event_loop.exit();
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

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                if let AppState::Running(state) = &mut self.state {
                    state.render();
                    state.window.request_redraw();
                }
            }
            WindowEvent::Occluded(false) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let _ = console_log::init_with_level(log::Level::Info);
    }
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    let event_loop = EventLoop::with_user_event()
        .build()
        .expect("failed to create event loop");
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut app = App::new(&event_loop);

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
    #[cfg(not(target_arch = "wasm32"))]
    event_loop.run_app(&mut app).expect("event loop failed");
}
