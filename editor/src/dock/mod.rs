//! Which panel is where, and what the user is allowed to do about it.
//!
//! The workspace used to be two hardcoded arrangements: a `match` in the frame
//! loop named the panels in the order they claimed space, and changing that
//! order meant changing the editor. Anyone who wanted the console beside the
//! scene rather than stacked under the project browser could not have it, and
//! anyone whose screen was a different shape from the one the arrangement was
//! written for got a worse editor for it.
//!
//! So the arrangement is data. A [`Workspace`] says which panels live in which
//! [`Place`] and how big each is; the frame loop reads it rather than knowing
//! it. The arrangements that used to be the only choices are now
//! [presets](Workspace::preset) — starting points someone can drag away from,
//! rather than the only shapes the editor has.
//!
//! A panel is placed one of two ways, and the difference is whether it takes
//! room from the scene or covers it. A [`Slot`] is a dock: it claims space, and
//! the scene view gets what is left. A [`Corner`] is an overlay: it floats over
//! the scene, anchored to one of its corners, and the scene keeps the whole
//! window. Overlays are what makes the canvas-first arrangement expressible
//! without a second editor — see `docs/editor-direction.md`.
//!
//! Overlays anchor rather than float freely, and stack when they share a
//! corner. Free-floating panels do not overlap in a drawing because someone
//! placed them; they overlap constantly in use, and an editor whose panels can
//! be piled on each other by accident is one where rearranging furniture
//! becomes the work.

mod drag;
mod place;

pub use drag::{Drag, DropTarget, edge_place, tab_index};
pub use place::{Chrome, Corner, Place, Slot};

use serde::{Deserialize, Serialize};

/// One panel the workspace can place.
///
/// A closed set rather than anything extensible: every one of these is a region
/// the editor knows how to draw, and a workspace naming a panel that does not
/// exist is a workspace that cannot be drawn. Persisted by name, so adding one
/// here does not invalidate anyone's saved arrangement.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Panel {
    /// The world as a place to work in.
    Scene,
    /// The world as the player would see it.
    Game,
    /// The entities in the scene, and their parentage.
    Hierarchy,
    /// The selected entity's components, or whatever else is being previewed.
    Inspector,
    /// The project's files.
    Project,
    /// What the editor and the running game have said.
    Console,
    /// Every step taken, and the one we are standing on.
    History,
}

impl Panel {
    /// Every panel, in the order a menu should offer them.
    pub const ALL: [Self; 7] = [
        Self::Scene,
        Self::Game,
        Self::Hierarchy,
        Self::Inspector,
        Self::Project,
        Self::Console,
        Self::History,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Game => "Game",
            Self::Hierarchy => "Hierarchy",
            Self::Inspector => "Inspector",
            Self::Project => "Project",
            Self::Console => "Console",
            Self::History => "History",
        }
    }

    /// Whether the panel draws the world through the GPU.
    ///
    /// Worth asking because a viewport costs a render target and a pass
    /// whenever it is on screen, and the editor declines to pay for one nobody
    /// is looking at: a viewport on an unselected tab is not drawn at all.
    pub const fn is_viewport(self) -> bool {
        matches!(self, Self::Scene | Self::Game)
    }
}

/// One place's contents: the panels in it, and which of them is showing.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Group {
    /// The panels here, in tab order.
    pub panels: Vec<Panel>,
    /// Which tab is selected, as an index into `panels`.
    ///
    /// An index rather than a `Panel` so that a group holding the same panel
    /// twice — which cannot happen, but which a hand-edited settings file could
    /// ask for — still resolves to one tab. Clamped on read rather than trusted.
    pub active: usize,
    /// How big it is along its own axis, in points: a docked column's width, a
    /// docked row's height, an overlay's width.
    pub size: f32,
    /// How tall an overlay is. Docks take the whole of their cross axis and
    /// ignore this.
    pub height: f32,
    /// Whether an overlay is rolled up to just its tab strip.
    ///
    /// The answer to the honest objection to overlays: they cover the world.
    /// Clicking the showing tab rolls one up, so getting the scene back is the
    /// same gesture as putting the panel away and costs no travel to a control
    /// somewhere else. Docks ignore this — collapsing one is what dragging its
    /// edge already does.
    pub collapsed: bool,
}

