use orbital_last_stand::project;

#[test]
fn elite_identity_marks_match_the_reference_trait_family() {
    let assets = project().join("assets");
    let ring: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("prefabs/enemy-ring.prefab.json"))
            .expect("the elite pulse ring prefab exists"),
    )
    .expect("the elite pulse ring prefab is JSON");
    let properties = &ring["entities"][0]["components"]["sindri.script"]["properties"];
    assert_eq!(
        properties["identity_ring"].as_str(),
        Some("prefabs/elite-identity-ring.prefab.json")
    );
    assert_eq!(
        properties["identity_crack"].as_str(),
        Some("prefabs/elite-identity-crack.prefab.json")
    );

    for path in [
        "prefabs/elite-identity-ring.prefab.json",
        "prefabs/elite-identity-crack.prefab.json",
    ] {
        let prefab: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(assets.join(path)).expect("the identity prefab exists"),
        )
        .expect("the identity prefab is JSON");
        let entity = &prefab["entities"][0];
        assert!(entity["components"].get("sindri.tags").is_none());
        assert!(
            entity["components"]
                .get("sindri.physics2d.collider")
                .is_none()
        );
    }

    let visual = std::fs::read_to_string(assets.join("scripts/elite-visual.decay"))
        .expect("the elite visual script exists");
    assert!(visual.contains("if this.trait == 1.0 { return; }"));
    assert!(visual.contains("while slot < 3.0"));
    assert!(visual.contains("World.spawn(this.identity_crack)"));
    assert!(visual.contains("spawn_identity_ring(0.0)"));

    let identity = std::fs::read_to_string(assets.join("scripts/elite-identity.decay"))
        .expect("the elite identity script exists");
    assert!(identity.contains("this.shape.sweep_turns = 0.207"));
    assert!(identity.contains("this.shape.fill.a = 0.55 + 0.35 * pulse"));
    assert!(identity.contains("this.shape.stroke.a = 0.18 + 0.22 * pulse"));
    assert!(identity.contains("this.index * 1.05"));
}
