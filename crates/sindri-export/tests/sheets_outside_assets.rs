//! A sheet is found beside its texture wherever the project keeps it.
//!
//! Sheets are the one asset nothing names: they are derived from the texture's
//! own id. Finding them only under `assets/` meant a project that keeps
//! generated art elsewhere and names it from the project root shipped its
//! textures with no slices at all — and an animated sprite with no sheet draws
//! its whole sheet squeezed into one quad rather than one frame of it.

use std::path::{Path, PathBuf};

use sindri_assets::AssetKind;
use sindri_export::ProjectExport;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A one-scene project whose art lives outside `assets/`, named from the root.
fn project(name: &str) -> Scratch {
    let root = std::env::temp_dir().join(format!("sindri-sheet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let art = root.join("generated/art");
    std::fs::create_dir_all(&art).expect("the art directory is made");

    std::fs::write(
        root.join("sindri.toml"),
        "format_version = 1\n\n[project]\nname = \"Sheets\"\nmain_scene = \"main.scene.json\"\n",
    )
    .expect("the project file is written");
    // A 1x1 PNG is enough: the exporter carries bytes and never decodes them.
    std::fs::write(
        art.join("walk.png"),
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R',
            0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0, b'I', b'E',
            b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ],
    )
    .expect("the texture is written");
    std::fs::write(
        art.join("walk.sheet.json"),
        r#"{"format_version":1,"grid":{"columns":2,"rows":1,"names":["a","b"]}}"#,
    )
    .expect("the sheet is written");
    std::fs::write(
        root.join("main.scene.json"),
        r#"{
          "format_version": 9,
          "metadata": {"name": "Sheets"},
          "entities": [{
            "id": "walker",
            "name": "Walker",
            "transform_3d": {"position": [0,0,0], "rotation": [0,0,0,1], "scale": [1,1,1]},
            "components": {
              "sindri.sprite": {"texture": "generated/art/walk.png#a"}
            }
          }]
        }"#,
    )
    .expect("the scene is written");
    Scratch(root)
}

#[test]
fn a_sheet_beside_a_texture_outside_assets_is_carried() {
    let scratch = project("outside");
    let export = ProjectExport::gather(Path::new(&scratch.0)).expect("the project gathers");
    let carried = export
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Sheet)
        .map(|asset| asset.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(carried, ["generated/art/walk.sheet.json"]);
}
