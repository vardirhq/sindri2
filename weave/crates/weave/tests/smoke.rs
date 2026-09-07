use weave::{Viewport, parse};

#[test]
fn mobile_rule_matches_small_viewport() {
    let sheet = parse("#menu { width: 420px; } @media (max-width: 700px) { #menu { width: 90vw; } }")
        .expect("valid Weave");
    let mobile = Viewport { width: 390.0, height: 844.0 };
    assert_eq!(sheet.rules.iter().filter(|rule| rule.applies("menu", &[], mobile)).count(), 2);
}
