#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, path::PathBuf};

    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("game/assets"));
    let manifest = sindri_assets::AssetManifest::of_directory(&root)?;
    print!("{}", manifest.to_canonical_json()?);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
