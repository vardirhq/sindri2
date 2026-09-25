//! Pseudo-classes about where an element sits in the tree: `:root`,
//! `:first-child`, `:last-child`, `:only-child`, `:nth-child()` and
//! `:nth-last-child()`, read as CSS reads them.

/// Where an element sits among its parent's children: its index from zero,
/// and how many children there are. An element with no parent is the only
/// child of the document, as CSS's root is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub index: usize,
    pub count: usize,
}

/// One structural condition in a compound selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Structural {
    /// The `an+b`th child, counted from the first or, `from_end`, the last.
    /// `:first-child` is `0n+1`; `:last-child` is the same from the end.
    Nth { a: i32, b: i32, from_end: bool },
    /// The only child of its parent.
    Only,
    /// An element with no parent: the top of the tree, where CSS's
    /// variables are usually declared.
    Root,
}

impl Structural {
    pub(crate) fn matches(self, position: Option<Position>) -> bool {
        let Some(Position { index, count }) = position else {
            return false;
        };
        match self {
            Self::Only => count == 1,
            // Answered from the tree, not a position: see `Compound::matches`.
            Self::Root => false,
            Self::Nth { a, b, from_end } => {
                let ordinal = if from_end { count - index } else { index + 1 };
                let Ok(ordinal) = i32::try_from(ordinal) else {
                    return false;
                };
                // Is there a whole n >= 0 with a*n + b == ordinal?
                if a == 0 {
                    return ordinal == b;
                }
                let offset = ordinal - b;
                offset % a == 0 && offset / a >= 0
            }
        }
    }

    /// The pseudo-class `name` with argument `argument`, if it is one of these.
    pub(crate) fn parse(name: &str, argument: Option<&str>) -> Option<Self> {
        let first = Self::Nth {
            a: 0,
            b: 1,
            from_end: false,
        };
        match (name, argument) {
            ("first-child", None) => Some(first),
            ("last-child", None) => Some(Self::Nth {
                a: 0,
                b: 1,
                from_end: true,
            }),
            ("only-child", None) => Some(Self::Only),
            ("root", None) => Some(Self::Root),
            ("nth-child", Some(formula)) => {
                let (a, b) = parse_nth(formula)?;
                Some(Self::Nth {
                    a,
                    b,
                    from_end: false,
                })
            }
            ("nth-last-child", Some(formula)) => {
                let (a, b) = parse_nth(formula)?;
                Some(Self::Nth {
                    a,
                    b,
                    from_end: true,
                })
            }
            _ => None,
        }
    }
}

/// `an+b` as CSS writes it: `odd`, `even`, `3`, `2n+1`, `-n+3`, `n`.
fn parse_nth(text: &str) -> Option<(i32, i32)> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    match compact.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    let Some(n_at) = compact.find('n') else {
        return compact.parse().ok().map(|b| (0, b));
    };
    let a = match &compact[..n_at] {
        "" | "+" => 1,
        "-" => -1,
        written => written.parse().ok()?,
    };
    let rest = &compact[n_at + 1..];
    let b = if rest.is_empty() {
        0
    } else {
        // A sign is required between the parts: `2n+1`, not `2n1`.
        if !rest.starts_with(['+', '-']) {
            return None;
        }
        rest.parse().ok()?
    };
    Some((a, b))
}

#[cfg(test)]
mod tests {
    use super::{Position, Structural, parse_nth};

    fn picks(structural: Structural, count: usize) -> Vec<usize> {
        (0..count)
            .filter(|index| {
                structural.matches(Some(Position {
                    index: *index,
                    count,
                }))
            })
            .map(|index| index + 1)
            .collect()
    }

    #[test]
    fn formulas_read_as_css_writes_them() {
        assert_eq!(parse_nth("odd"), Some((2, 1)));
        assert_eq!(parse_nth("even"), Some((2, 0)));
        assert_eq!(parse_nth("3"), Some((0, 3)));
        assert_eq!(parse_nth("2n + 1"), Some((2, 1)));
        assert_eq!(parse_nth("-n+3"), Some((-1, 3)));
        assert_eq!(parse_nth("n"), Some((1, 0)));
        assert_eq!(parse_nth("3n-1"), Some((3, -1)));
        assert_eq!(parse_nth("2n1"), None);
        assert_eq!(parse_nth("wide"), None);
    }

    #[test]
    fn each_picks_the_children_css_does() {
        let parse = |name: &str, argument: Option<&str>| {
            Structural::parse(name, argument).expect("a structural pseudo-class")
        };
        assert_eq!(picks(parse("first-child", None), 5), [1]);
        assert_eq!(picks(parse("last-child", None), 5), [5]);
        assert_eq!(picks(parse("nth-child", Some("odd")), 5), [1, 3, 5]);
        assert_eq!(picks(parse("nth-child", Some("even")), 5), [2, 4]);
        assert_eq!(picks(parse("nth-child", Some("-n+3")), 5), [1, 2, 3]);
        assert_eq!(picks(parse("nth-child", Some("3n")), 7), [3, 6]);
        assert_eq!(picks(parse("nth-last-child", Some("2")), 5), [4]);
        assert!(picks(parse("only-child", None), 2).is_empty());
        assert_eq!(picks(parse("only-child", None), 1), [1]);
        // An element with no position, in a host that does not say, matches
        // none of them.
        assert!(!parse("first-child", None).matches(None));
    }
}
