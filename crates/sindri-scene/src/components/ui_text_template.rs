//! Filling a text component's slots with numbers a script supplies.
//!
//! Decay has no string concatenation, no interpolation and no formatting
//! library, and `decay/LANGUAGE.md` says so deliberately — `+` is numeric
//! addition and nothing else. A script therefore cannot build `"Score: 1200"`,
//! and a HUD that cannot show a number is not a HUD.
//!
//! So the split is the other way round from most engines: **the scene owns the
//! words and the script owns the numbers.** A designer authors `"Score: {}"`
//! and a script calls `Ui.set_number`. The words stay in the scene file where
//! they can be read, reviewed and one day translated, rather than being
//! assembled inside a script where none of that is possible.

/// What a template's slot asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    /// `{}` — as few decimals as say the value, up to three.
    Shortest,
    /// `{.N}` — exactly N decimals.
    Fixed(u8),
}

impl Slot {
    /// The most decimals a slot may ask for.
    ///
    /// Three past six is noise at `f32` precision, and a HUD asking for nine is
    /// a typo rather than a request.
    const MAX_DECIMALS: u8 = 6;

    /// Renders one value, or `0` for a slot no script has filled yet.
    ///
    /// Missing means zero rather than blank or an error marker, because the
    /// frame before a script's first `set_number` should read `Score: 0` — a
    /// scoreboard that has not been written to has a score of nothing.
    fn render(self, value: Option<f32>) -> String {
        let value = value.unwrap_or(0.0);
        if !value.is_finite() {
            // A NaN reaching a HUD is a gameplay bug, and `NaN` on the screen
            // says so far better than a number that looks plausible.
            return "NaN".to_owned();
        }
        match self {
            Self::Fixed(decimals) => format!("{value:.*}", decimals as usize),
            Self::Shortest => {
                let rendered = format!("{value:.3}");
                let trimmed = rendered.trim_end_matches('0').trim_end_matches('.');
                // `-0` reads as a mistake, and a score of minus nothing is nothing.
                if trimmed == "-0" || trimmed.is_empty() {
                    "0".to_owned()
                } else {
                    trimmed.to_owned()
                }
            }
        }
    }
}

