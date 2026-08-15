use std::{future::Future, sync::Arc};

use glam::Vec2;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::AssetId;
use sindri_gpu::{GpuContext, GpuRequestOptions, SurfaceProfile};
use sindri_platform::{InputState, Key};
use sindri_render::{
    DepthTarget, DrawContext, FrameCommand, PreparedFrame, SpriteBatchError, SpriteBatchRenderer,
    SpriteBatchStats, Texture2D, TextureRegistry, TexturedCubeRenderer, Viewport,
};
use web_time::Instant;
use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod scene;

pub use scene::{DemoScene, DemoSceneError};
pub use sindri_scene::{CameraView, TextureBindings, WorldProjection};

#[derive(Clone, Copy)]
pub struct FrameTarget<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a DepthTarget,
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(future: impl Future<Output = ()> + 'static) {
    pollster::block_on(future);
}

#[cfg(target_arch = "wasm32")]
fn spawn(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

struct RenderState {
    instance: wgpu::Instance,
    window: Arc<Window>,
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    surface_profile: SurfaceProfile,
    depth: DepthTarget,
    cube_renderer: TexturedCubeRenderer,
    sprite_renderer: SpriteBatchRenderer,
    textures: TextureRegistry,
    bindings: TextureBindings,
    scene: DemoScene,
    input: InputState,
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
        let cube_renderer = TexturedCubeRenderer::new(&gpu.device, surface_profile.format());
        let sprite_renderer = SpriteBatchRenderer::new(&gpu.device, surface_profile.format());
        let (textures, bindings) = demo_textures(&gpu.device, &gpu.queue);
        let scene = DemoScene::load().map_err(|error| error.to_string())?;

        Ok(Self {
            instance,
            window,
            gpu,
            surface,
            surface_profile,
            depth,
            cube_renderer,
            sprite_renderer,
            textures,
            bindings,
            scene,
            input: InputState::default(),
            rotation: Vec2::ZERO,
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

    fn handle_input(&mut self, event: &WindowEvent) {
        if let Some(event) = sindri_desktop::input_event(event, self.window.scale_factor()) {
            self.input.apply(event);
        }
    }

    fn render(&mut self) {
        let now = Instant::now();
        let delta_seconds = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let axis = Vec2::new(
            self.input.axis(Key::ArrowLeft, Key::ArrowRight),
            self.input.axis(Key::ArrowUp, Key::ArrowDown),
        );
        self.rotation += axis * delta_seconds * 1.8;
        self.input.begin_frame();

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
        let viewport = Viewport::new(self.surface_profile.width(), self.surface_profile.height());
        // Gameplay writes the world; extraction reads whatever it now holds.
        self.scene
            .spin_cube(self.rotation)
            .expect("the demo scene keeps its cube");
        let prepared = self
            .scene
            .extract_frame(viewport, &self.bindings)
            .expect("embedded demo scene extracts into a valid frame");
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut self.cube_renderer,
                sprites: &mut self.sprite_renderer,
                textures: &self.textures,
            },
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            FrameTarget {
                color: &view,
                depth: &self.depth,
            },
            &prepared,
        )
        .expect("demo sprite batch fits the GPU instance buffer");
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
            WindowEvent::KeyboardInput { event: ref key, .. } => {
                if key.physical_key == PhysicalKey::Code(KeyCode::Escape) && key.state.is_pressed()
                {
                    event_loop.exit();
                } else if let AppState::Running(state) = &mut self.state {
                    state.handle_input(&event);
                }
            }
            WindowEvent::Focused(_) | WindowEvent::CursorLeft { .. } => {
                if let AppState::Running(state) = &mut self.state {
                    state.handle_input(&event);
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

/// The demo's textures, registered under the references its scene names.
///
/// The checkerboard is generated, which is what `procedural:` marks; the badge
/// is a real PNG decoded through the asset pipeline. Both bind the same way,
/// because binding cares about the reference, not where the pixels came from.
pub fn demo_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (TextureRegistry, TextureBindings) {
    let mut registry = TextureRegistry::new(device, queue);
    let mut bindings = TextureBindings::new();

    let checkerboard = registry.insert(
        Texture2D::checkerboard(
            device,
            queue,
            "Sindri cube checkerboard",
            64,
            8,
            [[18, 34, 55, 255], [240, 114, 43, 255]],
        )
        .expect("static checkerboard texture dimensions are valid"),
    );
    bindings.bind("procedural:checkerboard", checkerboard);

    let badge = registry.insert(decode_texture(
        device,
        queue,
        "textures/badge.png",
        BADGE_PNG,
    ));
    bindings.bind("textures/badge.png", badge);

    (registry, bindings)
}

/// The badge image, decoded from a real PNG through the asset pipeline.
///
/// Embedded rather than read from disk so the example needs no I/O on either
/// target; the bytes still travel the same decode path a file or fetch would.
const BADGE_PNG: &[u8] = include_bytes!("../assets/textures/badge.png");

/// The badge as raw RGBA, which `assets/textures/badge.png` encodes.
///
/// Kept so a test can prove the shipped PNG is exactly this image, rather than
/// trusting that swapping a generator for a file left the frame unchanged.
pub fn demo_badge_pixels() -> Vec<u8> {
    const SIZE: u32 = 64;
    let mut pixels =
        Vec::with_capacity(usize::try_from(SIZE * SIZE * 4).expect("badge fits usize"));
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = i64::from(x) - 31;
            let dy = i64::from(y) - 31;
            let distance_squared = dx * dx + dy * dy;
            let bolt = (30..=38).contains(&x) && (12..=31).contains(&y)
                || (24..=38).contains(&x) && (27..=36).contains(&y)
                || (24..=32).contains(&x) && (32..=51).contains(&y);
            let color = if distance_squared > 31 * 31 {
                [0, 0, 0, 0]
            } else if distance_squared > 27 * 27 {
                [240, 114, 43, 255]
            } else if bolt {
                [255, 244, 214, 255]
            } else {
                [18, 34, 55, 235]
            };
            pixels.extend_from_slice(&color);
        }
    }
    pixels
}

/// Decodes an embedded PNG into an upload-ready texture.
///
/// This is the whole bridge: `sindri-assets` turns bytes into a `TextureAsset`,
/// and `sindri-render` turns that into something drawable. A file read or an
/// HTTP fetch produces the same bytes and joins here.
pub fn decode_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    id: &str,
    bytes: &[u8],
) -> Texture2D {
    let asset_id = AssetId::new(id).expect("demo asset IDs are valid");
    let decoded = TextureAssetDecoder
        .decode(AssetBytes::new(asset_id, bytes.to_vec()))
        .expect("the embedded texture decodes");
    Texture2D::from_rgba8(
        device,
        queue,
        id,
        decoded.width(),
        decoded.height(),
        decoded.rgba8(),
    )
    .expect("a decoded texture has valid dimensions")
}

/// The renderers a frame is drawn with.
pub struct FrameRenderers<'a> {
    pub cube: &'a mut TexturedCubeRenderer,
    pub sprites: &'a mut SpriteBatchRenderer,
    pub textures: &'a TextureRegistry,
}

pub fn encode_prepared_frame(
    renderers: FrameRenderers<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: FrameTarget<'_>,
    frame: &PreparedFrame,
) -> Result<SpriteBatchStats, SpriteBatchError> {
    let FrameRenderers {
        cube: cube_renderer,
        sprites: sprite_renderer,
        textures,
    } = renderers;
    let mut sprite_stats = SpriteBatchStats::default();
    for pass in frame.passes() {
        match &pass.command {
            FrameCommand::TexturedCube { model, texture } => cube_renderer.encode_with_clear(
                DrawContext {
                    device,
                    queue,
                    textures,
                    texture: *texture,
                },
                encoder,
                target.color,
                target.depth,
                pass.camera.view_projection * *model,
                frame.clear(),
            ),
            FrameCommand::SpriteBatch { texture, instances } => {
                sprite_stats =
                    sprite_renderer.prepare(device, queue, textures, *texture, instances)?;
                sprite_renderer.encode(queue, encoder, target.color, pass.camera.view_projection);
            }
        }
    }
    Ok(sprite_stats)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped PNG must be exactly the image the demo used to generate, so
    /// moving the badge onto the asset pipeline cannot change what is drawn.
    #[test]
    fn the_badge_png_decodes_to_the_authored_image() {
        let id = AssetId::new("textures/badge.png").expect("a valid asset ID");
        let decoded = TextureAssetDecoder
            .decode(AssetBytes::new(id, BADGE_PNG.to_vec()))
            .expect("the badge PNG decodes");

        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);
        assert_eq!(
            decoded.rgba8(),
            demo_badge_pixels().as_slice(),
            "the PNG and the generated image must be the same pixels"
        );
    }

    /// Scene texture references are real asset IDs, not arbitrary strings.
    #[test]
    fn every_scene_texture_reference_is_a_loadable_asset_id() {
        let document = DemoScene::authored_document().expect("the demo scene parses");
        let mut checked = 0;
        for entity in &document.entities {
            for payload in entity.components.values() {
                let Some(reference) = payload.get("texture").and_then(|value| value.as_str())
                else {
                    continue;
                };
                checked += 1;
                if let Some(generated) = reference.strip_prefix("procedural:") {
                    assert!(!generated.is_empty(), "a generated texture needs a name");
                    continue;
                }
                AssetId::new(reference).unwrap_or_else(|error| {
                    panic!("scene texture reference {reference:?} is not loadable: {error}")
                });
            }
        }
        assert!(checked > 0, "the demo scene should reference textures");
    }
}
