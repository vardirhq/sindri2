#[cfg(not(target_arch = "wasm32"))]
fn main() {
    sindri_triangle::run();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