/// Fills `template`'s slots from `values`, left to right.
///
/// A doubled brace is a literal one, either way round, which is the convention
/// a designer will already have seen. A lone `}` outside a slot is simply
/// itself rather than an error: text is content, and refusing to draw a label
/// over a stray brace helps nobody. A slot that does not parse is left in the
/// output exactly as written, because a designer who typed `{.x}` should see it
/// on the screen and fix it, rather than watch it silently vanish.
#[must_use]
pub fn fill(template: &str, values: &[f32]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut taken = 0usize;
    let mut rest = template;
    while let Some(brace) = rest.find(['{', '}']) {
        out.push_str(&rest[..brace]);
        rest = &rest[brace..];
        if let Some(after) = rest.strip_prefix("{{") {
            out.push('{');
            rest = after;
            continue;
        }
        if let Some(after) = rest.strip_prefix("}}") {
            out.push('}');
            rest = after;
            continue;
        }
        if rest.starts_with('}') {
            out.push('}');
            rest = &rest[1..];
            continue;
        }
        let Some(close) = rest.find('}') else {
            // An unclosed brace is the rest of the string, verbatim.
            break;
        };
        match parse_slot(&rest[1..close]) {
            Some(slot) => {
                out.push_str(&slot.render(values.get(taken).copied()));
                taken += 1;
            }
            None => out.push_str(&rest[..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// How many values a template asks for, which is how many slots it has.
#[must_use]
pub fn slot_count(template: &str) -> usize {
    let mut count = 0usize;
    let mut rest = template;
    while let Some(brace) = rest.find(['{', '}']) {
        rest = &rest[brace..];
        if let Some(after) = rest.strip_prefix("{{").or_else(|| rest.strip_prefix("}}")) {
            rest = after;
            continue;
        }
        if rest.starts_with('}') {
            rest = &rest[1..];
            continue;
        }
        let Some(close) = rest.find('}') else { break };
        if parse_slot(&rest[1..close]).is_some() {
            count += 1;
        }
        rest = &rest[close + 1..];
    }
    count
}

/// The inside of a `{...}`, or `None` when it is not a slot at all.
fn parse_slot(inside: &str) -> Option<Slot> {
    if inside.is_empty() {
        return Some(Slot::Shortest);
    }
    let digits = inside.strip_prefix('.')?;
    let decimals: u8 = digits.parse().ok()?;
    (decimals <= Slot::MAX_DECIMALS).then_some(Slot::Fixed(decimals))
}

#[cfg(test)]
mod tests {
    use super::{fill, slot_count};

    #[test]
    fn a_template_with_no_slots_is_its_own_text() {
        assert_eq!(fill("Game Over", &[]), "Game Over");
        assert_eq!(slot_count("Game Over"), 0);
    }

    /// The case the whole design exists for.
    #[test]
    fn a_score_reads_as_a_whole_number() {
        assert_eq!(fill("Score: {}", &[1200.0]), "Score: 1200");
    }

    #[test]
    fn a_multiplier_keeps_the_decimals_it_needs() {
        assert_eq!(fill("x{}", &[1.5]), "x1.5");
        assert_eq!(fill("x{}", &[2.0]), "x2");
    }

    #[test]
    fn slots_are_filled_left_to_right() {
        assert_eq!(fill("{}/{}", &[45.0, 100.0]), "45/100");
        assert_eq!(slot_count("{}/{}"), 2);
    }

    /// The frame before a script's first `set_number`.
    #[test]
    fn a_slot_no_script_has_filled_yet_reads_as_zero() {
        assert_eq!(fill("Score: {}", &[]), "Score: 0");
        assert_eq!(fill("{}/{}", &[45.0]), "45/0");
    }

    #[test]
    fn a_fixed_slot_keeps_exactly_the_decimals_it_asked_for() {
        assert_eq!(fill("{.2}s", &[1.5]), "1.50s");
        assert_eq!(fill("{.0}%", &[99.6]), "100%");
    }

    #[test]
    fn extra_values_are_ignored_rather_than_appended() {
        assert_eq!(fill("Score: {}", &[7.0, 9.0]), "Score: 7");
    }

    #[test]
    fn a_doubled_brace_is_a_literal_one() {
        assert_eq!(fill("{{}}", &[]), "{}");
        assert_eq!(slot_count("{{}}"), 0);
        assert_eq!(fill("{{{}}}", &[5.0]), "{5}", "a slot between two literals");
    }

    /// Text is content, and refusing to draw a label over a stray brace helps
    /// nobody.
    #[test]
    fn a_lone_closing_brace_is_just_itself() {
        assert_eq!(fill("a } b", &[]), "a } b");
        assert_eq!(slot_count("a } b"), 0);
    }

    /// A designer who typed it wrong should see it on the screen.
    #[test]
    fn something_that_is_not_a_slot_is_left_exactly_as_written() {
        assert_eq!(fill("{.x}", &[1.0]), "{.x}");
        assert_eq!(fill("{9}", &[1.0]), "{9}");
        assert_eq!(fill("{.9}", &[1.0]), "{.9}", "more decimals than f32 has");
        assert_eq!(fill("a {b", &[1.0]), "a {b", "never closed");
    }

    /// A HUD showing `NaN` is telling the truth about a gameplay bug.
    #[test]
    fn a_value_that_is_not_a_number_says_so() {
        assert_eq!(fill("{}", &[f32::NAN]), "NaN");
        assert_eq!(fill("{.2}", &[f32::INFINITY]), "NaN");
    }

    #[test]
    fn minus_nothing_is_nothing() {
        assert_eq!(fill("{}", &[-0.0]), "0");
        assert_eq!(fill("{}", &[-0.0001]), "0");
    }

    #[test]
    fn a_slot_is_shortest_rather_than_endless() {
        assert_eq!(fill("{}", &[1.0 / 3.0]), "0.333");
    }
}
