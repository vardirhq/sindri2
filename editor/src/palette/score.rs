//! Deciding which of several hundred things someone meant by four letters.
//!
//! Subsequence matching rather than substring: typing `oscn` should find
//! `orbital.scene.json`, and a palette that only finds what you spelled
//! contiguously is a filter box with a keyboard shortcut. Scored rather than
//! merely matched, because every candidate matches something eventually — what
//! decides a palette's usefulness is the order the matches come back in.

/// How well a candidate answers a query, larger being better.
///
/// Only ever compared, never displayed, so the numbers mean nothing on their
/// own. What they encode is a ranking anyone would agree with: a word someone
/// started typing beats one where the letters happen to appear.
pub type Score = i32;

/// Awarded when a run of matched characters starts a word.
///
/// The largest single term, because this is what makes `sas` find "Save scene
/// as" ahead of every path containing an s, an a and an s.
const WORD_START: Score = 18;
/// Awarded when a matched character follows the previous one.
///
/// Compounding along a run — see [`RUN_GROWTH`] — because a flat bonus loses to
/// a scattered match through a name full of separators: every character after
/// an underscore starts a word, so `p_l_a_y_e_r` collected six word-start
/// bonuses and outranked `player`, which is the opposite of what anyone means.
const ADJACENT: Score = 10;
/// Added per character already in the current run, so a long contiguous match
/// pulls away from a scattered one rather than merely keeping pace.
const RUN_GROWTH: Score = 4;
/// Charged per character skipped before the first match, capped, so a hit deep
/// inside a long path is not ruled out — only ranked below a shallower one.
const LEADING_PENALTY: Score = 2;
const MAX_LEADING_PENALTY: Score = 20;
/// Awarded when the whole query appears contiguously, on top of the rest.
const CONTIGUOUS_BONUS: Score = 24;
/// Awarded when the candidate begins with the query.
const PREFIX_BONUS: Score = 30;

/// Whether a character begins a word, for the purpose of ranking.
///
/// Case changes count, so `sceneFile` has two word starts, and so do the
/// separators that appear in paths, identifiers and file names.
fn boundary(previous: Option<char>, current: char) -> bool {
    match previous {
        None => true,
        Some(previous) => {
            !previous.is_alphanumeric() || (previous.is_lowercase() && current.is_uppercase())
        }
    }
}

/// Scores `candidate` against `query`, or `None` when it does not match.
///
/// An empty query matches everything with a score of zero, which is what lets
/// the palette open showing something useful rather than a blank list.
pub fn score(candidate: &str, query: &str) -> Option<Score> {
    if query.is_empty() {
        return Some(0);
    }
    let lowered: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();
    if lowered.len() != candidate.chars().count() {
        // A character whose lowercase is more than one character would put the
        // index below out of step with the original. Rare enough to be worth
        // handling by falling back rather than by carrying a mapping around.
        return simple_score(&candidate.to_lowercase(), &query.to_lowercase());
    }
    let original: Vec<char> = candidate.chars().collect();
    let wanted: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();

    let mut total: Score = 0;
    let mut at = 0usize;
    let mut previous_match: Option<usize> = None;
    let mut run: Score = 0;
    for needle in wanted {
        let found = lowered[at..].iter().position(|c| *c == needle)? + at;
        let previous = found.checked_sub(1).map(|index| original[index]);
        if boundary(previous, original[found]) {
            total += WORD_START;
        }
        if previous_match == found.checked_sub(1) && previous_match.is_some() {
            total += ADJACENT + run * RUN_GROWTH;
            run += 1;
        } else {
            run = 0;
        }
        if previous_match.is_none() {
            let leading = Score::try_from(found).unwrap_or(Score::MAX);
            total -= (leading * LEADING_PENALTY).min(MAX_LEADING_PENALTY);
        }
        previous_match = Some(found);
        at = found + 1;
    }
    let lowered_text: String = lowered.iter().collect();
    let lowered_query = query.to_lowercase();
    if lowered_text.starts_with(&lowered_query) {
        total += PREFIX_BONUS;
    } else if lowered_text.contains(&lowered_query) {
        total += CONTIGUOUS_BONUS;
    }
    // A shorter candidate saying the same thing is the better answer: "Scene"
    // should beat "Scene tools do not apply to the game view".
    total -= Score::try_from(candidate.chars().count()).unwrap_or(Score::MAX) / 12;
    Some(total)
}

