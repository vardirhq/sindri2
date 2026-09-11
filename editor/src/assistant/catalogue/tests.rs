//! What the manifest must hold, and what a grade must mean.

use super::*;

/// The manifest ships compiled in, so a broken one is a build problem. This is
/// what makes it one.
#[test]
fn the_manifest_is_valid() {
    let manifest: Manifest = serde_json::from_str(MANIFEST).expect("the committed manifest parses");
    assert_eq!(manifest.schema_version, 1);
    assert!(!manifest.models.is_empty());
    for (key, entries) in &manifest.models {
        assert!(!entries.is_empty(), "{key} names no entries");
        for entry in entries {
            assert_eq!(&entry.id, key, "entry id must match its key");
            assert!(entry.params_b > 0.0, "{key} has no parameter count");
            assert!(!entry.license.is_empty(), "{key} has no licence");
            assert!(
                entry.source.starts_with("https://"),
                "{key} has no source to check"
            );
            assert!(
                !entry.description.is_empty(),
                "{key} says nothing for itself"
            );
            assert!(!entry.quantisation.is_empty(), "{key} has no quantisation");
        }
    }
}

/// Every model offered has to answer in a schema and call a tool, because the
/// protocol is a schema and every proposal is a tool call.
#[test]
fn everything_in_the_manifest_can_drive_the_protocol() {
    for profile in profiles() {
        assert!(
            profile.supports.structured_output && profile.supports.tool_calling,
            "{} cannot drive a proposal and should not be listed",
            profile.id
        );
    }
}

#[test]
fn profiles_are_ordered_smallest_first() {
    let mut previous = 0.0;
    for profile in profiles() {
        assert!(
            profile.params_b >= previous,
            "{} is out of order",
            profile.id
        );
        previous = profile.params_b;
    }
}

/// The point of estimating rather than hardcoding: context is the part of the
/// bill the person changes, and the answer has to move with it.
#[test]
fn a_longer_context_costs_more_memory() {
    let profile = profile_for("qwen2.5-coder:7b").expect("the standard is profiled");
    assert!(profile.residency(65_536) > profile.residency(4_096));
}

#[test]
fn a_bigger_model_costs_more_memory() {
    let small = profile_for("qwen2.5-coder:7b").expect("profiled");
    let large = profile_for("qwen2.5-coder:14b").expect("profiled");
    assert!(large.residency(DEFAULT_CONTEXT) > small.residency(DEFAULT_CONTEXT));
}

/// The reference card the table is sized against. The standard must be
/// comfortable on it, and the model named as its ceiling must not be.
#[test]
fn the_reference_card_gets_the_standard_and_not_the_ceiling() {
    let standard = profile_for("qwen2.5-coder:7b").expect("profiled");
    assert_eq!(
        standard.tier(12.0, DEFAULT_CONTEXT),
        Tier::Recommended,
        "the standard must be comfortable on the card it was chosen for"
    );
    let ceiling = profile_for("qwen2.5-coder:14b").expect("profiled");
    assert_ne!(
        ceiling.tier(12.0, DEFAULT_CONTEXT),
        Tier::Recommended,
        "the card's ceiling is not what Sindri leads with"
    );
}

/// A model that runs but cannot be trusted through a multi-step edit has its
/// own grade, rather than being hidden or presented as fine.
#[test]
fn a_small_model_that_fits_is_best_effort_rather_than_recommended() {
    let small = profile_for("qwen3:1.7b").expect("profiled");
    assert_eq!(small.tier(24.0, DEFAULT_CONTEXT), Tier::BestEffort);
}

#[test]
fn a_model_that_does_not_fit_is_unsupported_however_good_it_is() {
    let large = profile_for("qwen2.5-coder:14b").expect("profiled");
    assert_eq!(large.tier(4.0, DEFAULT_CONTEXT), Tier::Unsupported);
}

/// Near the ceiling is its own answer: it runs, and saying so is more useful
/// than either recommending it or pretending it will not.
#[test]
fn a_model_that_only_just_fits_is_supported_rather_than_recommended() {
    let profile = profile_for("qwen2.5-coder:7b").expect("profiled");
    let needs = profile.residency(DEFAULT_CONTEXT);
    assert_eq!(profile.tier(needs, DEFAULT_CONTEXT), Tier::Supported);
    assert_eq!(
        profile.tier(needs * 1.5, DEFAULT_CONTEXT),
        Tier::Recommended
    );
}

#[test]
fn a_recommendation_is_always_comfortable_on_the_machine_it_was_made_for() {
    for available in [6.0, 9.0, 12.0, 16.0, 24.0, 48.0] {
        if let Some(profile) = recommended(available, DEFAULT_CONTEXT) {
            assert_eq!(
                profile.tier(available, DEFAULT_CONTEXT),
                Tier::Recommended,
                "{} was recommended for {available} GB",
                profile.id
            );
        }
    }
}

/// The manifest names a standard, and that is what Sindri leads with wherever
/// it is comfortable — rather than the largest thing that happens to fit.
/// Bigger is not better past the point where a model drives the tool loop:
/// beyond it the parameters cost context headroom and speed.
#[test]
fn the_standard_is_what_gets_recommended_where_it_fits() {
    let standard = profiles()
        .iter()
        .find(|profile| profile.standard)
        .expect("the manifest names a standard");
    for available in [12.0, 24.0, 48.0] {
        assert_eq!(
            recommended(available, DEFAULT_CONTEXT).map(|profile| &profile.id),
            Some(&standard.id),
            "{available} GB should be led to the standard"
        );
    }
}

/// And where the standard does not fit, the best that does is offered instead
/// of nothing.
#[test]
fn a_machine_too_small_for_the_standard_still_gets_the_best_that_fits() {
    let standard = profiles()
        .iter()
        .find(|profile| profile.standard)
        .expect("the manifest names a standard");
    let tight = standard.residency(DEFAULT_CONTEXT);
    if let Some(chosen) = recommended(tight, DEFAULT_CONTEXT) {
        assert_ne!(chosen.id, standard.id);
        assert_eq!(chosen.tier(tight, DEFAULT_CONTEXT), Tier::Recommended);
    }
}

#[test]
fn more_memory_is_offered_more_capability() {
    let small = recommended(12.0, DEFAULT_CONTEXT).expect("12 GB runs something");
    let large = recommended(48.0, DEFAULT_CONTEXT).expect("48 GB runs more");
    assert!(large.params_b >= small.params_b);
}

/// The rule this exists for.
#[test]
fn a_machine_that_can_run_nothing_comfortably_is_told_so() {
    assert!(recommended(1.0, DEFAULT_CONTEXT).is_none());
}

#[test]
fn a_model_the_user_pulled_themselves_is_simply_unprofiled() {
    assert!(profile_for("some-local-model:latest").is_none());
}

#[test]
fn only_the_grades_sindri_would_put_forward_are_offered() {
    assert!(Tier::Recommended.is_offered());
    assert!(Tier::Supported.is_offered());
    assert!(!Tier::BestEffort.is_offered());
    assert!(!Tier::Unsupported.is_offered());
}
