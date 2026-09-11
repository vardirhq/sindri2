//! What setup must do, in every state a machine can be in.

use super::*;

fn nothing() -> Probe {
    Probe::default()
}

fn installed_but_stopped() -> Probe {
    Probe {
        backend_installed: true,
        ..Probe::default()
    }
}

fn running(models: &[&str], memory: f32) -> Probe {
    Probe {
        backend_installed: true,
        backend_reachable: true,
        backend_version: Some("0.5.0".to_owned()),
        models: models.iter().map(|model| (*model).to_owned()).collect(),
        available_memory: Some(memory),
    }
}

fn all_features() -> Vec<Feature> {
    Feature::ALL.to_vec()
}

/// The rule the whole flow is built around: the only thing a person ever types
/// is a password, and only where the operating system demands one. Asserted
/// structurally rather than by reading the UI, so an action added later that
/// wants a name or a path fails here.
#[test]
fn no_step_ever_asks_a_person_to_type_anything() {
    let states = [
        (Readiness::BackendMissing, nothing()),
        (Readiness::BackendStopped, installed_but_stopped()),
        (Readiness::NoModel, running(&[], 12.0)),
        (Readiness::NoModel, running(&[], 2.0)),
        (
            Readiness::Unverified {
                model: "qwen3:8b".to_owned(),
            },
            running(&["qwen3:8b"], 12.0),
        ),
        (
            Readiness::Verifying {
                model: "qwen3:8b".to_owned(),
            },
            running(&["qwen3:8b"], 12.0),
        ),
        (
            Readiness::Ready {
                model: "qwen3:8b".to_owned(),
                verified: all_features(),
            },
            running(&["qwen3:8b"], 12.0),
        ),
        (
            Readiness::Unusable {
                model: "qwen3:8b".to_owned(),
                failed: Feature::ToolCalling,
            },
            running(&["qwen3:8b"], 12.0),
        ),
    ];
    for (state, probe) in states {
        match state.step(&probe).action {
            // Every one of these is a button. `Install` may raise an OS
            // password prompt, which is the one allowed exception and is not
            // Sindri's field to read.
            Action::Install { .. }
            | Action::Start
            | Action::Pull { .. }
            | Action::PickFrom { .. }
            | Action::Verify { .. }
            | Action::None => {}
        }
    }
}

#[test]
fn a_machine_with_nothing_on_it_is_offered_the_install() {
    let probe = nothing();
    let state = readiness(&probe, None);
    assert_eq!(state, Readiness::BackendMissing);
    let step = state.step(&probe);
    assert!(matches!(step.action, Action::Install { .. }));
    assert!(state.awaiting_the_machine());
}

/// Installed-but-not-running is a different sentence from not-installed, which
/// is the entire reason the states are named rather than collapsed into "could
/// not connect".
#[test]
fn a_runner_that_is_installed_but_quiet_is_told_apart_from_a_missing_one() {
    let probe = installed_but_stopped();
    let state = readiness(&probe, None);
    assert_eq!(state, Readiness::BackendStopped);
    assert_eq!(state.step(&probe).action, Action::Start);
}

#[test]
fn a_running_runner_with_no_model_is_offered_one_that_fits() {
    let probe = running(&[], 12.0);
    let state = readiness(&probe, None);
    assert_eq!(state, Readiness::NoModel);
    let Action::Pull { profile } = state.step(&probe).action else {
        panic!("a 12 GB machine should be offered a download");
    };
    assert!(profile.fits(12.0));
}

/// Consent has to be informed, so the step says the size, the licence and the
/// reason before anything multi-gigabyte begins.
#[test]
fn a_download_says_what_it_costs_before_it_starts() {
    let probe = running(&[], 12.0);
    let step = readiness(&probe, None).step(&probe);
    let Action::Pull { profile } = step.action else {
        panic!("expected a download");
    };
    assert!(step.detail.contains(&format!("{:.1} GB", profile.download)));
    assert!(step.detail.contains(profile.licence));
    assert!(step.detail.contains(profile.because));
}

/// A machine too small for anything measured still gets a list to click, marked
/// with what will not fit, rather than a dead end or a text field.
#[test]
fn a_small_machine_is_told_the_truth_and_still_given_the_choice() {
    let probe = running(&[], 2.0);
    let step = readiness(&probe, None).step(&probe);
    let Action::PickFrom { choices } = step.action else {
        panic!("expected a list to pick from");
    };
    assert_eq!(
        choices.len(),
        catalogue::PROFILES.len(),
        "all of them, marked"
    );
    assert!(choices.iter().all(|(_, fits)| !fits));
    assert!(step.detail.contains("2.0 GB"));
}

#[test]
fn an_undetectable_gpu_is_not_treated_as_an_empty_one() {
    let probe = Probe {
        backend_installed: true,
        backend_reachable: true,
        models: vec!["qwen3:8b".to_owned()],
        available_memory: None,
        ..Probe::default()
    };
    assert_eq!(
        readiness(&probe, None),
        Readiness::Unverified {
            model: "qwen3:8b".to_owned()
        },
        "a model that is there is usable even when the memory cannot be read"
    );
}

