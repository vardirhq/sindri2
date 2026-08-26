//! Noticing that a browser tab went away, so the frame clock does not
//! come back with a delta of several minutes.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use winit::event_loop::EventLoopProxy;

#[cfg(target_arch = "wasm32")]
use super::startup::Startup;

#[cfg(target_arch = "wasm32")]
pub(super) struct VisibilityListener {
    document: web_sys::Document,
    _callback: Closure<dyn FnMut()>,
}

#[cfg(target_arch = "wasm32")]
impl VisibilityListener {
    pub(super) fn new(proxy: EventLoopProxy<Startup>) -> Option<Self> {
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

    pub(super) fn visible(&self) -> bool {
        !self.document.hidden()
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for VisibilityListener {
    fn drop(&mut self) {
        self.document.set_onvisibilitychange(None);
    }
}
