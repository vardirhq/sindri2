use weave::{Viewport, parse};

#[test]
fn mobile_media_rule_overrides_base_rule() {
    let sheet = parse("#menu { width: 420px; } @media (max-width: 700px) { #menu { width: 90vw; } }")
        .expect("valid Weave");
    let mobile = Viewport { width: 390.0, height: 844.0 };
    let matching: Vec<_> = sheet.rules.iter().filter(|rule| rule.applies("menu", &[], mobile)).collect();
    assert_eq!(matching.len(), 2);
}
