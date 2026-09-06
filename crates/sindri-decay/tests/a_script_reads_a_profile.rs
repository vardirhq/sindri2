use std::collections::BTreeMap;

use serde_json::json;
use sindri_core::{ComponentSchemaRegistry, EntityData, ProfileDocument, SceneComponent, World};
use sindri_decay::{ProfileSources, ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;

#[test]
fn a_typed_profile_export_reads_scalars_and_records() {
    let mut components = ComponentSchemaRegistry::default();
    components.register::<ScriptComponent>("Script").unwrap();
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        components: BTreeMap::from([(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({
                "source": "reader.decay",
                "script": "Reader",
                "properties": {"catalog": "profiles/modules.profile.json"}
            }),
        )]),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "reader.decay",
        r#"
        script Reader {
            @export let catalog: Profile;
            fn start() {
                Game.set("weight", Profiles.number_at(this.catalog, "modules", 0.0, "weight", 1.0));
                Game.set("count", Profiles.count(this.catalog, "modules"));
                if Profiles.text_at(this.catalog, "modules", 0.0, "name", "") == "Hot Core" {
                    Game.set("named", 1.0);
                }
                if Profiles.name(this.catalog) == "Modules" { Game.set("profile_name", 1.0); }
                if Profiles.kind(this.catalog) == "module_catalog" { Game.set("kind", 1.0); }
                if Profiles.text(this.catalog, "theme", "") == "orbital" {
                    Game.set("theme", 1.0);
                }
                Game.set("threshold", Profiles.number(this.catalog, "threshold", 0.0));
                if Profiles.flag(this.catalog, "enabled", false) { Game.set("enabled", 1.0); }
            }
        }
        "#,
    );
    let profile = ProfileDocument::from_json(
        r#"{
          "format_version": 1,
          "name": "Modules",
          "type": "module_catalog",
          "values": {
            "enabled": true,
            "theme": "orbital",
            "threshold": 3.5,
            "modules": [{"name": "Hot Core", "weight": 2.0}]
          }
        }"#,
    )
    .unwrap();
    let mut profiles = ProfileSources::new();
    profiles.insert("profiles/modules.profile.json", profile);
    let mut scripts = Scripts::new();
    let report = scripts.advance(
        &mut world,
        &components,
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0).with_profiles(&profiles),
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(scripts.is_running(entity));
    let board = scripts.blackboard();
    let close = |name: &str, expected: f64| {
        let actual = board.get(name, 0.0);
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "{name}: expected {expected}, got {actual}"
        );
    };
    close("weight", 2.0);
    close("count", 1.0);
    close("named", 1.0);
    close("profile_name", 1.0);
    close("kind", 1.0);
    close("theme", 1.0);
    close("threshold", 3.5);
    close("enabled", 1.0);
}
