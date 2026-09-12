//! A project with more than one scene ships all of them, and everything they
//! name.
//!
//! The failure this exists to stop is quiet. A second scene reached only by
//! `Scene.go("house")` is a string inside a program, so nothing can find it by
//! walking the first scene — and listing it under `[assets] include` would ship
//! the scene file and none of its textures, which is an export that looks
//! complete and opens a door onto nothing.

use std::path::{Path, PathBuf};

use sindri_assets::AssetKind;
use sindri_export::ProjectExport;

/// A throwaway project on disk. Written out rather than borrowed from a real
/// game, because no shipped project has a second scene yet and the point is to
/// test the shape rather than a particular farm.
struct Project(PathBuf);

impl Project {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("sindri-scenes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("assets/textures")).expect("a project directory");
        Self(path)
    }

    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(path, bytes).expect("the file writes");
    }

    /// A scene holding one sprite, so it names exactly one texture.
    fn scene(&self, relative: &str, texture: &str) {
        self.write(
            relative,
            format!(
                r#"{{
                  "format_version": 9,
                  "metadata": {{ "name": "A scene" }},
                  "entities": [
                    {{
                      "id": "thing",
                      "name": "Thing",
                      "components": {{
                        "sindri.sprite": {{
                          "texture": "{texture}",
                          "tint": [1.0, 1.0, 1.0, 1.0],
                          "layer": 0
                        }}
                      }}
                    }}
                  ]
                }}"#
            ),
        );
    }

    /// A one-pixel PNG, which is enough for the exporter to carry bytes.
    fn texture(&self, relative: &str) {
        const PIXEL: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        self.write(relative, PIXEL);
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project whose manifest lists a second scene, each scene naming its own
/// texture.
fn two_scene_project(name: &str, extra: &str) -> Project {
    let project = Project::new(name);
    project.write(
        "sindri.toml",
        format!(
            r#"format_version = 1

[project]
name = "Two Places"
main_scene = "assets/outside.scene.json"
scenes = {extra}
"#
        ),
    );
    project.scene("assets/outside.scene.json", "textures/outside.png");
    project.scene("assets/inside.scene.json", "textures/inside.png");
    project.texture("assets/textures/outside.png");
    project.texture("assets/textures/inside.png");
    project
}

fn ids(export: &ProjectExport, kind: AssetKind) -> Vec<String> {
    export
        .assets
        .iter()
        .filter(|asset| asset.kind == kind)
        .map(|asset| asset.id.clone())
        .collect()
}

#[test]
fn every_declared_scene_ships() {
    let project = two_scene_project("both", r#"["assets/inside.scene.json"]"#);
    let export = ProjectExport::gather(project.path()).expect("it gathers");
    let scenes = ids(&export, AssetKind::Scene);
    assert!(
        scenes.contains(&"outside.scene.json".to_owned()),
        "{scenes:?}"
    );
    assert!(
        scenes.contains(&"inside.scene.json".to_owned()),
        "{scenes:?}"
    );
}

/// The whole point. A scene that shipped without its own textures would draw
/// nothing, and the export would have looked like it worked.
#[test]
fn a_second_scene_brings_its_own_assets() {
    let project = two_scene_project("assets", r#"["assets/inside.scene.json"]"#);
    let export = ProjectExport::gather(project.path()).expect("it gathers");
    let textures = ids(&export, AssetKind::Texture);
    assert!(
        textures.contains(&"textures/inside.png".to_owned()),
        "the second scene's texture did not ship: {textures:?}"
    );
}

/// Which scene a host opens on cannot be "the first one gathered" once more
/// than one is shipping.
#[test]
fn the_main_scene_is_the_one_the_project_names() {
    let project = two_scene_project("main", r#"["assets/inside.scene.json"]"#);
    let export = ProjectExport::gather(project.path()).expect("it gathers");
    assert_eq!(export.scene_id(), Some("outside.scene.json"));
}

/// Listing the main scene among the others is someone being explicit, not a
/// mistake — but shipping it twice would be.
#[test]
fn a_scene_listed_twice_ships_once() {
    let project = two_scene_project(
        "twice",
        r#"["assets/outside.scene.json", "assets/inside.scene.json"]"#,
    );
    let export = ProjectExport::gather(project.path()).expect("it gathers");
    let scenes = ids(&export, AssetKind::Scene);
    assert_eq!(
        scenes
            .iter()
            .filter(|id| *id == "outside.scene.json")
            .count(),
        1,
        "{scenes:?}"
    );
}

/// A project that declares none still works exactly as it did, which is most of
/// them.
#[test]
fn a_project_with_one_scene_is_unchanged() {
    let project = two_scene_project("one", "[]");
    let export = ProjectExport::gather(project.path()).expect("it gathers");
    assert_eq!(ids(&export, AssetKind::Scene), vec!["outside.scene.json"]);
    let textures = ids(&export, AssetKind::Texture);
    assert!(
        !textures.contains(&"textures/inside.png".to_owned()),
        "an undeclared scene's assets should not ship: {textures:?}"
    );
}

#[test]
fn a_declared_scene_that_is_not_there_is_reported() {
    let project = two_scene_project("missing", r#"["assets/attic.scene.json"]"#);
    let error = ProjectExport::gather(project.path()).expect_err("it should refuse");
    assert!(
        format!("{error}").contains("attic"),
        "the error should name the scene it could not read: {error}"
    );
}