impl Group {
    fn new(place: Place, panels: &[Panel]) -> Self {
        Self {
            panels: panels.to_vec(),
            active: 0,
            size: place.default_size(),
            height: place.default_height(),
            collapsed: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }

    /// The panel whose contents should be drawn, if any.
    pub fn selected(&self) -> Option<Panel> {
        self.panels
            .get(self.active.min(self.panels.len().saturating_sub(1)))
            .copied()
    }

    /// The selected index, clamped into the panels that are actually there.
    pub fn active_index(&self) -> usize {
        self.active.min(self.panels.len().saturating_sub(1))
    }
}

/// Where every panel is, and how much room its slot has.
///
/// Persisted with the rest of the preferences: an arrangement someone dragged
/// into shape is a decision made once, and making it again every launch is the
/// thing settings exist to stop.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Workspace {
    /// One entry per place. A place with no entry is empty and is not drawn.
    places: Vec<(Place, Group)>,
    /// Whether the window's furniture takes room from the scene or floats over
    /// it.
    ///
    /// Part of the arrangement rather than a separate setting, because it is
    /// the same question every other entry here answers: does this take space,
    /// or cover it. A preset sets both together, and they are not independently
    /// meaningful — floating panels around a docked title bar is neither shape.
    chrome: Chrome,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::preset(Preset::Canvas)
    }
}

/// An arrangement to start from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    /// The scene has the window; everything else overlays a corner of it or is
    /// one click away.
    ///
    /// The default, and the direction the editor is being taken in: a scene
    /// view that is the document rather than the rectangle five panels left
    /// over. The inspector stays docked on purpose — see
    /// `docs/editor-direction.md` for why properties want an edge and verbs do
    /// not.
    #[default]
    Canvas,
    /// Every panel docked, taking room from the scene: hierarchy left, both
    /// views centre, project and inspector right.
    ///
    /// What the editor was before the canvas direction, kept as a preset rather
    /// than archived in a folder. Changing your mind about the default should
    /// cost a menu click, not a checkout.
    Docked,
    /// One view at a time, with the project browser along the bottom.
    ///
    /// The whole width for the viewport, which suits a laptop screen or working
    /// on one view without the other competing for attention.
    Wide,
}

impl Preset {
    pub const ALL: [Self; 3] = [Self::Canvas, Self::Docked, Self::Wide];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Canvas => "Canvas",
            Self::Docked => "Docked",
            Self::Wide => "Wide",
        }
    }

    /// What choosing it would do, for the menu that offers it.
    pub const fn note(self) -> &'static str {
        match self {
            Self::Canvas => "The scene fills the window; panels overlay its corners",
            Self::Docked => "Every panel takes its own room beside the scene",
            Self::Wide => "One view at a time, project browser along the bottom",
        }
    }
}

impl Workspace {
    /// How the window's furniture is drawn in this arrangement.
    pub const fn chrome(&self) -> Chrome {
        self.chrome
    }

