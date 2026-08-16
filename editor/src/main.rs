#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    sindri_editor::native::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
