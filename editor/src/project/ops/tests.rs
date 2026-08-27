//! What each file operation does to a directory, and what it refuses.

use std::path::{Path, PathBuf};

use super::{AssetOpError, create_folder, delete, duplicate, import, rename, split_name};

/// A project directory with a few files in it.
fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("level.scene.json"), "{}").unwrap();
    std::fs::create_dir(root.path().join("textures")).unwrap();
    std::fs::write(root.path().join("textures/orb.png"), b"png").unwrap();
    root
}

fn names(directory: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

#[test]
fn a_folder_is_made_where_it_was_asked_for() {
    let project = project();
    let made = create_folder(project.path(), project.path(), "audio").unwrap();
    assert!(made.is_dir());
    assert!(names(project.path()).contains(&"audio".to_owned()));
}

/// Nothing here overwrites. A duplicate that silently replaced the file it was
/// named after would be a delete wearing another verb.
#[test]
fn a_name_something_already_has_is_refused() {
    let project = project();
    assert!(matches!(
        create_folder(project.path(), project.path(), "textures"),
        Err(AssetOpError::Exists(_))
    ));
    assert!(matches!(
        rename(
            project.path(),
            &project.path().join("textures"),
            "level.scene.json"
        ),
        Err(AssetOpError::Exists(_))
    ));
}

/// A browser row hands over whatever was typed into it, and `../secrets` is a
/// perfectly good string — it is just not a file name.
#[test]
fn a_name_that_points_somewhere_else_is_not_a_name() {
    let project = project();
    for typed in ["../escape", "sub/dir", "", "   ", ".."] {
        assert!(
            matches!(
                create_folder(project.path(), project.path(), typed),
                Err(AssetOpError::EmptyName | AssetOpError::NotAName(_))
            ),
            "{typed:?} must not be treated as a file name"
        );
    }
}

/// Nothing the browser offers may reach outside the directory it is showing.
#[test]
fn a_target_outside_the_project_is_refused() {
    let project = project();
    let elsewhere = tempfile::tempdir().unwrap();
    let stranger = elsewhere.path().join("other.png");
    std::fs::write(&stranger, b"png").unwrap();

    assert!(matches!(
        delete(project.path(), &stranger),
        Err(AssetOpError::OutsideProject)
    ));
    assert!(matches!(
        rename(project.path(), &stranger, "renamed.png"),
        Err(AssetOpError::OutsideProject)
    ));
    assert!(stranger.exists(), "and it is still there");
}

/// A rename keeps a file where it is: the new name is joined to the same
/// parent rather than treated as a path.
#[test]
fn renaming_keeps_the_file_in_its_own_folder() {
    let project = project();
    let renamed = rename(
        project.path(),
        &project.path().join("textures/orb.png"),
        "pip.png",
    )
    .unwrap();
    assert_eq!(renamed, project.path().join("textures/pip.png"));
    assert_eq!(names(&project.path().join("textures")), vec!["pip.png"]);
}

/// A copy keeps the suffix that says what kind of asset it is.
///
/// `file_stem` stops at the last dot, so a scene duplicated through it would
/// become `level.scene copy.json` — a file the browser no longer reads as a
/// scene and the editor can no longer open from a row.
#[test]
fn a_copy_is_still_the_same_kind_of_file() {
    let project = project();
    let copy = duplicate(project.path(), &project.path().join("level.scene.json")).unwrap();
    assert_eq!(
        copy.file_name().unwrap(),
        "level copy.scene.json",
        "a copied scene is still a scene"
    );

    let again = duplicate(project.path(), &project.path().join("level.scene.json")).unwrap();
    assert_eq!(again.file_name().unwrap(), "level copy 2.scene.json");
}

#[test]
fn splitting_a_name_keeps_the_whole_suffix() {
    let cases = [
        ("level.scene.json", ("level", ".scene.json")),
        ("tiles.sheet.json", ("tiles", ".sheet.json")),
        ("orb.png", ("orb", ".png")),
        ("README", ("README", "")),
        (".gitignore", (".gitignore", "")),
    ];
    for (name, (stem, suffix)) in cases {
        assert_eq!(
            split_name(&PathBuf::from(name)),
            (stem.to_owned(), suffix.to_owned()),
            "{name} split wrongly"
        );
    }
}

/// A folder is duplicated with everything under it.
#[test]
fn duplicating_a_folder_takes_its_contents() {
    let project = project();
    let copy = duplicate(project.path(), &project.path().join("textures")).unwrap();
    assert_eq!(copy.file_name().unwrap(), "textures copy");
    assert_eq!(names(&copy), vec!["orb.png"]);
}

/// Deleting a folder takes what is in it, which is why the panel asks first.
#[test]
fn deleting_a_folder_removes_what_is_under_it() {
    let project = project();
    delete(project.path(), &project.path().join("textures")).unwrap();
    assert_eq!(names(project.path()), vec!["level.scene.json"]);
}

/// An import that would overwrite is skipped and reported, rather than failing
/// the whole batch: choosing eight images and losing all of them because one
/// shares a name is not a useful answer.
#[test]
fn an_import_brings_in_what_it_can_and_names_what_it_could_not() {
    let project = project();
    let elsewhere = tempfile::tempdir().unwrap();
    let fresh = elsewhere.path().join("pip.png");
    let clashing = elsewhere.path().join("level.scene.json");
    std::fs::write(&fresh, b"png").unwrap();
    std::fs::write(&clashing, "{}").unwrap();

    let (arrived, refused) = import(
        project.path(),
        project.path(),
        &[fresh.clone(), clashing.clone()],
    );

    assert_eq!(arrived, vec![project.path().join("pip.png")]);
    assert_eq!(refused.len(), 1);
    assert!(matches!(refused[0], AssetOpError::Exists(_)));
    assert_eq!(
        std::fs::read_to_string(project.path().join("level.scene.json")).unwrap(),
        "{}",
        "and the file it would have replaced is untouched"
    );
}