    /// One of the arrangements the editor ships with.
    pub fn preset(preset: Preset) -> Self {
        use Corner::{BottomLeft, TopLeft, TopRight};
        use Slot::{Bottom, FarRight, Left, Main, MainBottom, Right};
        let places: &[(Place, &[Panel])] = match preset {
            Preset::Canvas => &[
                // Nothing docks. The scene is the window and every other
                // surface is drawn over it, the title bar and the status bar
                // included -- see `Chrome::Floating`.
                //
                // The inspector floats here but is *anchored*, not attached to
                // the selection. Anchoring is what keeps the thing a mockup
                // gets right (the scene is the document) without the thing it
                // gets wrong: a panel that jumps to whatever was last clicked
                // forms no muscle memory and covers the neighbours a value is
                // being judged against.
                (Place::Dock(Main), &[Panel::Scene, Panel::Game]),
                (Place::Overlay(TopLeft), &[Panel::Hierarchy]),
                (
                    Place::Overlay(BottomLeft),
                    &[Panel::Project, Panel::Console, Panel::History],
                ),
                (Place::Overlay(TopRight), &[Panel::Inspector]),
            ],
            Preset::Docked => &[
                (Place::Dock(Left), &[Panel::Hierarchy]),
                (Place::Dock(Main), &[Panel::Scene, Panel::Console]),
                (Place::Dock(MainBottom), &[Panel::Game]),
                (Place::Dock(Right), &[Panel::Project, Panel::History]),
                (Place::Dock(FarRight), &[Panel::Inspector]),
            ],
            Preset::Wide => &[
                (Place::Dock(Left), &[Panel::Hierarchy]),
                (Place::Dock(Main), &[Panel::Scene, Panel::Game]),
                (
                    Place::Dock(Bottom),
                    &[Panel::Project, Panel::Console, Panel::History],
                ),
                (Place::Dock(FarRight), &[Panel::Inspector]),
            ],
        };
        let mut workspace = Self {
            places: places
                .iter()
                .map(|(place, panels)| (*place, Group::new(*place, panels)))
                .collect(),
            chrome: match preset {
                Preset::Canvas => Chrome::Floating,
                Preset::Docked | Preset::Wide => Chrome::Docked,
            },
        };
        if preset == Preset::Canvas {
            // An inspector is a column of fields, and the default overlay
            // height is a note's worth. Given its own figure rather than a
            // taller default for every overlay, which would make the hierarchy
            // and the project browser cover the scene for no reason.
            let inspector = workspace.group_mut(Place::Overlay(TopRight));
            inspector.size = 320.0;
            inspector.height = 560.0;
        }
        workspace
    }

    /// The group in a place, if it holds anything.
    pub fn group(&self, place: Place) -> Option<&Group> {
        self.places
            .iter()
            .find(|(candidate, group)| *candidate == place && !group.is_empty())
            .map(|(_, group)| group)
    }

    fn group_mut(&mut self, place: Place) -> &mut Group {
        if let Some(index) = self
            .places
            .iter()
            .position(|(candidate, _)| *candidate == place)
        {
            return &mut self.places[index].1;
        }
        self.places.push((place, Group::new(place, &[])));
        let last = self.places.len() - 1;
        &mut self.places[last].1
    }

    /// Selects a tab, and unrolls the overlay it is in if it was rolled up.
    ///
    /// Choosing a tab is asking to see it. An overlay that stayed collapsed
    /// after one was picked would have answered a click with nothing.
    pub fn select(&mut self, place: Place, index: usize) {
        let group = self.group_mut(place);
        group.active = index;
        group.collapsed = false;
    }

    /// Rolls an overlay up to its tab strip, or back down.
    pub fn toggle_collapsed(&mut self, place: Place) {
        if place.is_overlay() {
            let group = self.group_mut(place);
            group.collapsed = !group.collapsed;
        }
    }

    /// Records a size after the user has dragged an edge.
    pub fn resize(&mut self, place: Place, size: f32) {
        self.group_mut(place).size = size;
    }

    /// Records an overlay's height after the user has dragged its edge.
    pub fn resize_height(&mut self, place: Place, height: f32) {
        self.group_mut(place).height = height;
    }

    /// Where a panel currently lives, if it is placed at all.
    pub fn location(&self, panel: Panel) -> Option<(Place, usize)> {
        self.places.iter().find_map(|(place, group)| {
            group
                .panels
                .iter()
                .position(|candidate| *candidate == panel)
                .map(|index| (*place, index))
        })
    }

    pub fn is_open(&self, panel: Panel) -> bool {
        self.location(panel).is_some()
    }

