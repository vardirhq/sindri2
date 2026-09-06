use orbital_last_stand::Run;
use serde_json::json;
use sindri_core::ProfileDocument;

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

fn catalog() -> ProfileDocument {
    ProfileDocument::from_json(include_str!(
        "../assets/profiles/module-catalog.profile.json"
    ))
    .expect("the module catalog parses")
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
    let catalog = catalog();
    let modules = catalog
        .value("modules")
        .and_then(serde_json::Value::as_array)
        .expect("the catalog has modules");
    let names: Vec<_> = NAMES.split('|').collect();
    assert_eq!(names.len(), 160);
    assert_eq!(modules.len(), names.len());
    for (index, (module, expected)) in modules.iter().zip(names).enumerate() {
        assert_eq!(module["id"].as_u64(), Some(index as u64));
        assert_eq!(module["name"].as_str(), Some(expected));
        assert!(module["key"].as_str().is_some_and(|key| !key.is_empty()));
        assert!(
            module["owned_key"]
                .as_str()
                .is_some_and(|key| !key.is_empty())
        );
    }
}

#[test]
fn a_new_profile_entry_works_without_a_new_script_branch() {
    let mut run = started_run();
    let catalog = run
        .profiles
        .get_mut("profiles/module-catalog.profile.json")
        .expect("the catalog is loaded");
    catalog
        .values
        .get_mut("modules")
        .and_then(serde_json::Value::as_array_mut)
        .expect("the modules are a list")
        .push(json!({
            "id": 999,
            "key": "test-extension",
            "name": "Test Extension",
            "description": "Added only by the test.",
            "rarity": "TEST",
            "weight": 1,
            "pool_mask": 1,
            "black_signal": false,
            "owned_key": "owned_test_extension",
            "requires_key": "",
            "requires_min": 0
        }));
    catalog
        .values
        .get_mut("effects")
        .and_then(serde_json::Value::as_array_mut)
        .expect("the effects are a list")
        .push(json!({
            "module_id": 999,
            "operation": "add",
            "key": "profile_extension_probe",
            "value": 7
        }));

    offer(&mut run, 160.0);
    assert_eq!(run.board("profile_extension_probe"), 7.0);
    assert_eq!(run.board("owned_test_extension"), 1.0);
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

fn is_companion(id: f32, catalog: &ProfileDocument) -> bool {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = id as usize;
    let mask = catalog
        .value_at("modules", index, "pool_mask")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    mask & 4 != 0
}

#[test]
fn foundry_signal_only_offers_companion_modules() {
    let mut run = started_run();
    let catalog = catalog();
    run.set_board("module_pool", 2.0);
    run.set_board("run_state", 2.0);
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    let offers = [run.board("offer_0"), run.board("offer_1"), run.board("offer_2")];
    assert!(
        offers.iter().all(|id| is_companion(*id, &catalog)),
        "{offers:?}"
    );
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
