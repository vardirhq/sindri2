use orbital_last_stand::Run;
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
    let enemy_sides = run.world.get(enemy).expect("the elite remains").components
        ["sindri.shape"]["count"]
        .as_f64()
        .expect("the elite shape has a side count");
    let children = run
        .world
        .get(enemy)
        .expect("the elite remains")
        .children
        .clone();

    let shell = children
        .iter()
        .copied()
        .find(|child| {
            run.world
                .get(*child)
                .and_then(|data| data.name.as_deref())
                == Some("Enemy Shell")
        })
        .expect("the elite has its polygon shell");
    let ring = children
        .iter()
        .copied()
        .find(|child| {
            run.world
                .get(*child)
                .and_then(|data| data.name.as_deref())
                == Some("Enemy Ring")
        })
        .expect("the elite has its pulse ring");

    let shell_data = run.world.get(shell).expect("the shell remains");
    assert_eq!(shell_data.parent, Some(enemy));
    assert_eq!(
        shell_data.components["sindri.shape"]["count"].as_f64(),
        Some(enemy_sides),
        "the shell must match the enemy silhouette"
    );
    assert_eq!(run.world.get(ring).expect("the ring remains").parent, Some(enemy));

    let shell_rotation = shell_data
        .transform_3d
        .as_ref()
        .expect("the shell has a transform")
        .rotation;
    let ring_alpha = run.world.get(ring).expect("the ring remains").components
        ["sindri.shape"]["stroke"][3]
        .as_f64()
        .expect("the ring has an alpha");

    step(&mut run);

    assert_ne!(
        run.world
            .get(shell)
            .expect("the shell remains")
            .transform_3d
            .as_ref()
            .expect("the shell has a transform")
            .rotation,
        shell_rotation,
        "the elite shell must rotate independently of its parent"
    );
    assert_ne!(
        run.world.get(ring).expect("the ring remains").components["sindri.shape"]
            ["stroke"][3]
            .as_f64()
            .expect("the ring has an alpha"),
        ring_alpha,
        "the reference elite ring must pulse"
    );
}
