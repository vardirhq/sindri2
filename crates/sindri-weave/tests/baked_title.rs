//! The baked variant's title screen, composed the way a browser composes it.
//!
//! The native capture lays the title out from the scene alone. Weave is only
//! applied in the browser, and only the portrait rules are applied on a phone —
//! so a stylesheet mistake in a media block is invisible to every other test in
//! the repository and to every screenshot taken on a desktop.
//!
//! It cost exactly that. The menu grew a BOSS RUSH row and a boss picker, each
//! carrying `title-action` alongside its own class so it inherits the button's
//! look. The portrait block overrides `.title-action` and had no rules for the
//! new rows; Weave settles equal specificity by source order, and the portrait
//! override is further down the sheet than `.title-rush`, so `y: -24vw` was the
//! last word for all three. On a phone they drew on top of one another —
//! "OPENS ON WARDEN", "BOSS RUSH" and "START" in one illegible pile on a single
//! button.

use std::collections::BTreeMap;

use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Stylesheet, Viewport, compose};

const SCENE: &str = include_str!("../../../games/orbital-baked/assets/orbital.scene.json");
const STYLE_ENTRY: &str = include_str!("../../../games/orbital-baked/assets/ui.weave");
const STYLE_HUD: &str = include_str!("../../../games/orbital-baked/assets/ui/hud.weave");
const STYLE_OVERLAYS: &str = include_str!("../../../games/orbital-baked/assets/ui/overlays.weave");
const STYLE_SCREENS: &str = include_str!("../../../games/orbital-baked/assets/ui/screens.weave");

fn stylesheet() -> Stylesheet {
    let sources = BTreeMap::from([
        ("ui.weave".to_owned(), STYLE_ENTRY.to_owned()),
        ("ui/hud.weave".to_owned(), STYLE_HUD.to_owned()),
        ("ui/overlays.weave".to_owned(), STYLE_OVERLAYS.to_owned()),
        ("ui/screens.weave".to_owned(), STYLE_SCREENS.to_owned()),
    ]);
    compose("ui.weave", &sources).expect("the baked variant's Weave composes")
}

/// Where a UI element sits and how tall it is, after composition.
fn band(world: &World, id: &str) -> (f32, f32) {
    let data = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|source_id| source_id.as_str() == id)
        })
        .map(|(_, data)| data)
        .unwrap_or_else(|| panic!("the scene has a {id}"));
    let transform = data.transform_3d.expect("a UI element has a transform");
    (transform.position[1], transform.scale_2d()[1])
}

fn composed(viewport: Viewport) -> World {
    let document = SceneDocument::from_json(SCENE).expect("the baked scene parses");
    let authored = World::from_scene(&document)
        .expect("the baked scene loads")
        .world;
    PresentationWorld::resolve(&authored, &stylesheet(), viewport)
        .expect("the title resolves")
        .world()
        .clone()
}

/// The three menu rows, top to bottom, and the hint under them.
const ROWS: [&str; 4] = ["title-start", "title-rush", "title-pick", "title-hint"];

fn assert_rows_are_stacked(world: &World, where_: &str) {
    let mut previous: Option<(&str, f32)> = None;
    for id in ROWS {
        let (y, height) = band(world, id);
        let top = y + height / 2.0;
        if let Some((above, bottom_of_above)) = previous {
            assert!(
                top < bottom_of_above,
                "{where_}: {id} starts at {top} but {above} only ends at \
                 {bottom_of_above} — the rows overlap"
            );
        }
        previous = Some((id, y - height / 2.0));
    }
}

#[test]
fn the_title_rows_do_not_overlap_on_a_phone() {
    // The viewport `it_fits_a_phone` holds the game to, which is also the one
    // the phone screenshots are taken at.
    let world = composed(Viewport {
        width: 390.0,
        height: 844.0,
    });
    assert_rows_are_stacked(&world, "portrait");
}

#[test]
fn the_title_rows_do_not_overlap_on_a_desktop() {
    let world = composed(Viewport {
        width: 1280.0,
        height: 720.0,
    });
    assert_rows_are_stacked(&world, "landscape");
}

/// Each row's label rides on its own row rather than on a neighbour's.
///
/// Separate from the rows themselves because a label carries its own class and
/// its own portrait rule: the buttons could be spaced correctly while the text
/// on them was not, which is a different mistake with the same symptom.
#[test]
fn every_label_sits_on_its_own_row_on_a_phone() {
    let world = composed(Viewport {
        width: 390.0,
        height: 844.0,
    });
    for (row, label) in [
        ("title-start", "title-start-text"),
        ("title-rush", "title-rush-text"),
        ("title-pick", "title-pick-text"),
    ] {
        let (row_y, row_height) = band(&world, row);
        let (label_y, label_height) = band(&world, label);
        assert!(
            label_y + label_height / 2.0 <= row_y + row_height / 2.0 + 1.0e-4
                && label_y - label_height / 2.0 >= row_y - row_height / 2.0 - 1.0e-4,
            "{label} is not inside {row}: the label spans \
             {}..{} and the button {}..{}",
            label_y - label_height / 2.0,
            label_y + label_height / 2.0,
            row_y - row_height / 2.0,
            row_y + row_height / 2.0
        );
    }
}
