use orbital_last_stand::{Run, project};
use serde_json::json;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn set_script_number(run: &mut Run, entity: EntityId, name: &str, value: f32) {
    run.world
        .get_mut(entity)
        .and_then(|data| data.components.get_mut("sindri.script"))
        .and_then(|script| script.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
        .expect("the entity has authored script properties")
        .insert(name.to_owned(), json!(value));
}

#[test]
fn elites_spawn_the_reference_counter_rotating_shell_and_pulse_ring() {
    let mut run = Run::open().expect("the project opens");
    let director = run.find("Director").expect("the director exists");
    set_script_number(&mut run, director, "first_boss_at", 1000.0);
    set_script_number(&mut run, director, "elite_after", -1.0);
    set_script_number(&mut run, director, "elite_base", 1.0);
    set_script_number(&mut run, director, "elite_cap", 1.0);

    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    step(&mut run);

    let enemy = run
        .world
        .entities()
        .find(|(entity, _)| {
            run.components
                .get::<sindri_core::TagsComponent>(&run.world, *entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has("enemy"))
        })
        .map(|(entity, _)| entity)
        .expect("the forced elite spawned");
    let enemy_data = run.world.get(enemy).expect("the elite remains");
    let enemy_sides = enemy_data.components["sindri.shape"]["count"]
        .as_f64()
        .expect("the elite shape has a side count");
    let children = enemy_data.children.clone();

    let shell = children
        .iter()
        .copied()
        .find(|child| {
            run.world.get(*child).and_then(|data| data.name.as_deref()) == Some("Enemy Shell")
        })
        .expect("the elite has its polygon shell");
    let ring = children
        .iter()
        .copied()
        .find(|child| {
            run.world.get(*child).and_then(|data| data.name.as_deref()) == Some("Enemy Ring")
        })
        .expect("the elite has its pulse ring");

    let shell_data = run.world.get(shell).expect("the shell remains");
    assert_eq!(shell_data.parent, Some(enemy));
    assert_eq!(
        shell_data.components["sindri.shape"]["count"].as_f64(),
        Some(enemy_sides),
        "the shell must match the enemy silhouette"
    );
    let ring_data = run.world.get(ring).expect("the ring remains");
    assert_eq!(ring_data.parent, Some(enemy));

    let shell_rotation = shell_data
        .transform_3d
        .as_ref()
        .expect("the shell has a transform")
        .rotation;
    let ring_alpha = ring_data.components["sindri.shape"]["stroke"][3]
        .as_f64()
        .expect("the ring has an alpha");

    step(&mut run);

    let next_shell_rotation = run
        .world
        .get(shell)
        .expect("the shell remains")
        .transform_3d
        .as_ref()
        .expect("the shell has a transform")
        .rotation;
    assert_ne!(
        next_shell_rotation, shell_rotation,
        "the elite shell must rotate independently of its parent"
    );
    let next_ring_alpha = run.world.get(ring).expect("the ring remains").components["sindri.shape"]
        ["stroke"][3]
        .as_f64()
        .expect("the ring has an alpha");
    assert_ne!(
        next_ring_alpha, ring_alpha,
        "the reference elite ring must pulse"
    );
}

#[test]
fn secondary_enemy_marks_are_authored_as_non_gameplay_children() {
    let assets = project().join("assets");
    let drifter: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("prefabs/drifter.prefab.json"))
            .expect("the drifter prefab exists"),
    )
    .expect("the drifter prefab is JSON");
    let properties = &drifter["entities"][0]["components"]["sindri.script"]["properties"];
    assert_eq!(
        properties["arc_mark"].as_str(),
        Some("prefabs/enemy-arc-mark.prefab.json")
    );
    assert_eq!(
        properties["phaser_mark"].as_str(),
        Some("prefabs/phaser-mark.prefab.json")
    );
    assert_eq!(
        properties["prong_mark"].as_str(),
        Some("prefabs/enemy-prong.prefab.json")
    );

    let arc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("prefabs/enemy-arc-mark.prefab.json"))
            .expect("the arc mark prefab exists"),
    )
    .expect("the arc mark prefab is JSON");
    let arc_entity = &arc["entities"][0];
    assert_eq!(arc_entity["components"]["sindri.shape"]["kind"], "ellipse");
    assert!(arc_entity["components"].get("sindri.tags").is_none());
    assert!(
        arc_entity["components"]
            .get("sindri.physics2d.collider")
            .is_none()
    );

    let phaser: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("prefabs/phaser-mark.prefab.json"))
            .expect("the Phaser mark prefab exists"),
    )
    .expect("the Phaser mark prefab is JSON");
    let phaser_entity = &phaser["entities"][0];
    assert_eq!(
        phaser_entity["components"]["sindri.shape"]["kind"],
        "polygon"
    );
    assert_eq!(
        phaser_entity["components"]["sindri.shape"]["count"].as_f64(),
        Some(3.0)
    );
    assert!(phaser_entity["components"].get("sindri.tags").is_none());
    assert!(
        phaser_entity["components"]
            .get("sindri.physics2d.collider")
            .is_none()
    );

    let prong: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("prefabs/enemy-prong.prefab.json"))
            .expect("the radial prong prefab exists"),
    )
    .expect("the radial prong prefab is JSON");
    let prong_entity = &prong["entities"][0];
    assert_eq!(prong_entity["components"]["sindri.shape"]["kind"], "rect");
    assert!(prong_entity["components"].get("sindri.tags").is_none());
    assert!(
        prong_entity["components"]
            .get("sindri.physics2d.collider")
            .is_none()
    );

    let script = std::fs::read_to_string(assets.join("scripts/enemy-mark.decay"))
        .expect("the secondary mark script exists");
    assert!(script.contains("this.shape.sweep_turns = 0.675"));
    assert!(script.contains("this.shape.sweep_turns = 0.75"));
    assert!(script.contains("this.shape.dashes = 9.0"));
    assert!(script.contains("let cycle = this.clock % 3.4"));
    assert!(script.contains("cycle < 1.7"));
    assert!(script.contains("this.shape.stroke.a = 0.2"));
    assert!(script.contains("this.shape.stroke.a = 0.75"));
    assert!(script.contains("sin(this.clock * 12.0)"));
    assert!(script.contains("sin(this.clock * 4.0 + this.phase)"));

    let drifter_script = std::fs::read_to_string(assets.join("scripts/drifter.decay"))
        .expect("the shared enemy script exists");
    assert!(drifter_script.contains("spawn_radial_prongs(1.0, 4.0)"));
    assert!(drifter_script.contains("spawn_radial_prongs(0.0, 3.0)"));
    assert!(drifter_script.contains("spawn_arc_mark(4.0)"));
}
