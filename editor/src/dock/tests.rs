//! What an arrangement must always be true of, however it was reached.

use super::*;

#[test]
fn the_default_arrangement_places_every_panel() {
    let workspace = Workspace::default();
    for panel in Panel::ALL {
        assert!(
            workspace.is_open(panel),
            "{} is not in the default workspace, so nothing would show it",
            panel.label()
        );
    }
}

/// The console beside the scene rather than under the project browser: what
/// the console says is about the thing in the viewport next to it.
#[test]
fn the_console_opens_beside_the_scene() {
    let workspace = Workspace::preset(Preset::Docked);
    let (scene, _) = workspace
        .location(Panel::Scene)
        .expect("the scene is placed");
    let (console, _) = workspace
        .location(Panel::Console)
        .expect("the console is placed");
    assert_eq!(scene, console);
    assert_eq!(scene, Place::MAIN);
}

#[test]
fn both_presets_place_every_panel() {
    for preset in Preset::ALL {
        let workspace = Workspace::preset(preset);
        for panel in Panel::ALL {
            assert!(
                workspace.is_open(panel),
                "{} is missing from the {} preset",
                panel.label(),
                preset.label()
            );
        }
    }
}

#[test]
fn moving_a_panel_takes_it_out_of_where_it_was() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace.place(Panel::Inspector, Place::Dock(Slot::Left), 0);
    assert_eq!(
        workspace.location(Panel::Inspector),
        Some((Place::Dock(Slot::Left), 0))
    );
    assert!(
        workspace.group(Place::Dock(Slot::FarRight)).is_none(),
        "the slot it left held only the inspector, so it is now empty and undrawn"
    );
}

#[test]
fn a_panel_dropped_between_two_tabs_lands_between_them() {
    let mut workspace = Workspace::preset(Preset::Wide);
    workspace.place(Panel::Inspector, Place::Dock(Slot::Bottom), 1);
    let group = workspace
        .group(Place::Dock(Slot::Bottom))
        .expect("the bottom dock is drawn");
    assert_eq!(
        group.panels,
        vec![
            Panel::Project,
            Panel::Inspector,
            Panel::Console,
            Panel::History
        ]
    );
}

/// Dropping a tab selects it. Landing somewhere and staying hidden behind the
/// tab that was already there reads as the drop having failed.
#[test]
fn a_dropped_panel_is_the_one_showing() {
    let mut workspace = Workspace::preset(Preset::Wide);
    workspace.place(Panel::Inspector, Place::Dock(Slot::Bottom), 2);
    let group = workspace
        .group(Place::Dock(Slot::Bottom))
        .expect("the bottom dock is drawn");
    assert_eq!(group.selected(), Some(Panel::Inspector));
}

#[test]
fn closing_the_selected_tab_shows_a_neighbour() {
    let mut workspace = Workspace::preset(Preset::Wide);
    workspace.select(Place::Dock(Slot::Bottom), 2);
    assert!(workspace.take(Panel::History));
    let group = workspace
        .group(Place::Dock(Slot::Bottom))
        .expect("two tabs are left");
    assert_eq!(group.selected(), Some(Panel::Console));
}

/// The centre is what every other slot is measured against. An editor with a
/// hole where the work goes is not a smaller editor, so neither the drag nor
/// the View menu is allowed to produce one.
#[test]
fn the_centre_cannot_be_emptied() {
    let mut workspace = Workspace::preset(Preset::Docked);
    assert!(workspace.take(Panel::Console), "the centre had two tabs");
    assert!(!workspace.take(Panel::Scene), "and now it has one");
    assert_eq!(
        workspace.group(Place::MAIN).map(Group::selected),
        Some(Some(Panel::Scene))
    );
}

#[test]
fn the_last_centre_tab_cannot_be_dragged_out_either() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace.take(Panel::Console);
    workspace.place(Panel::Scene, Place::Dock(Slot::Left), 0);
    assert_eq!(
        workspace.location(Panel::Scene),
        Some((Place::MAIN, 0)),
        "the drag is refused rather than obeyed and then undone"
    );
}

#[test]
fn a_closed_panel_reopens_in_the_centre() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace.toggle(Panel::Inspector);
    assert!(!workspace.is_open(Panel::Inspector));
    workspace.toggle(Panel::Inspector);
    assert_eq!(
        workspace.location(Panel::Inspector).map(|(slot, _)| slot),
        Some(Place::MAIN)
    );
}

#[test]
fn a_slot_remembers_the_size_it_was_dragged_to() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace.resize(Place::Dock(Slot::Right), 512.0);
    assert_eq!(
        workspace
            .group(Place::Dock(Slot::Right))
            .map(|group| group.size),
        Some(512.0)
    );
}

