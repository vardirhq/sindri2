use std::path::Path;

use sindri_core::SceneDocument;

use super::{FORMAT_VERSION, MANIFEST_NAME, Project, ProjectError, is_project, root_for};

/// A project made the way the welcome window makes one.
fn created(root: &Path, name: &str) -> Project {
    Project::create(root, name, &SceneDocument::default())
        .expect("a fresh directory takes a project")
}

#[test]
fn a_directory_without_a_manifest_is_not_a_project() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(directory.path().join("level.scene.json"), "{}").expect("a scene file");
    assert!(
        !is_project(directory.path()),
        "a folder with a scene in it is a folder, not a project"
    );
    assert!(matches!(
        Project::open(directory.path()),
        Err(ProjectError::NotAProject { .. })
    ));
}

#[test]
fn creating_a_project_writes_a_manifest_a_scene_and_somewhere_to_put_assets() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    let project = created(&root, "My Game");

    assert!(
        root.join(MANIFEST_NAME).is_file(),
        "the manifest is written"
    );
    assert!(
        root.join("main.scene.json").is_file(),
        "a project with no scene is a project the editor opens on nothing"
    );
    for expected in ["textures", "scripts", "fonts"] {
        assert!(
            root.join(expected).is_dir(),
            "{expected}/ is where assets resolve from"
        );
    }
    assert_eq!(project.name(), "My Game");
    assert_eq!(project.root(), root);
}

#[test]
fn a_created_project_opens_on_the_scene_it_made() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    let project = created(&root, "My Game");
    assert_eq!(
        project.main_scene(),
        Some(root.join("main.scene.json")),
        "creating a project nominates its scene, or opening it would ask which"
    );
}

#[test]
fn what_is_written_is_what_comes_back() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    let written = created(&root, "My Game");
    let read = Project::open(&root).expect("a created project opens");
    assert_eq!(read, written);
}

#[test]
fn a_project_is_named_by_its_manifest_rather_than_by_its_folder() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("assets");
    let project = created(&root, "Gather");
    assert_eq!(
        Project::open(&root).expect("it opens").name(),
        "Gather",
        "the game is called Gather even though the folder is called assets"
    );
    assert_eq!(project.name(), "Gather");
}

#[test]
fn a_project_is_not_created_over_a_project() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    created(&root, "My Game");
    assert!(
        matches!(
            Project::create(&root, "Something Else", &SceneDocument::default()),
            Err(ProjectError::AlreadyAProject { .. })
        ),
        "creating over a project would overwrite a name somebody chose"
    );
    assert_eq!(
        Project::open(&root).expect("it opens").name(),
        "My Game",
        "and the refusal leaves the first project as it was"
    );
}

#[test]
fn a_project_needs_a_name() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    assert!(matches!(
        Project::create(&root, "   ", &SceneDocument::default()),
        Err(ProjectError::Unnamed)
    ));
    assert!(
        !root.exists(),
        "a refused creation leaves nothing behind to open"
    );
}

#[test]
fn a_project_from_a_newer_editor_is_refused_rather_than_guessed_at() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path();
    std::fs::write(
        root.join(MANIFEST_NAME),
        format!(
            "format_version = {}\n\n[project]\nname = \"From The Future\"\n",
            FORMAT_VERSION + 1
        ),
    )
    .expect("a manifest");
    assert!(matches!(
        Project::open(root),
        Err(ProjectError::FromTheFuture { .. })
    ));
}

#[test]
fn a_manifest_that_is_not_a_manifest_says_so() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path();
    std::fs::write(root.join(MANIFEST_NAME), "this is not toml =").expect("a manifest");
    assert!(matches!(
        Project::open(root),
        Err(ProjectError::Malformed { .. })
    ));
}

#[test]
fn a_nominated_scene_that_is_gone_opens_nothing_rather_than_something_else() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    let project = created(&root, "My Game");
    std::fs::remove_file(root.join("main.scene.json")).expect("the scene is removed");
    std::fs::write(root.join("other.scene.json"), "{}").expect("another scene");
    assert_eq!(
        project.main_scene(),
        None,
        "standing another scene in for the named one would read as though it loaded"
    );
}

#[test]
fn nominating_a_scene_stores_it_relative_to_the_project() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    let mut project = created(&root, "My Game");
    let scene = root.join("levels").join("two.scene.json");
    std::fs::create_dir_all(scene.parent().expect("a parent")).expect("a levels folder");
    std::fs::write(&scene, "{}").expect("a scene");

    project
        .set_main_scene(&scene)
        .expect("it is inside the project");
    assert_eq!(project.main_scene(), Some(scene));
    let text = std::fs::read_to_string(root.join(MANIFEST_NAME)).expect("the manifest");
    assert!(
        text.contains("levels/two.scene.json"),
        "the path is stored project-relative and with forward slashes, so a \
         project checked out on another platform still finds it: {text}"
    );
}

#[test]
fn a_scene_outside_the_project_cannot_be_nominated() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    let mut project = created(&root, "My Game");
    let outside = directory.path().join("elsewhere.scene.json");
    std::fs::write(&outside, "{}").expect("a scene");
    assert!(
        project.set_main_scene(&outside).is_err(),
        "the field is relative to the root, so a path escaping it names nothing"
    );
}

#[test]
fn a_scene_deep_inside_a_project_still_finds_it() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("my-game");
    created(&root, "My Game");
    let scene = root.join("levels").join("act-one").join("two.scene.json");
    std::fs::create_dir_all(scene.parent().expect("a parent")).expect("the folders");
    std::fs::write(&scene, "{}").expect("a scene");

    assert_eq!(
        root_for(&scene).as_deref(),
        Some(root.as_path()),
        "opening a scene from the command line opens it as its project"
    );
}

#[test]
fn a_scene_in_no_project_belongs_to_no_project() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let scene = directory.path().join("loose.scene.json");
    std::fs::write(&scene, "{}").expect("a scene");
    assert_eq!(root_for(&scene), None);
}
