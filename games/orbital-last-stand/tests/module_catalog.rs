use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;
const NAMES: &str = "Hot Core|Cold Forge|Redline Coil|Pulse Divider|Rail Accelerator|Dense Slug|Split Bus|Trident Relay|Phase Jacket|Ghost Bore|Lucky Circuit|Loaded Die|Guidance Kernel|Arc Imprint|Nova Imprint|Gravity Anchor|Prism Imprint|Glass Reactor|Gyro Stabilizer|Ion Choke|Reinforced Hull|Titanium Ribs|Emergency Foam|Repair Nanites|Reactive Plating|Ablative Shell|Vector Thrusters|Afterburner|Slipstream|Gravity Well|Signal Harvest|Black Box|Combat Medic|Thin Skin|Heavy Frame|Razor Orbit|Aegis Halo|Ember Familiar|Void Wisp|Gundrone|Saw Moon|Needle Satellite|Halo Shard|Mirror Moon|Storm Sprite|Seeker Sprite|Nova Mote|Gravity Mote|Phase Drone|Fork Drone|Mercury Switch|Copper Heart|Ceramic Fuse|Blue Capacitor|Gold Capacitor|Fat Capacitor|Long Barrel|Short Barrel|Warped Lens|Dead Channel|Live Channel|Spare Bulkhead|Field Rations|Mag Clamp|Data Leech|Hot Wiring|Coolant Loop|Overpressure|Needle Rounds|Soft Rounds|Salvage Map|Combat Telemetry|Shock Mount|Drive Belt|Ballast|Razor Wire|Bright Powder|Dark Powder|Fast Clock|Slow Clock|Spare Reactor|Dirty Reactor|Clean Reactor|Twin Pump|Wide Nozzle|Pinpoint Nozzle|Hunter Array|Surveyor|Vacuum Scoop|Learning Core|Blood Battery|Revenge Relay|Last Bulkhead|Kill Switch|Scrap Feast|Critical Reboot|Phase Memory|Terminal Velocity|Big Bang Board|Echo Chamber|Fork Tax|Ghost Protocol|Arc Battery|Prism Mirror|Anchor Clock|Homing Instinct|Second Opinion|Orbital Foundry|Mutual Defense|Crowded Orbit|Black Sun|White Noise|Needle Storm|Glass Needle|Heavy Phase|Bright Ghost|Seeking Splitter|Storm Lens|Gravity Prism|Nova Guidance|Phase Anchor|Critical Arc|Razor Payload|Razor Velocity|Ember Arc|Wisp Anchor|Drone Fork|Aegis Nova|Familiar Guidance|Orbital Prism|Saint Elmo|Funeral Star|Choir Engine|Event Horizon Chip|Recursive Bus|Prism Rail|Critical Mass Cell|Guardian Network|Moon Court|Dead God Circuit|Rusted Key|Lucky Bolt|Warm Seat|Red Tape|Blue Tape|Green Tape|Wrecking Node|Tiny Magnet|Training Manual|Polished Casing|Constellation Engine|Reversal Chamber|Aegis Reservoir|Orbit Loom|Broadside Protocol|Grave Echo|Split Horizon|Devouring Moon|Pulse Heart|Execution Mark";

fn started_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        assert!(run.step(STEP).is_empty());
    }
    run.click("TitleStart");
    assert!(run.step(STEP).is_empty());
    run
}

fn offer(run: &mut Run, id: f32) {
    run.set_board("cores", run.board("next_level"));
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    assert_eq!(run.board("run_state"), 2.0);
    run.step(STEP);
    run.set_board("offer_0", id);
    run.click("Guidance Kernel");
    assert_eq!(run.board("run_state"), 1.0);
}

#[test]
fn the_reference_catalog_has_all_160_names() {
    let chooser = include_str!("../assets/scripts/upgrade-chooser.decay");
    let names: Vec<_> = NAMES.split('|').collect();
    assert_eq!(names.len(), 160);
    for name in names {
        assert!(
            chooser.contains(&format!("return \"{name}\";")),
            "catalog is missing {name}"
        );
    }
}

#[test]
fn core_template_and_special_modules_change_the_build() {
    let mut damage = started_run();
    offer(&mut damage, 0.0);
    assert!((damage.board("damage") - 1.28).abs() < 0.001);

    let mut template = started_run();
    let before_gap = template.board("fire_gap");
    offer(&mut template, 1.0);
    assert!(template.board("fire_gap") < before_gap);

    let mut fork = started_run();
    offer(&mut fork, 6.0);
    assert_eq!(fork.board("shots"), 2.0);

    let mut guidance = started_run();
    offer(&mut guidance, 12.0);
    assert_eq!(guidance.board("missile"), 1.0);

    let mut special = started_run();
    offer(&mut special, 90.0);
    assert_eq!(special.board("special_blood_battery"), 1.0);
}

fn is_companion(id: f32) -> bool {
    matches!(
        id as i32,
        35 | 36 | 37 | 38 | 39 | 107 | 108 | 109 | 122 | 123 | 124 | 125 | 126 | 127 | 128
            | 129 | 130 | 132 | 137 | 138 | 139 | 146 | 152 | 153 | 159
    )
}

#[test]
fn foundry_signal_only_offers_companion_modules() {
    let mut run = started_run();
    run.set_board("module_pool", 2.0);
    run.set_board("run_state", 2.0);
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    let offers = [run.board("offer_0"), run.board("offer_1"), run.board("offer_2")];
    assert!(offers.iter().all(|id| is_companion(*id)), "{offers:?}");
    assert_ne!(offers[0], offers[1]);
    assert_ne!(offers[0], offers[2]);
    assert_ne!(offers[1], offers[2]);
}

#[test]
fn second_opinion_adds_the_fourth_last_stand_choice() {
    let mut run = started_run();
    run.set_board("special_second_opinion", 1.0);
    run.set_board("run_state", 2.0);
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    assert!(run.board("offer_3") >= 0.0);
    let active = run.active_named("upgrade");
    assert_eq!(active.len(), 4, "{active:?}");
}
