use std::path::{Path, PathBuf};

use sindri_assets::AssetKind;
use sindri_export::ProjectExport;

fn last_stand() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../games/orbital-last-stand")
        .canonicalize()
        .expect("Last Stand is beside the exporter")
}

#[test]
fn a_weave_entry_carries_its_import_graph_without_duplicate_config() {
    let project = ProjectExport::gather(&last_stand()).expect("Last Stand gathers");
    let styles = project
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Other && asset.id.ends_with(".weave"))
        .map(|asset| asset.id.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "ui.weave",
        "ui/hud.weave",
        "ui/overlays.weave",
        "ui/screens.weave",
    ] {
        assert!(styles.contains(&expected), "missing {expected}: {styles:?}");
    }
}
