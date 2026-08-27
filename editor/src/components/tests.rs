//! That the table stays in step with the engine as components are added.

use std::collections::BTreeSet;

use super::{Family, KNOWN, family, icon};
use crate::ui::icons;

/// The promise the table makes: every component the editor registers has a
/// family and a glyph.
///
/// This is the test that makes adding a component to the engine safe. Without
/// it, a new type arrives with no family — so the Add Component menu lists it
/// loose at the top level, which reads as an oversight — and with the generic
/// entity box for an icon, which is what had already happened to audio,
/// rigid bodies and colliders before there was a table to check.
#[test]
fn every_registered_component_has_a_family_and_a_glyph() {
    let extractor = crate::native::scene_extractor();
    let missing: Vec<&str> = extractor
        .components()
        .registered_components()
        .map(|metadata| metadata.type_name.as_str())
        .filter(|type_name| family(type_name).is_none())
        .collect();

    assert!(
        missing.is_empty(),
        "add a row to `KNOWN` for each of these: {missing:?}"
    );
}

/// And nothing in the table names a component that no longer exists, which is
/// the same drift from the other end.
#[test]
fn the_table_names_no_component_the_engine_has_dropped() {
    let extractor = crate::native::scene_extractor();
    let registered: BTreeSet<&str> = extractor
        .components()
        .registered_components()
        .map(|metadata| metadata.type_name.as_str())
        .collect();
    let stale: Vec<&str> = KNOWN
        .iter()
        .map(|known| known.type_name)
        .filter(|type_name| !registered.contains(type_name))
        .collect();

    assert!(stale.is_empty(), "no longer registered: {stale:?}");
}

/// A family with one member is a submenu holding one entry, which is worse
/// than the flat list it replaced. Every family has to earn its submenu.
#[test]
fn no_family_holds_only_one_component() {
    for wanted in Family::ALL {
        let held = KNOWN.iter().filter(|known| known.family == wanted).count();
        assert!(
            held > 1,
            "{} holds {held}; a submenu of one is worse than no submenu",
            wanted.label()
        );
    }
}

/// A component nothing has heard of still draws, because a scene may carry one
/// from a newer tool and the inspector still gives it a header.
#[test]
fn an_unknown_component_still_gets_a_glyph() {
    assert!(family("game.something.new").is_none());
    assert_eq!(
        icon("game.something.new").codepoint,
        icons::ENTITY.codepoint
    );
}

/// Two components that mean different things should not draw the same, which
/// is the drift the table exists to stop. Audio and sprite animation shared
/// the play glyph until this table put them side by side.
#[test]
fn components_that_mean_different_things_look_different() {
    assert_ne!(
        icon("sindri.audio.source").codepoint,
        icon("sindri.animation.sprite").codepoint
    );
    assert_ne!(
        icon("sindri.physics2d.rigid_body").codepoint,
        icon("sindri.physics2d.collider").codepoint
    );
}
