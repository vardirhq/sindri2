//! The Weave presentation a project asks the editor to show.
//!
//! A `.weave` file sitting in the project is not necessarily an independent
//! stylesheet: it may be imported by another one. The manifest's
//! `[assets].include` list names the roots, and the language's own `@use` graph
//! names everything below them. Loading those two answers here keeps Scene and
//! Game views aligned with export/browser composition instead of applying every
//! file the project browser happens to find.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sindri_core::World;
use sindri_weave::PresentationWorld;
use weave::{Stylesheet, Viewport};

use crate::project::Project;

/// The composed stylesheets a project presents through.
#[derive(Clone, Debug, Default)]
pub struct ProjectStyles {
    sheets: Vec<Stylesheet>,
}

impl ProjectStyles {
    /// Reads and composes every `.weave` root explicitly included by a project.
    ///
    /// A project with no Weave roots is not an error; its authored UI is its
    /// presentation. A malformed or missing declared root is an error, because
    /// silently falling back there would make the editor disagree with export.
    pub fn load(project: &Project) -> Result<Self, String> {
        let roots = project
            .included_assets()
            .iter()
            .filter(|id| {
                Path::new(id)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("weave"))
            })
            .cloned()
            .collect::<Vec<_>>();
        if roots.is_empty() {
            return Ok(Self::default());
        }

        let mut sources = BTreeMap::new();
        let mut walked = BTreeSet::new();
        for root in &roots {
            gather_sources(project.root(), root, &mut sources, &mut walked)?;
        }

        // Compose only the manifest roots. `compose_all` over every source in
        // the import graph would also return imported files as roots, which is
        // exactly the double-application this loader exists to avoid.
        let mut sheets = Vec::with_capacity(roots.len());
        for root in roots {
            sheets
                .push(weave::compose(&root, &sources).map_err(|error| format!("{root}: {error}"))?);
        }
        Ok(Self { sheets })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sheets.len()
    }

    /// Resolves a disposable presented world for one viewport.
    ///
    /// Each root is applied in manifest order, matching the host's sequential
    /// stylesheet semantics. The authored world is never mutated, so changing
    /// a viewport or closing the editor cannot leak presentation into a save.
    pub fn resolve(&self, authored: &World, viewport: Viewport) -> Result<World, String> {
        let mut world = authored.clone();
        for sheet in &self.sheets {
            let presented = PresentationWorld::resolve(&world, sheet, viewport)
                .map_err(|error| error.to_string())?;
            world = presented.world().clone();
        }
        Ok(world)
    }
}

fn gather_sources(
    project: &Path,
    id: &str,
    sources: &mut BTreeMap<String, String>,
    walked: &mut BTreeSet<String>,
) -> Result<(), String> {
    if !walked.insert(id.to_owned()) {
        return Ok(());
    }
    let path = resolve(project, id);
    let source =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let imports = weave::imports(&source).map_err(|error| format!("{id}: {error}"))?;
    sources.insert(id.to_owned(), source);

    for reference in imports {
        let dependency =
            weave::resolve_import(id, &reference).map_err(|error| format!("{id}: {error}"))?;
        gather_sources(project, &dependency, sources, walked)?;
    }
    Ok(())
}

/// Resolves one logical asset ID exactly where project export does: under the
/// conventional `assets/` directory first, and at the root for flatter project
/// layouts. Editor presentation and a shipped build therefore read the same
/// source for the same ID instead of merely agreeing about the language.
fn resolve(project: &Path, id: &str) -> PathBuf {
    let in_assets = project.join("assets").join(id);
    if in_assets.exists() {
        in_assets
    } else {
        project.join(id)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use sindri_core::{EntityData, SceneEntityId, Transform3D, World};
    use weave::Viewport;

    use super::ProjectStyles;
    use crate::project::{MANIFEST_NAME, Project};

    fn project_with_manifest(manifest: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("a temporary project");
        fs::write(directory.path().join(MANIFEST_NAME), manifest).expect("the manifest");
        directory
    }

    #[test]
    fn no_weave_root_means_authored_presentation() {
        let directory =
            project_with_manifest("format_version = 1\n\n[project]\nname = \"Plain\"\n");
        let project = Project::open(directory.path()).expect("project opens");
        let styles = ProjectStyles::load(&project).expect("no style is valid");
        assert!(styles.is_empty());
    }

    #[test]
    fn manifest_roots_follow_imports_without_applying_imports_twice() {
        let directory = project_with_manifest(
            "format_version = 1\n\n[project]\nname = \"Styled\"\n\n[assets]\ninclude = [\"ui.weave\"]\n",
        );
        let assets = directory.path().join("assets");
        fs::create_dir_all(assets.join("ui")).expect("style directory");
        fs::write(
            assets.join("ui.weave"),
            "@use \"ui/base.weave\";\n#panel { width: 50vw; }",
        )
        .expect("entry style");
        fs::write(assets.join("ui/base.weave"), ".card { height: 25vh; }")
            .expect("imported style");

        let project = Project::open(directory.path()).expect("project opens");
        let styles = ProjectStyles::load(&project).expect("styles compose");
        assert_eq!(styles.len(), 1, "only the manifest entry is a root");

        let mut world = World::default();
        world.spawn(EntityData {
            source_id: Some(SceneEntityId::new("panel").expect("id")),
            transform_3d: Some(Transform3D::default()),
            components: std::collections::BTreeMap::from([(
                "weave.style".to_owned(),
                json!({ "classes": ["card"] }),
            )]),
            ..EntityData::default()
        });
        let presented = styles
            .resolve(
                &world,
                Viewport {
                    width: 1000.0,
                    height: 500.0,
                },
            )
            .expect("presentation resolves");
        let data = presented.entities().next().expect("entity").1;
        let scale = data.transform_3d.expect("transform").scale_2d();
        assert!((scale[0] - 2.0).abs() < 1.0e-6, "50vw at 2:1 is 2.0");
        assert!((scale[1] - 0.5).abs() < 1.0e-6, "25vh is 0.5");
    }

    #[test]
    fn a_missing_declared_root_is_reported() {
        let directory = project_with_manifest(
            "format_version = 1\n\n[project]\nname = \"Broken\"\n\n[assets]\ninclude = [\"missing.weave\"]\n",
        );
        let project = Project::open(directory.path()).expect("project opens");
        let error = ProjectStyles::load(&project).expect_err("missing style must be visible");
        assert!(error.contains("missing.weave"));
    }
}
