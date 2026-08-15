//! Checks that a captured image shows the colours the demo scene authors.
//!
//! The headless capture verifies its own pixels before writing them, but the
//! editor screenshot had nothing looking at it, which is how the editor's
//! viewport spent a release sampling its colour target through a view that
//! decoded a second time: every check passed and only the picture was wrong.
//!
//! This reads back an image CI has already produced and holds it to the same
//! expectation, so the two renders of one scene cannot disagree unnoticed.
//!
//! ```bash
//! cargo run -p sindri-cube --bin verify -- target/render-artifacts/editor.png
//! ```

#[cfg(not(target_arch = "wasm32"))]
fn verify(path: &str) -> Result<String, String> {
    use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
    use sindri_core::AssetId;

    let bytes = std::fs::read(path).map_err(|error| format!("could not read {path}: {error}"))?;

    // Decoded through the same path a game loads a texture with, rather than a
    // second image reader that could disagree about what it read.
    let id = AssetId::new("captures/image.png").expect("a fixed literal asset ID is valid");
    let image = TextureAssetDecoder
        .decode(AssetBytes::new(id, bytes))
        .map_err(|error| format!("could not decode {path}: {error}"))?;

    sindri_cube::verify_authored_colors(image.rgba8())
        .map_err(|error| format!("{path} is not the colour the scene authored.\n{error}"))?;
    Ok(format!(
        "verified authored scene colours in {path} ({}x{})",
        image.width(),
        image.height()
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let Some(path) = std::env::args().nth(1) else {
        panic!("usage: verify IMAGE.png");
    };
    match verify(&path) {
        Ok(message) => println!("{message}"),
        Err(error) => panic!("{error}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
