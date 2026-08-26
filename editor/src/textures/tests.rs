//! Where a manifest is looked for, relative to a scene.

use sindri_scene::PROCEDURAL_TEXTURES;

use super::*;

/// The rule that keeps the two kinds of texture reference apart without
/// anybody having to remember it.
#[test]
fn a_procedural_reference_cannot_be_mistaken_for_a_file() {
    for procedural in PROCEDURAL_TEXTURES {
        assert!(
            AssetId::new(procedural.reference).is_err(),
            "{} would be asked of the filesystem",
            procedural.reference
        );
    }
    assert!(AssetId::new("textures/badge.png").is_ok());
}

/// A scene's directory is where its references resolve, which is what makes
/// the same scene file work from wherever it is checked out.
#[test]
fn references_resolve_against_the_scene_s_own_directory() {
    assert_eq!(
        root_of(Some(Path::new("game/levels/one.scene.json"))),
        Some(PathBuf::from("game/levels"))
    );
    assert_eq!(root_of(None), None);
}