/// Scores every whitespace-separated term, requiring all of them to match.
///
/// Terms rather than one string because that is how people search: `prefabs
/// drifter` and `drifter prefabs` mean the same thing, and a matcher that takes
/// the query as one sequence answers the second and not the first. Each term is
/// matched independently against the whole candidate, and the scores are added,
/// so a candidate answering both terms well beats one answering either.
pub fn score_terms(candidate: &str, query: &str) -> Option<Score> {
    let mut terms = query.split_whitespace().peekable();
    if terms.peek().is_none() {
        return Some(0);
    }
    let mut total = 0;
    for term in terms {
        total += score(candidate, term)?;
    }
    Some(total)
}

/// The fallback for text the index arithmetic above cannot safely walk.
fn simple_score(candidate: &str, query: &str) -> Option<Score> {
    let mut at = 0;
    for needle in query.chars() {
        at = candidate[at..].find(needle)? + at + needle.len_utf8();
    }
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn better(one: &str, other: &str, query: &str) {
        let first = score(one, query).unwrap_or_else(|| panic!("{one:?} should match {query:?}"));
        let second =
            score(other, query).unwrap_or_else(|| panic!("{other:?} should match {query:?}"));
        assert!(
            first > second,
            "{one:?} ({first}) should rank above {other:?} ({second}) for {query:?}"
        );
    }

    #[test]
    fn letters_need_not_be_next_to_each_other() {
        assert!(score("orbital.scene.json", "oscn").is_some());
    }

    #[test]
    fn letters_in_the_wrong_order_do_not_match() {
        assert!(score("orbital.scene.json", "nosc").is_none());
    }

    #[test]
    fn a_missing_letter_does_not_match() {
        assert!(score("Save scene", "savez").is_none());
    }

    #[test]
    fn matching_ignores_case() {
        assert!(score("Save Scene As", "sas").is_some());
        assert!(score("save scene as", "SAS").is_some());
    }

    /// The property the whole thing exists for: initials of words beat letters
    /// that merely occur.
    #[test]
    fn initials_beat_incidental_letters() {
        better("Save scene as", "assets/sheets/atlas.png", "ssa");
    }

    #[test]
    fn a_prefix_beats_a_match_further_in() {
        better("Console", "Weave console styles", "cons");
    }

    #[test]
    fn a_contiguous_run_beats_a_scattered_one() {
        better("player.decay", "p_l_a_y_e_r.decay", "player");
    }

    #[test]
    fn the_shorter_of_two_answers_wins() {
        better("Scene", "Scene tools do not apply here", "scene");
    }

    #[test]
    fn an_empty_query_matches_everything_equally() {
        assert_eq!(score("anything at all", ""), Some(0));
        assert_eq!(score("", ""), Some(0));
    }

    #[test]
    fn nothing_matches_a_query_against_an_empty_candidate() {
        assert!(score("", "a").is_none());
    }

    /// Paths are most of what a project holds, and a deep one must still be
    /// reachable — ranked below a shallow hit, never excluded by it.
    #[test]
    fn a_deep_path_still_matches() {
        assert!(score("assets/prefabs/enemies/drifter.prefab.json", "drifter").is_some());
        better(
            "drifter.prefab.json",
            "assets/prefabs/enemies/drifter.prefab.json",
            "drifter",
        );
    }

    /// How people actually search: the order they remember things in is not
    /// the order the name is written in.
    #[test]
    fn terms_match_in_any_order() {
        let path = "drifter.prefab.json assets/prefabs";
        assert!(score_terms(path, "prefabs drifter").is_some());
        assert!(score_terms(path, "drifter prefabs").is_some());
    }

    #[test]
    fn every_term_has_to_match() {
        assert!(score_terms("drifter.prefab.json assets/prefabs", "drifter zzz").is_none());
    }

    #[test]
    fn a_query_of_only_spaces_matches_everything() {
        assert_eq!(score_terms("anything", "   "), Some(0));
    }

    #[test]
    fn text_needing_multi_character_lowercasing_still_matches() {
        // `İ` lowercases to two characters, which is the case the index walk
        // cannot take; it must still find the thing rather than panic.
        assert!(score("İstanbul level", "level").is_some());
        assert!(score("İstanbul level", "zzz").is_none());
    }
}
