//! Keeping a browser canvas the size of the page it is on.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;

#[cfg(target_arch = "wasm32")]
use super::startup::Startup;

/// How big the page is, in the logical pixels a window is asked for.
///
/// `None` when there is no document to ask, which is every native build and a
/// browser one being torn down.
#[cfg(target_arch = "wasm32")]
pub(super) fn page_size() -> Option<(f64, f64)> {
    let window = web_sys::window()?;
    let width = window.inner_width().ok()?.as_f64()?;
    let height = window.inner_height().ok()?.as_f64()?;
    (width > 0.0 && height > 0.0).then_some((width, height))
}

/// Watches the page for a size change and says so.
///
/// A window opens at whatever size its project asked for, which on a desktop is
/// a window someone can drag and in a browser is a fixed rectangle in the
/// middle of a page. A phone held in portrait showed a game a letterbox: the
/// canvas kept the 960 by 540 it was born with, and the whole screen around it
/// stayed empty. A page is the window in a browser, so the canvas is the page.
///
/// Rotating a phone is a resize like any other, which is why this is a listener
/// and not something read once at startup.
#[cfg(target_arch = "wasm32")]
pub(super) struct PageSizeListener {
    window: web_sys::Window,
    _callback: Closure<dyn FnMut()>,
}

#[cfg(target_arch = "wasm32")]
impl PageSizeListener {
    pub(super) fn new(proxy: EventLoopProxy<Startup>) -> Option<Self> {
        let window = web_sys::window()?;
        let callback = Closure::wrap(Box::new(move || {
            let Some((width, height)) = page_size() else {
                return;
            };
            if proxy
                .send_event(Startup::PageResized(width, height))
                .is_err()
            {
                log::debug!("the page resized after the event loop closed");
            }
        }) as Box<dyn FnMut()>);
        window.set_onresize(Some(callback.as_ref().unchecked_ref()));
        Some(Self {
            window,
            _callback: callback,
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for PageSizeListener {
    fn drop(&mut self) {
        self.window.set_onresize(None);
    }
}
