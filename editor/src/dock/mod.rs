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
//! [`Slot`] and how big each slot is; the frame loop reads it rather than
//! knowing it. The two arrangements that used to be the only choices are now
//! two [presets](Workspace::preset) — starting points someone can drag away
//! from, rather than the only two shapes the editor has.

mod drag;

pub use drag::{Drag, DropTarget, edge_slot, tab_index};

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

/// Where in the window a group of panels sits.
///
/// Seven rather than four because the arrangements people actually want are
/// asymmetric: two columns down one side and one down the other is the common
/// shape of an editor, and a model with a single slot per edge cannot say it.
/// Slots claim space from the outside in, so `FarLeft` is against the window
/// and `Left` sits inside it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    FarLeft,
    Left,
    FarRight,
    Right,
    /// Under the centre, between the side columns.
    Bottom,
    /// The centre itself: whatever is left when every other slot has taken its
    /// share. Cannot be empty — see [`Workspace::take`].
    Main,
    /// A second group under [`Slot::Main`], for showing two views at once.
    MainBottom,
}

impl Slot {
    /// Every slot in the order it claims space, outermost first.
    ///
    /// The frame loop walks this, so the order here *is* the arrangement:
    /// a slot claiming space earlier ends up further out, and `Main` is last
    /// because it is defined as the remainder.
    pub const ALL: [Self; 7] = [
        Self::FarLeft,
        Self::FarRight,
        Self::Left,
        Self::Right,
        Self::Bottom,
        Self::MainBottom,
        Self::Main,
    ];

    /// Which way the slot is measured: `true` when its size is a width.
    pub const fn is_column(self) -> bool {
        matches!(
            self,
            Self::FarLeft | Self::Left | Self::FarRight | Self::Right
        )
    }

    /// How big a slot is before anyone has resized it.
    const fn default_size(self) -> f32 {
        match self {
            Self::FarLeft | Self::Left => 260.0,
            Self::FarRight => 340.0,
            Self::Right | Self::Bottom | Self::MainBottom => 300.0,
            // Never used: `Main` is the remainder and has no size of its own.
            Self::Main => 0.0,
        }
    }

    /// The smallest a slot may be dragged to.
    ///
    /// Small enough to be a genuine choice rather than the editor overruling
    /// one. The old panels had minimums near their defaults, which meant the
    /// resize handle had a few pixels of travel and reads as broken.
    pub const fn min_size(self) -> f32 {
        if self.is_column() { 150.0 } else { 90.0 }
    }

    /// A stable id for the egui panel, so a slot keeps its size across frames.
    pub const fn id(self) -> &'static str {
        match self {
            Self::FarLeft => "dock-far-left",
            Self::Left => "dock-left",
            Self::FarRight => "dock-far-right",
            Self::Right => "dock-right",
            Self::Bottom => "dock-bottom",
            Self::Main => "dock-main",
            Self::MainBottom => "dock-main-bottom",
        }
    }
}

/// One slot's contents: the panels in it, and which of them is showing.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Group {
    /// The panels in this slot, in tab order.
    pub panels: Vec<Panel>,
    /// Which tab is selected, as an index into `panels`.
    ///
    /// An index rather than a `Panel` so that a group holding the same panel
    /// twice — which cannot happen, but which a hand-edited settings file could
    /// ask for — still resolves to one tab. Clamped on read rather than trusted.
    pub active: usize,
    /// How wide or tall the slot is, in points.
    pub size: f32,
}

impl Group {
    fn new(slot: Slot, panels: &[Panel]) -> Self {
        Self {
            panels: panels.to_vec(),
            active: 0,
            size: slot.default_size(),
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
    /// One entry per slot. A slot with no entry is empty and is not drawn.
    slots: Vec<(Slot, Group)>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::preset(Preset::Studio)
    }
}

/// An arrangement to start from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    /// Hierarchy left, both views centre, project and inspector right.
    ///
    /// The default, and the descendant of the old "2 by 3": the scene and what
    /// the player would see at the same time, which is the comparison an editor
    /// exists to make. The console is a tab beside the scene rather than a
    /// third row in the project column, because what the console says is about
    /// the thing in the viewport next to it.
    #[default]
    Studio,
    /// One view at a time, with the project browser along the bottom.
    ///
    /// The whole width for the viewport, which suits a laptop screen or working
    /// on one view without the other competing for attention.
    Wide,
}

impl Preset {
    pub const ALL: [Self; 2] = [Self::Studio, Self::Wide];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::Wide => "Wide",
        }
    }
}

impl Workspace {
    /// One of the arrangements the editor ships with.
    pub fn preset(preset: Preset) -> Self {
        let slots: &[(Slot, &[Panel])] = match preset {
            Preset::Studio => &[
                (Slot::Left, &[Panel::Hierarchy]),
                (Slot::Main, &[Panel::Scene, Panel::Console]),
                (Slot::MainBottom, &[Panel::Game]),
                (Slot::Right, &[Panel::Project, Panel::History]),
                (Slot::FarRight, &[Panel::Inspector]),
            ],
            Preset::Wide => &[
                (Slot::Left, &[Panel::Hierarchy]),
                (Slot::Main, &[Panel::Scene, Panel::Game]),
                (
                    Slot::Bottom,
                    &[Panel::Project, Panel::Console, Panel::History],
                ),
                (Slot::FarRight, &[Panel::Inspector]),
            ],
        };
        Self {
            slots: slots
                .iter()
                .map(|(slot, panels)| (*slot, Group::new(*slot, panels)))
                .collect(),
        }
    }

