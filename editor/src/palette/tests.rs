//! What the palette must always do, whatever is in the project.

use super::*;

fn panel(label: &str) -> Candidate {
    Candidate::new(label, Kind::Panel, Action::ShowPanel(Panel::Console))
}

fn asset(label: &str, note: &str) -> Candidate {
    Candidate::new(label, Kind::Asset, Action::SelectAsset(label.to_owned())).noted(note)
}

fn labels(results: &[Candidate]) -> Vec<String> {
    results.iter().map(|result| result.label.clone()).collect()
}

#[test]
fn an_empty_query_offers_the_candidates_in_the_order_they_were_built() {
    let palette = Palette::default();
    let results = palette.rank(vec![panel("Console"), panel("Project"), panel("History")]);
    assert_eq!(labels(&results), ["Console", "Project", "History"]);
}

/// The reason panels and verbs are built before the project's files: opening on
/// ten assets in alphabetical order tells nobody anything.
#[test]
fn an_empty_query_shows_the_few_things_before_the_many() {
    let palette = Palette::default();
    let mut candidates = vec![panel("Console")];
    candidates.extend((0..50).map(|n| asset(&format!("texture{n}.png"), "assets")));
    let results = palette.rank(candidates);
    assert_eq!(results[0].label, "Console");
}

#[test]
fn no_more_than_a_screenful_is_offered() {
    let palette = Palette::default();
    let candidates: Vec<_> = (0..100).map(|n| panel(&format!("panel {n}"))).collect();
    assert_eq!(palette.rank(candidates).len(), VISIBLE);
}

#[test]
fn what_does_not_match_is_left_out() {
    let palette = Palette {
        query: "console".to_owned(),
        ..Palette::default()
    };
    let results = palette.rank(vec![panel("Console"), panel("Inspector")]);
    assert_eq!(labels(&results), ["Console"]);
}

/// A file is found by the folder it is in as well as by its name, because that
/// is how people remember where things are.
#[test]
fn a_note_is_searched_as_well_as_a_label() {
    let palette = Palette {
        query: "prefabs drifter".to_owned(),
        ..Palette::default()
    };
    let results = palette.rank(vec![
        asset("drifter.prefab.json", "assets/prefabs"),
        asset("drifter.png", "assets/textures"),
    ]);
    assert_eq!(
        results.first().map(|r| r.label.as_str()),
        Some("drifter.prefab.json")
    );
}

#[test]
fn the_selection_wraps_at_both_ends() {
    let mut palette = Palette::default();
    palette.step(-1, 3);
    assert_eq!(
        palette.chosen(3),
        2,
        "up from the first entry reaches the last"
    );
    palette.step(1, 3);
    assert_eq!(palette.chosen(3), 0);
}

/// Typing narrows the list under a selection that was pointing further down, so
/// the index has to be answered against what is actually there.
#[test]
fn a_selection_past_the_end_resolves_to_the_last_result() {
    let mut palette = Palette::default();
    palette.choose(7);
    assert_eq!(palette.chosen(3), 2);
    assert_eq!(palette.chosen(0), 0);
}

#[test]
fn stepping_through_nothing_selects_nothing() {
    let mut palette = Palette::default();
    palette.step(1, 0);
    assert_eq!(palette.chosen(0), 0);
}

/// Reopening on the last search would put the next keystroke in the middle of
/// it, and nobody reopens a search they have already acted on.
#[test]
fn opening_always_starts_from_an_empty_query() {
    let mut palette = Palette::default();
    palette.open();
    palette.query = "drifter".to_owned();
    palette.choose(4);
    palette.close();
    palette.open();
    assert!(palette.query.is_empty());
    assert_eq!(palette.chosen(9), 0);
}

#[test]
fn the_field_takes_focus_once_and_then_leaves_it_alone() {
    let mut palette = Palette::default();
    palette.open();
    assert!(palette.take_focus(), "the frame it opens on");
    assert!(!palette.take_focus(), "and no frame after that");
}

/// Every verb is offered, or one added to the enum is one nobody can reach.
#[test]
fn every_verb_is_listed() {
    assert_eq!(Verb::ALL.len(), 13);
    let mut seen: Vec<Verb> = Verb::ALL.iter().map(|(verb, _, _)| *verb).collect();
    seen.dedup();
    assert_eq!(seen.len(), Verb::ALL.len(), "a verb is listed once");
    for (_, label, _) in Verb::ALL {
        assert!(!label.is_empty());
    }
}
