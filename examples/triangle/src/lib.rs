//! The smallest thing Sindri can draw, on a desktop and in a browser.
//!
//! There is no window, event loop, or device request here. `sindri-desktop`
//! owns those, which is what leaves this example short enough to read as the
//! proof it is meant to be: build a renderer, encode a pass.

use std::convert::Infallible;

use sindri_desktop::{AppContext, DesktopApp, WindowConfig};
use sindri_render::TriangleRenderer;

struct Triangle {
    renderer: TriangleRenderer,
}

impl DesktopApp for Triangle {
    type Error = Infallible;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            renderer: TriangleRenderer::new(context.device(), context.format()),
        })
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri triangle encoder"),
                });
        self.renderer.encode(&mut encoder, view);
        context.queue().submit([encoder.finish()]);
        Ok(())
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

    if let Err(error) =
        sindri_desktop::run::<Triangle>(WindowConfig::new("Sindri — shared native/web triangle"))
    {
        log::error!("{error}");
    }
}