    /// The group in a slot, if the slot holds anything.
    pub fn group(&self, slot: Slot) -> Option<&Group> {
        self.slots
            .iter()
            .find(|(candidate, group)| *candidate == slot && !group.is_empty())
            .map(|(_, group)| group)
    }

    fn group_mut(&mut self, slot: Slot) -> &mut Group {
        if let Some(index) = self
            .slots
            .iter()
            .position(|(candidate, _)| *candidate == slot)
        {
            return &mut self.slots[index].1;
        }
        self.slots.push((slot, Group::new(slot, &[])));
        let last = self.slots.len() - 1;
        &mut self.slots[last].1
    }

    /// Selects a tab within a slot.
    pub fn select(&mut self, slot: Slot, index: usize) {
        self.group_mut(slot).active = index;
    }

    /// Records a slot's size after the user has dragged its edge.
    pub fn resize(&mut self, slot: Slot, size: f32) {
        self.group_mut(slot).size = size;
    }

    /// Where a panel currently lives, if it is placed at all.
    pub fn location(&self, panel: Panel) -> Option<(Slot, usize)> {
        self.slots.iter().find_map(|(slot, group)| {
            group
                .panels
                .iter()
                .position(|candidate| *candidate == panel)
                .map(|index| (*slot, index))
        })
    }

    pub fn is_open(&self, panel: Panel) -> bool {
        self.location(panel).is_some()
    }

    /// Takes a panel out of wherever it is.
    ///
    /// `Main` is refused when it would empty it. The centre is what every other
    /// slot is measured against, so a workspace with nothing in the middle is
    /// not a smaller editor — it is an editor with a hole where the work goes,
    /// and no drag or menu entry should be able to produce one.
    pub fn take(&mut self, panel: Panel) -> bool {
        let Some((slot, index)) = self.location(panel) else {
            return false;
        };
        if slot == Slot::Main && self.group_mut(Slot::Main).panels.len() == 1 {
            return false;
        }
        let group = self.group_mut(slot);
        group.panels.remove(index);
        group.active = group.active.min(group.panels.len().saturating_sub(1));
        true
    }

    /// Whether moving a panel to a slot would be allowed.
    ///
    /// Asked before the highlight is drawn as well as before the move is made,
    /// so a drag that will be refused is never shown as one that will be
    /// obeyed. A gesture that lights up and then does nothing is worse than one
    /// that never lights up.
    pub fn can_place(&self, panel: Panel, slot: Slot) -> bool {
        if slot == Slot::Main {
            return true;
        }
        match self.location(panel) {
            Some((Slot::Main, _)) => self
                .group(Slot::Main)
                .is_some_and(|group| group.panels.len() > 1),
            _ => true,
        }
    }

    /// Puts a panel into a slot at a given tab position, taking it from
    /// wherever it was.
    ///
    /// Moving rather than copying: a panel is one region of the window, and two
    /// of them would be two views of one piece of state fighting over the same
    /// scroll position and the same selection.
    pub fn place(&mut self, panel: Panel, slot: Slot, index: usize) {
        if !self.can_place(panel, slot) {
            return;
        }
        let from = self.location(panel);
        if let Some((origin, at)) = from {
            let group = self.group_mut(origin);
            group.panels.remove(at);
            group.active = group.active.min(group.panels.len().saturating_sub(1));
        }
        let group = self.group_mut(slot);
        let index = index.min(group.panels.len());
        group.panels.insert(index, panel);
        group.active = index;
    }

    /// Closes a panel, or reopens it where it last was.
    pub fn toggle(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.take(panel);
        } else {
            self.place(panel, Slot::Main, usize::MAX);
        }
    }

    /// Repairs an arrangement read back from settings.
    ///
    /// Settings are a file on disk, and a file on disk can say anything: a
    /// panel listed twice, a panel in no slot at all, an empty centre, a size
    /// of minus one. None of those should stop the editor opening, and none of
    /// them should be trusted either, so every one is corrected here rather
    /// than guarded against at each of the places that reads this.
    pub fn repair(&mut self) {
        let mut seen = Vec::new();
        for (slot, group) in &mut self.slots {
            group.panels.retain(|panel| {
                let first = !seen.contains(panel);
                if first {
                    seen.push(*panel);
                }
                first
            });
            // `Main` is the remainder and has no size of its own, so there is
            // nothing here to correct — and clamping it to a minimum it never
            // uses would make a repaired settings file differ from a fresh one.
            if *slot != Slot::Main {
                group.size = if group.size.is_finite() {
                    group.size.max(slot.min_size())
                } else {
                    slot.default_size()
                };
            }
            group.active = group.active.min(group.panels.len().saturating_sub(1));
        }
        // A panel in no slot is unreachable, and the View menu is how it is
        // reopened — so an unplaced panel is a closed panel, which is allowed.
        // An empty centre is not.
        if self.group(Slot::Main).is_none() {
            let rescued = Panel::ALL
                .into_iter()
                .find(|panel| panel.is_viewport() && !seen.contains(panel))
                .unwrap_or(Panel::Scene);
            self.take(rescued);
            self.group_mut(Slot::Main).panels.push(rescued);
            self.group_mut(Slot::Main).active = 0;
        }
    }
}

#[cfg(test)]
mod tests;