    /// Takes a panel out of wherever it is.
    ///
    /// The centre is refused when it would empty it. Every other place is
    /// measured against the centre, so a workspace with nothing in the middle
    /// is not a smaller editor — it is an editor with a hole where the work
    /// goes, and no drag or menu entry should be able to produce one.
    pub fn take(&mut self, panel: Panel) -> bool {
        let Some((place, index)) = self.location(panel) else {
            return false;
        };
        if place == Place::MAIN && self.group_mut(Place::MAIN).panels.len() == 1 {
            return false;
        }
        let group = self.group_mut(place);
        group.panels.remove(index);
        group.active = group.active.min(group.panels.len().saturating_sub(1));
        true
    }

    /// Whether moving a panel somewhere would be allowed.
    ///
    /// Asked before the highlight is drawn as well as before the move is made,
    /// so a drag that will be refused is never shown as one that will be
    /// obeyed. A gesture that lights up and then does nothing is worse than one
    /// that never lights up.
    pub fn can_place(&self, panel: Panel, place: Place) -> bool {
        if place == Place::MAIN {
            return true;
        }
        match self.location(panel) {
            Some((Place::MAIN, _)) => self
                .group(Place::MAIN)
                .is_some_and(|group| group.panels.len() > 1),
            _ => true,
        }
    }

    /// Puts a panel somewhere at a given tab position, taking it from wherever
    /// it was.
    ///
    /// Moving rather than copying: a panel is one region of the window, and two
    /// of them would be two views of one piece of state fighting over the same
    /// scroll position and the same selection.
    pub fn place(&mut self, panel: Panel, place: Place, index: usize) {
        if !self.can_place(panel, place) {
            return;
        }
        if let Some((origin, at)) = self.location(panel) {
            let group = self.group_mut(origin);
            group.panels.remove(at);
            group.active = group.active.min(group.panels.len().saturating_sub(1));
        }
        let group = self.group_mut(place);
        let index = index.min(group.panels.len());
        group.panels.insert(index, panel);
        group.active = index;
        group.collapsed = false;
    }

    /// Closes a panel, or reopens it where it last was.
    pub fn toggle(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.take(panel);
        } else {
            self.place(panel, Place::MAIN, usize::MAX);
        }
    }

    /// Repairs an arrangement read back from settings.
    ///
    /// Settings are a file on disk, and a file on disk can say anything: a
    /// panel listed twice, a panel placed nowhere, an empty centre, a size of
    /// minus one. None of those should stop the editor opening, and none of
    /// them should be trusted either, so every one is corrected here rather
    /// than guarded against at each of the places that reads this.
    pub fn repair(&mut self) {
        let mut seen = Vec::new();
        for (place, group) in &mut self.places {
            group.panels.retain(|panel| {
                let first = !seen.contains(panel);
                if first {
                    seen.push(*panel);
                }
                first
            });
            // The centre is the remainder and has no size of its own, so there
            // is nothing here to correct — and clamping it to a minimum it
            // never uses would make a repaired settings file differ from a
            // fresh one.
            if *place != Place::MAIN {
                group.size = repaired(group.size, *place, Place::default_size);
            }
            if place.is_overlay() {
                group.height = repaired(group.height, *place, Place::default_height);
            } else {
                group.collapsed = false;
            }
            group.active = group.active.min(group.panels.len().saturating_sub(1));
        }
        // A panel placed nowhere is unreachable, and the View menu is how it is
        // reopened — so an unplaced panel is a closed panel, which is allowed.
        // An empty centre is not.
        if self.group(Place::MAIN).is_none() {
            let rescued = Panel::ALL
                .into_iter()
                .find(|panel| panel.is_viewport() && !seen.contains(panel))
                .unwrap_or(Panel::Scene);
            self.take(rescued);
            self.group_mut(Place::MAIN).panels.push(rescued);
            self.group_mut(Place::MAIN).active = 0;
        }
    }
}

/// One stored measurement, corrected: a number that is not a number falls back
/// to the default, and one that is merely too small is raised to the minimum.
fn repaired(stored: f32, place: Place, fallback: fn(Place) -> f32) -> f32 {
    if stored.is_finite() {
        stored.max(place.min_size())
    } else {
        fallback(place)
    }
}

#[cfg(test)]
mod tests;