/// A reachable model is not a working one, which is the mistake the whole
/// verification step exists to stop.
#[test]
fn a_model_that_is_merely_present_is_not_called_ready() {
    let probe = running(&["qwen3:8b"], 12.0);
    let state = readiness(&probe, None);
    assert!(!state.is_ready());
    assert_eq!(
        state.step(&probe).action,
        Action::Verify {
            model: "qwen3:8b".to_owned()
        }
    );
}

#[test]
fn a_verified_model_is_ready_and_has_nothing_left_to_do() {
    let probe = running(&["qwen3:8b"], 12.0);
    let features = all_features();
    let state = readiness(&probe, Some(("qwen3:8b", &features)));
    assert!(state.is_ready());
    assert_eq!(state.step(&probe).action, Action::None);
    assert!(!state.awaiting_the_machine());
}

/// Verification belongs to the model it was run against. Swapping the model
/// must not inherit the last one's results.
#[test]
fn verifying_one_model_says_nothing_about_another() {
    let probe = running(&["gemma3:12b"], 24.0);
    let features = all_features();
    assert_eq!(
        readiness(&probe, Some(("qwen3:8b", &features))),
        Readiness::Unverified {
            model: "gemma3:12b".to_owned()
        }
    );
}

/// Missing something optional degrades the assistant; missing something
/// required means it cannot author at all, and the two must not read the same.
#[test]
fn a_model_missing_only_vision_is_still_ready() {
    let probe = running(&["qwen3:8b"], 12.0);
    let features: Vec<_> = Feature::ALL
        .into_iter()
        .filter(|feature| *feature != Feature::Vision)
        .collect();
    assert!(readiness(&probe, Some(("qwen3:8b", &features))).is_ready());
}

#[test]
fn a_model_that_cannot_hold_a_schema_cannot_author() {
    let probe = running(&["qwen3:8b"], 12.0);
    let features: Vec<_> = Feature::ALL
        .into_iter()
        .filter(|feature| *feature != Feature::StructuredOutput)
        .collect();
    assert_eq!(
        readiness(&probe, Some(("qwen3:8b", &features))),
        Readiness::Unusable {
            model: "qwen3:8b".to_owned(),
            failed: Feature::StructuredOutput,
        }
    );
}

#[test]
fn the_required_features_are_the_ones_every_proposal_needs() {
    assert!(Feature::StructuredOutput.required());
    assert!(Feature::ToolCalling.required());
    assert!(!Feature::Vision.required());
}

/// The editor keeps watching exactly while the next move happens outside it, so
/// that finishing an install advances the screen without anyone hunting for a
/// refresh button.
#[test]
fn the_editor_watches_only_while_it_is_waiting_on_the_machine() {
    assert!(Readiness::BackendMissing.awaiting_the_machine());
    assert!(Readiness::BackendStopped.awaiting_the_machine());
    assert!(!Readiness::NoModel.awaiting_the_machine());
    assert!(
        !Readiness::Ready {
            model: "qwen3:8b".to_owned(),
            verified: all_features(),
        }
        .awaiting_the_machine()
    );
}

/// A profiled model the machine can hold beats an unprofiled one, because the
/// profiled ones are the ones Sindri has actually measured.
#[test]
fn a_measured_model_is_preferred_over_an_unknown_one() {
    let probe = running(&["something-else:latest", "qwen3:8b"], 12.0);
    assert_eq!(
        readiness(&probe, None),
        Readiness::Unverified {
            model: "qwen3:8b".to_owned()
        }
    );
}

/// But a model someone pulled themselves is not refused for being unknown.
#[test]
fn an_unprofiled_model_is_still_usable() {
    let probe = running(&["something-else:latest"], 12.0);
    assert_eq!(
        readiness(&probe, None),
        Readiness::Unverified {
            model: "something-else:latest".to_owned()
        }
    );
}

/// A profiled model too big for the machine must not be chosen over one that
/// fits, however capable it is.
#[test]
fn a_model_that_does_not_fit_is_not_chosen() {
    let probe = running(&["qwen3:8b", "qwen3-coder:30b"], 12.0);
    assert_eq!(
        readiness(&probe, None),
        Readiness::Unverified {
            model: "qwen3:8b".to_owned()
        }
    );
}

#[test]
fn every_step_says_something_a_person_can_act_on() {
    for (state, probe) in [
        (Readiness::BackendMissing, nothing()),
        (Readiness::BackendStopped, installed_but_stopped()),
        (Readiness::NoModel, running(&[], 12.0)),
    ] {
        let step = state.step(&probe);
        assert!(!step.title.is_empty(), "{state:?} has no title");
        assert!(!step.detail.is_empty(), "{state:?} explains nothing");
    }
}

/// The install plan is shown before it runs, so the step is inspectable rather
/// than opaque — and its source is pinned rather than discovered.
#[test]
fn the_install_says_where_it_comes_from_and_what_it_will_do() {
    let probe = nothing();
    let Action::Install { plan } = readiness(&probe, None).step(&probe).action else {
        panic!("expected an install");
    };
    assert!(plan.source.starts_with("https://"));
    assert!(!plan.performs.is_empty());
    assert!(plan.download > 0.0);
}
