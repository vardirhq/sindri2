//! Shorthands that stand for one declaration per side.
//!
//! `padding: 4px 8px` is `padding-top: 4px; padding-right: 8px;
//! padding-bottom: 4px; padding-left: 8px`, and as in CSS it is expanded where
//! it is written: a later `padding-top` in the same rule, or a more specific
//! rule's, overrides one side and leaves the rest. The cascade only ever sees
//! the longhands.
//!
//! A value built from `var()` cannot be split before it is substituted, so it
//! is left as the shorthand for the host to split once it is resolved.

/// The shorthands that expand to four sides, and the longhands they stand for,
/// in CSS order: top, right, bottom, left.
const SIDED: [(&str, [&str; 4]); 2] = [
    (
        "margin",
        ["margin-top", "margin-right", "margin-bottom", "margin-left"],
    ),
    (
        "padding",
        [
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ],
    ),
];

/// The longhands `name: value` stands for, or `None` when it is not a sided
/// shorthand (or cannot be split yet).
#[must_use]
pub fn expand(name: &str, value: &str) -> Option<Vec<(&'static str, String)>> {
    let (_, longhands) = SIDED.iter().find(|(shorthand, _)| *shorthand == name)?;
    if value.contains("var(") {
        return None;
    }
    let sides = split_sides(value)?;
    Some(
        longhands
            .iter()
            .copied()
            .zip(sides.map(str::to_owned))
            .collect(),
    )
}

/// One to four values as top, right, bottom, left, the way CSS reads them:
/// one is every side, two are vertical then horizontal, three are top, the
/// sides, then bottom.
#[must_use]
pub fn split_sides(value: &str) -> Option<[&str; 4]> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.as_slice() {
        [all] => Some([all, all, all, all]),
        [vertical, horizontal] => Some([vertical, horizontal, vertical, horizontal]),
        [top, horizontal, bottom] => Some([top, horizontal, bottom, horizontal]),
        [top, right, bottom, left] => Some([top, right, bottom, left]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{expand, split_sides};

    #[test]
    fn one_to_four_values_read_as_css_reads_them() {
        assert_eq!(split_sides("1px"), Some(["1px"; 4]));
        assert_eq!(split_sides("1px 2px"), Some(["1px", "2px", "1px", "2px"]));
        assert_eq!(
            split_sides("1px 2px 3px"),
            Some(["1px", "2px", "3px", "2px"])
        );
        assert_eq!(
            split_sides("1px 2px 3px 4px"),
            Some(["1px", "2px", "3px", "4px"])
        );
        assert_eq!(split_sides("1px 2px 3px 4px 5px"), None);
    }

    #[test]
    fn padding_becomes_four_longhands_and_a_variable_waits() {
        let expanded = expand("padding", "4px 8px").expect("padding is sided");
        assert_eq!(expanded[0], ("padding-top", "4px".to_owned()));
        assert_eq!(expanded[3], ("padding-left", "8px".to_owned()));
        assert_eq!(expand("padding", "var(--gap)"), None);
        assert_eq!(expand("width", "4px"), None);
    }
}
