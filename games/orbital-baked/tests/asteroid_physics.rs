use orbital_baked::Run;
use serde_json::Value;

const STEP: f32 = 1.0 / 60.0;

fn asteroid_prefab(name: &str) -> Value {
    let path = orbital_baked::project().join("assets/prefabs").join(name);
    let text = std::fs::read_to_string(&path).expect("asteroid prefab reads");
    serde_json::from_str(&text).expect("asteroid prefab parses")
}

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
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

#[test]
fn two_asteroids_actually_bounce_in_the_game_runtime() {
    let mut run = Run::open_scene("combat-lab.scene.json").expect("combat lab opens");
    for _ in 0..3 {
        step(&mut run);
    }
    run.set_board("lab_autoplay", 0.0);
    run.set_board("run_state", 1.0);
    run.set_board("sector", 1.0);

    let prefab = run
        .prefabs
        .get("prefabs/hazard-asteroid-large.prefab.json")
        .expect("large asteroid prefab ships")
        .clone();
    let left = run
        .world
        .spawn_prefab(&prefab)
        .expect("left asteroid spawns")
        .root;
    let right = run
        .world
        .spawn_prefab(&prefab)
        .expect("right asteroid spawns")
        .root;

    run.world
        .get_mut(left)
        .expect("left asteroid remains")
        .transform_3d
        .as_mut()
        .expect("left asteroid has a transform")
        .set_position_2d([-1.25, 3.0]);
    run.world
        .get_mut(right)
        .expect("right asteroid remains")
        .transform_3d
        .as_mut()
        .expect("right asteroid has a transform")
        .set_position_2d([1.25, 3.0]);

    // One pass materializes both authored dynamic bodies. Their start scripts
    // assign ordinary drift, which this controlled test then replaces with a
    // head-on approach so a real physical response is unavoidable.
    step(&mut run);
    run.physics
        .world_mut()
        .set_linear_velocity(left, [2.0, 0.0])
        .expect("left asteroid accepts velocity");
    run.physics
        .world_mut()
        .set_linear_velocity(right, [-2.0, 0.0])
        .expect("right asteroid accepts velocity");

    let mut bounced = false;
    for _ in 0..90 {
        step(&mut run);
        let left_vx = run
            .physics
            .world()
            .linear_velocity(left)
            .expect("left asteroid stays physical")[0];
        let right_vx = run
            .physics
            .world()
            .linear_velocity(right)
            .expect("right asteroid stays physical")[0];
        if left_vx < -0.05 && right_vx > 0.05 {
            bounced = true;
            break;
        }
    }

    assert!(
        bounced,
        "two solid asteroid bodies crossed without exchanging momentum"
    );
}
