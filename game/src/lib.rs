//! Gather: the companion game.
//!
//! Five orbs on a floor, a thing you drive with the arrow keys, and a row of
//! lamps that fills as you collect them. That is the whole game, and it is the
//! first thing built with this engine that someone can lose interest in for the
//! right reasons rather than the wrong ones.
//!
//! **There are no game rules in this file.** Moving, gathering, counting, and
//! winning are Decay scripts in `assets/scripts/`; this is a window, a device,
//! and a loop. That split is the claim the game exists to test: if authoring
//! gameplay meant writing Rust here, the scripting layer would not be doing its
//! job.
//!
//! Native builds embed the project so the standalone binary has no working
//! directory requirement. Browser builds deliberately do not: `browser` loads
//! the same logical IDs through `FetchAssetSource` + `AssetLoader`, which proves
//! the static-hosting path rather than proving only that `include_bytes!` works
//! in WebAssembly.

use sindri_desktop::WindowConfig;

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(not(target_arch = "wasm32"))]
mod app;
mod assets;
mod error;
mod session;

// The crate's public surface: what `bin/`, `tests/`, and the browser host
// reach for. Where an item lives inside the crate is not their business.
pub use assets::extractor;
#[cfg(not(target_arch = "wasm32"))]
pub use assets::{AUDIO, FONTS, bind_fonts, bind_textures, sources, world};
pub use error::GatherError;
pub use session::Session;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn run() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        if let Err(error) = sindri_desktop::run::<app::GatherApp>(WindowConfig {
            title: "Gather".to_owned(),
            ..WindowConfig::default()
        }) {
            log::error!("{error}");
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Info);
        if let Err(error) = sindri_desktop::run::<browser::BrowserGatherApp>(WindowConfig {
            title: "Gather".to_owned(),
            ..WindowConfig::default()
        }) {
            log::error!("{error}");
        }
    }
}
