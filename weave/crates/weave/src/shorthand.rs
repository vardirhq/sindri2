//! Shorthands that stand for several declarations.
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

/// The longhands `name: value` stands for, or `None` when it is not a
/// shorthand (or cannot be split yet).
#[must_use]
pub fn expand(name: &str, value: &str) -> Option<Vec<(&'static str, String)>> {
    if value.contains("var(") {
        return None;
    }
    if name == "flex" {
        return flex(value);
    }
    let (_, longhands) = SIDED.iter().find(|(shorthand, _)| *shorthand == name)?;
    let sides = split_sides(value)?;
    Some(
        longhands
            .iter()
            .copied()
            .zip(sides.map(str::to_owned))
            .collect(),
    )
}

/// `flex` as CSS reads it: `none`, `auto`, or a grow factor, then optionally
/// a shrink factor, then optionally a basis. A lone number means a basis of
/// zero, so `flex: 1` children share a line equally whatever their sizes.
fn flex(value: &str) -> Option<Vec<(&'static str, String)>> {
    let (grow, shrink, basis) = match value.trim() {
        "none" => ("0", "0", "auto"),
        "auto" => ("1", "1", "auto"),
        "initial" => ("0", "1", "auto"),
        other => {
            let parts: Vec<&str> = other.split_whitespace().collect();
            let number = |text: &str| text.parse::<f32>().is_ok();
            match parts.as_slice() {
                [grow] if number(grow) => (*grow, "1", "0"),
                [basis] => ("1", "1", *basis),
                [grow, shrink] if number(grow) && number(shrink) => (*grow, *shrink, "0"),
                [grow, basis] if number(grow) => (*grow, "1", *basis),
                [grow, shrink, basis] if number(grow) && number(shrink) => (*grow, *shrink, *basis),
                _ => return None,
            }
        }
    };
    Some(vec![
        ("flex-grow", grow.to_owned()),
        ("flex-shrink", shrink.to_owned()),
        ("flex-basis", basis.to_owned()),
    ])
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

    #[test]
    fn flex_reads_as_css_reads_it() {
        let read = |value: &str| -> Vec<String> {
            expand("flex", value)
                .expect("flex expands")
                .into_iter()
                .map(|(_, value)| value)
                .collect()
        };
        assert_eq!(read("1"), ["1", "1", "0"]);
        assert_eq!(read("2 0"), ["2", "0", "0"]);
        assert_eq!(read("1 200px"), ["1", "1", "200px"]);
        assert_eq!(read("0 0 50%"), ["0", "0", "50%"]);
        assert_eq!(read("none"), ["0", "0", "auto"]);
        assert_eq!(read("auto"), ["1", "1", "auto"]);
        assert_eq!(expand("flex", "1 2 3 4"), None);
    }
}
