use std::{future::Future, sync::Arc};

use glam::{Mat4, UVec2, Vec2};
use sindri_gpu::{GpuContext, GpuRequestOptions, SurfaceProfile};
use sindri_render::{DepthTarget, PerspectiveCamera, TexturedCubeRenderer};
use web_time::Instant;
use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, PhysicalKey},
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

#[derive(Default)]
struct RotationInput {
    pressed: [bool; 4],
}

impl RotationInput {
    const LEFT: usize = 0;
    const RIGHT: usize = 1;
    const UP: usize = 2;
    const DOWN: usize = 3;

    fn set(&mut self, key: PhysicalKey, pressed: bool) {
        let index = match key {
            PhysicalKey::Code(KeyCode::ArrowLeft) => Some(Self::LEFT),
            PhysicalKey::Code(KeyCode::ArrowRight) => Some(Self::RIGHT),
            PhysicalKey::Code(KeyCode::ArrowUp) => Some(Self::UP),
            PhysicalKey::Code(KeyCode::ArrowDown) => Some(Self::DOWN),
            _ => None,
        };
        if let Some(index) = index {
            self.pressed[index] = pressed;
        }
    }

    fn axis(&self) -> Vec2 {
        Vec2::new(
            f32::from(u8::from(self.pressed[Self::RIGHT]))
                - f32::from(u8::from(self.pressed[Self::LEFT])),
            f32::from(u8::from(self.pressed[Self::DOWN]))
                - f32::from(u8::from(self.pressed[Self::UP])),
        )
    }
}

struct RenderState {
    instance: wgpu::Instance,
    window: Arc<Window>,
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    surface_profile: SurfaceProfile,
    depth: DepthTarget,
    renderer: TexturedCubeRenderer,
    camera: PerspectiveCamera,
    input: RotationInput,
    rotation: Vec2,
    last_frame: Instant,
}

impl RenderState {
    async fn new(
        display: winit::event_loop::OwnedDisplayHandle,
        window: Arc<Window>,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)),
        );
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| error.to_string())?;
        let gpu = GpuContext::request(&instance, Some(&surface), &GpuRequestOptions::default())
            .await
            .map_err(|error| error.to_string())?;
        let size = window.inner_size();
        let surface_profile = SurfaceProfile::new(&surface, &gpu.adapter, size.width, size.height)
            .map_err(|error| error.to_string())?;
        surface_profile.configure(&surface, &gpu.device);
        let depth = DepthTarget::new(
            &gpu.device,
            surface_profile.width(),
            surface_profile.height(),
        );
        let renderer = TexturedCubeRenderer::new(
            &gpu.device,
            &gpu.queue,
            surface_profile.format(),
        );

        Ok(Self {
            instance,
            window,
            gpu,
            surface,
            surface_profile,
            depth,
            renderer,
            camera: PerspectiveCamera::default(),
            input: RotationInput::default(),
            rotation: Vec2::new(0.45, -0.25),
            last_frame: Instant::now(),
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface_profile.resize(width, height);
        self.surface_profile
            .configure(&self.surface, &self.gpu.device);
        self.depth.resize(
            &self.gpu.device,
            self.surface_profile.width(),
            self.surface_profile.height(),
        );
        self.window.request_redraw();
    }

    fn set_key(&mut self, key: PhysicalKey, state: ElementState) {
        self.input.set(key, state.is_pressed());
    }

    fn render(&mut self) {
        let now = Instant::now();
        let delta_seconds = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.rotation += self.input.axis() * delta_seconds * 1.8;

        let frame = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => frame,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return,
            CurrentSurfaceTexture::Suboptimal(frame) => {
                drop(frame);
                self.reconfigure();
                return;
            }
            CurrentSurfaceTexture::Outdated => {
                self.reconfigure();
                return;
            }
            CurrentSurfaceTexture::Lost => {
                match self.instance.create_surface(self.window.clone()) {
                    Ok(surface) => self.surface = surface,
                    Err(error) => {
                        log::error!("failed to recreate lost surface: {error}");
                        return;
                    }
                }
                self.reconfigure();
                return;
            }
            CurrentSurfaceTexture::Validation => {
                unreachable!("wgpu validation errors are not scoped in the cube example")
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Sindri cube encoder"),
            });
        let viewport =
            UVec2::new(self.surface_profile.width(), self.surface_profile.height()).as_vec2();
        let aspect = viewport.x / viewport.y;
        let model = Mat4::from_rotation_y(self.rotation.x) * Mat4::from_rotation_x(self.rotation.y);
        self.renderer.encode(
            &self.gpu.queue,
            &mut encoder,
            &view,
            &self.depth,
            self.camera.view_projection(aspect) * model,
        );
        self.gpu.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.gpu.queue.present(frame);
    }

    fn reconfigure(&self) {
        self.surface_profile
            .configure(&self.surface, &self.gpu.device);
        self.window.request_redraw();
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
            .with_title("Sindri — shared native/web textured cube")
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                    && event.state.is_pressed()
                {
                    event_loop.exit();
                } else if let AppState::Running(state) = &mut self.state {
                    state.set_key(event.physical_key, event.state);
                }
            }
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
