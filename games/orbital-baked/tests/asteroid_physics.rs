use serde_json::Value;

fn asteroid_prefab(name: &str) -> Value {
    let path = orbital_baked::project().join("assets/prefabs").join(name);
    let text = std::fs::read_to_string(&path).expect("asteroid prefab reads");
    serde_json::from_str(&text).expect("asteroid prefab parses")
}

#[test]
fn asteroids_are_solid_dynamic_bodies_that_accept_each_other() {
    for name in [
        "hazard-asteroid-large.prefab.json",
        "hazard-asteroid-medium.prefab.json",
        "hazard-asteroid-small.prefab.json",
    ] {
        let prefab = asteroid_prefab(name);
        let entity = &prefab["entities"][0];
        let components = &entity["components"];
        let body = &components["sindri.physics2d.rigid_body"];
        let collider = &components["sindri.physics2d.collider"];

        assert_eq!(body["kind"], "dynamic", "{name} is not dynamic");
        assert_eq!(collider["sensor"], false, "{name} is still a sensor");

        let memberships = collider["layers"]["memberships"]
            .as_u64()
            .expect("memberships is numeric");
        let filter = collider["layers"]["filter"]
            .as_u64()
            .expect("filter is numeric");
        assert_ne!(
            memberships & filter,
            0,
            "{name} does not accept another asteroid using the same layers"
        );

        let tags = components["sindri.tags"]["tags"]
            .as_array()
            .expect("tags are an array");
        assert!(
            tags.iter().any(|tag| tag == "asteroid"),
            "{name} is not identifiable as an asteroid"
        );
    }
}