/// Settings are a file on disk, and a file on disk can say anything.
/// A panel is one region of the window. Two of them would be two views of one
/// piece of state fighting over the same scroll position and the same
/// selection, so a settings file asking for that gets one.
#[test]
fn a_settings_file_naming_a_panel_twice_keeps_one_of_them() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace
        .group_mut(Place::Dock(Slot::Left))
        .panels
        .push(Panel::Scene);
    workspace.repair();
    let places = Place::all()
        .filter(|slot| {
            workspace
                .group(*slot)
                .is_some_and(|group| group.panels.contains(&Panel::Scene))
        })
        .count();
    assert_eq!(places, 1);
}

/// And the centre still has something in it afterwards, however the duplicate
/// was resolved.
#[test]
fn deduplicating_never_leaves_the_centre_empty() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace
        .group_mut(Place::Dock(Slot::Left))
        .panels
        .push(Panel::Scene);
    workspace
        .group_mut(Place::Dock(Slot::Left))
        .panels
        .push(Panel::Console);
    workspace.repair();
    assert!(workspace.group(Place::MAIN).is_some());
}

#[test]
fn a_settings_file_with_an_empty_centre_is_given_one() {
    let mut workspace = Workspace {
        places: Vec::new(),
        chrome: Chrome::Floating,
    };
    workspace.repair();
    assert!(
        workspace.group(Place::MAIN).is_some(),
        "an editor with nothing in the middle must not be openable"
    );
}

#[test]
fn a_settings_file_with_a_nonsense_size_falls_back() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace.group_mut(Place::Dock(Slot::Right)).size = f32::NAN;
    workspace.group_mut(Place::Dock(Slot::Left)).size = -40.0;
    workspace.repair();
    assert_eq!(
        workspace
            .group(Place::Dock(Slot::Right))
            .map(|group| group.size),
        Some(Place::Dock(Slot::Right).default_size())
    );
    assert_eq!(
        workspace
            .group(Place::Dock(Slot::Left))
            .map(|group| group.size),
        Some(Place::Dock(Slot::Left).min_size())
    );
}

#[test]
fn a_settings_file_selecting_a_tab_that_is_gone_selects_one_that_is_there() {
    let mut workspace = Workspace::preset(Preset::Docked);
    workspace.group_mut(Place::Dock(Slot::Left)).active = 9;
    workspace.repair();
    assert_eq!(
        workspace
            .group(Place::Dock(Slot::Left))
            .map(Group::selected),
        Some(Some(Panel::Hierarchy))
    );
}

#[test]
fn an_arrangement_survives_a_round_trip() {
    let mut workspace = Workspace::preset(Preset::Wide);
    workspace.place(Panel::Console, Place::MAIN, 1);
    workspace.resize(Place::Dock(Slot::Left), 333.0);
    let text = serde_json::to_string(&workspace).expect("an arrangement serialises");
    assert_eq!(
        serde_json::from_str::<Workspace>(&text).expect("and reads back"),
        workspace
    );
}

/// A gesture that lights up and then does nothing is worse than one that never
/// lights up, so the refusal is answerable before the drop is made.
#[test]
fn a_move_the_workspace_would_refuse_can_be_asked_about_first() {
    let mut workspace = Workspace::preset(Preset::Docked);
    assert!(workspace.can_place(Panel::Scene, Place::Dock(Slot::Left)));
    workspace.take(Panel::Console);
    assert!(
        !workspace.can_place(Panel::Scene, Place::Dock(Slot::Left)),
        "the centre's last tab has nowhere else to go"
    );
    assert!(
        workspace.can_place(Panel::Scene, Place::MAIN),
        "but it can still be reordered within the centre"
    );
    assert!(
        workspace.can_place(Panel::Hierarchy, Place::Dock(Slot::Left)),
        "and every other panel moves as usual"
    );
}

/// Canvas means nothing docks. The first attempt moved two panels into floating
/// boxes and left the title bar, the tab strip, the toolbar, the inspector
/// column and the status bar all claiming their rows — a docked editor with two
/// insets, in which the scene had about half the window.
#[test]
fn the_canvas_arrangement_lets_nothing_claim_space_from_the_scene() {
    let workspace = Workspace::preset(Preset::Canvas);
    assert_eq!(workspace.chrome(), Chrome::Floating);
    for place in Place::all() {
        if place == Place::MAIN {
            continue;
        }
        assert!(
            workspace.group(place).is_none() || place.is_overlay(),
            "{place:?} docks, so the scene does not have the window"
        );
    }
}

/// And the arrangements that are meant to dock still do.
#[test]
fn the_docked_arrangements_keep_their_furniture() {
    for preset in [Preset::Docked, Preset::Wide] {
        assert_eq!(Workspace::preset(preset).chrome(), Chrome::Docked);
    }
}
