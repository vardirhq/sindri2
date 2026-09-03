//! Opening the window and getting a device, which is asynchronous in a
//! browser and routed back through the event loop.

use std::{future::Future, sync::Arc};

use sindri_gpu::{GpuContext, GpuError, GpuRequestOptions, WindowSurface};
use winit::{event_loop::EventLoop, window::Window};

use super::Host;
use super::app::{DesktopApp, DesktopError, WindowConfig};

/// The event name a browser host dispatches on `window` when startup or
/// gameplay fails.
///
/// Public because a page has to listen for something, and a name a page has to
/// guess is a name that goes stale silently.
#[cfg(target_arch = "wasm32")]
pub const FAILURE_EVENT: &str = "sindri:failed";

/// Tells the page a failure happened, with the message that describes it.
///
/// Best effort by design. Every step here can fail in a document that is being
/// torn down, and a failure to report a failure must not become the failure
/// anyone sees — the log line above has already recorded it either way.
#[cfg(target_arch = "wasm32")]
pub(super) fn announce_failure(message: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let detail = web_sys::CustomEventInit::new();
    detail.set_detail(&wasm_bindgen::JsValue::from_str(message));
    detail.set_bubbles(true);
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(FAILURE_EVENT, &detail) {
        let _ = window.dispatch_event(&event);
    }
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
pub(super) fn spawn(future: impl Future<Output = ()> + 'static) {
    pollster::block_on(future);
}

#[cfg(target_arch = "wasm32")]
pub(super) fn spawn(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

/// The device request, which is the only genuinely asynchronous part of startup.
pub(super) async fn open_surface(
    display: winit::event_loop::OwnedDisplayHandle,
    window: Arc<Window>,
) -> Result<(GpuContext, WindowSurface), GpuError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
        Box::new(display),
    ));
    // How to build the surface, so a lost one can be built again the same way.
    let target = Arc::clone(&window);
    let source = move |instance: &wgpu::Instance| instance.create_surface(Arc::clone(&target));
    let surface = source(&instance)?;
    let gpu = GpuContext::request(&instance, Some(&surface), &GpuRequestOptions::default()).await?;
    let size = window.inner_size();
    let surface = WindowSurface::new(instance, surface, source, &gpu, size.width, size.height)?;
    Ok((gpu, surface))
}

/// What asynchronous platform work sends back into the event loop.
pub(super) enum Startup {
    Opened(Result<(GpuContext, WindowSurface), GpuError>),
    #[cfg(target_arch = "wasm32")]
    VisibilityChanged(bool),
    /// The page changed size, which in a browser is the window changing size.
    ///
    /// Logical pixels, because that is what a window is asked for; the browser
    /// applies the device's pixel ratio itself, so a phone gets the sharp
    /// surface it deserves without anything here knowing what a phone is.
    #[cfg(target_arch = "wasm32")]
    PageResized(f64, f64),
}
