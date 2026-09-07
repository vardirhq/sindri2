use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn ui_text<'a>(run: &'a Run, name: &str) -> &'a str {
    let entity = run.find(name).unwrap_or_else(|| panic!("{name} exists"));
    run.world
        .get(entity)
        .expect("the UI entity remains")
        .components["sindri.ui.text"]["text"]
        .as_str()
        .expect("the UI entity carries text")
}

#[test]
fn the_run_hud_uses_the_reference_information_hierarchy() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);

    assert_eq!(ui_text(&run, "HealthText"), "HP {}/{}");
    assert_eq!(ui_text(&run, "Level"), "LV {}");
    assert_eq!(ui_text(&run, "Score"), "SCORE {}");
    assert_eq!(ui_text(&run, "Wave"), "OUTER DRIFT");

    let health = run.find("Health").expect("the health bar exists");
    let xp = run.find("Cores").expect("the XP bar exists");
    let health_text = run.find("HealthText").expect("the HP label exists");
    assert_eq!(
        run.world
            .get(health)
            .expect("health bar remains")
            .transform_3d
            .as_ref()
            .expect("health bar has a transform")
            .position[1],
        1.76
    );
    assert_eq!(
        run.world
            .get(xp)
            .expect("XP bar remains")
            .transform_3d
            .as_ref()
            .expect("XP bar has a transform")
            .position[1],
        1.71
    );
    assert_eq!(
        run.world
            .get(health_text)
            .expect("HP label remains")
            .transform_3d
            .as_ref()
            .expect("HP label has a transform")
            .position[1],
        1.83
    );
}

#[test]
fn health_hit_animation_keeps_the_bar_compact() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);

    let health = run.find("Health").expect("the health bar exists");
    let base_height = run
        .world
        .get(health)
        .expect("health bar remains")
        .transform_3d
        .as_ref()
        .expect("health bar has a transform")
        .scale[1];
    assert!(base_height < 0.05, "health bar starts compact: {base_height}");

    run.set_board("shield_impact", 1.0);
    step(&mut run);
    let hit_height = run
        .world
        .get(health)
        .expect("health bar remains")
        .transform_3d
        .as_ref()
        .expect("health bar has a transform")
        .scale[1];

    assert!(hit_height > base_height);
    assert!(
        hit_height < base_height * 1.2,
        "hit pulse stays relative to the authored bar height: {hit_height}"
    );
}

#[test]
fn the_hud_names_every_reference_sector_and_boss() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);

    // Pause campaign simulation so the test can drive the HUD board directly.
    run.set_board("run_state", 3.0);

    let sectors = [
        (1.0, "OUTER DRIFT"),
        (2.0, "EMBER BELT"),
        (3.0, "VIOLET WAKE"),
        (4.0, "NULL LATTICE"),
        (5.0, "CORE APPROACH"),
    ];
    for (kind, label) in sectors {
        run.set_board("sector", kind);
        step(&mut run);
        assert_eq!(ui_text(&run, "Wave"), label);
    }

    run.set_board("boss_max", 100.0);
    run.set_board("boss_hp", 50.0);
    let bosses = [
        (0.0, "WARDEN"),
        (1.0, "HARROWER"),
        (2.0, "PRISM"),
        (3.0, "SINGULARITY"),
        (4.0, "CROWN"),
        (5.0, "BROOD"),
        (6.0, "MIRROR"),
        (7.0, "ARCHITECT"),
        (8.0, "SPINE"),
        (9.0, "LEVIATHAN"),
        (10.0, "LAST LIGHT"),
    ];
    for (kind, label) in bosses {
        run.set_board("boss_kind", kind);
        step(&mut run);
        assert_eq!(ui_text(&run, "BossName"), label);
    }

    let boss_panel = run.find("BossPanel").expect("the boss HUD exists");
    assert!(run.world.is_active(boss_panel));
}
