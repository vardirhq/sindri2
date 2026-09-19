//! What the person at the controls is doing.
//!
//! Split from the calls beside it because these answer a different question.
//! Everything in `call.rs` is something a script does to the world or asks of
//! it; everything here is a fact about a keyboard, a pointer, a thumb, or the
//! screen it is all drawn into — read-only, decided by the host before any
//! script ran, and never written by one.
//!
//! A script that could change these would be deciding what the person did.

/// A question a script asks about the keyboard.
#[derive(Clone, Copy, Debug)]
pub(crate) enum InputQuery {
    /// Two opposing keys as -1, 0, or 1.
    Axis,
    Down,
    Pressed,
    Released,
}

impl InputQuery {
    /// How many key names it takes.
    pub(crate) const fn keys(self) -> usize {
        match self {
            Self::Axis => 2,
            Self::Down | Self::Pressed | Self::Released => 1,
        }
    }

    /// Whether it answers with a number rather than a truth.
    pub(crate) const fn is_number(self) -> bool {
        matches!(self, Self::Axis)
    }
}

pub(crate) const INPUT_QUERIES: &[(&str, InputQuery)] = &[
    ("axis", InputQuery::Axis),
    ("is_down", InputQuery::Down),
    ("just_pressed", InputQuery::Pressed),
    ("just_released", InputQuery::Released),
];

/// A question a script asks about where the person is pointing.
///
/// One namespace for the mouse and the finger, because a game that aims at a
/// point should not have to ask which the person is using — and a game written
/// for a mouse then works on a phone without a second code path. What each
/// unified answer means when both are present is in `docs/scripting.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerQuery {
    Down,
    Pressed,
    Released,
}

pub(crate) const POINTER_QUERIES: &[(&str, PointerQuery)] = &[
    ("is_down", PointerQuery::Down),
    ("just_pressed", PointerQuery::Pressed),
    ("just_released", PointerQuery::Released),
];

/// A value a script reads about where the pointer is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerValue {
    X,
    Y,
    /// Whether there is a pointer at all.
    ///
    /// A mouse outside the window and a screen nobody is touching are the same
    /// answer, and a game drawing a cursor or testing a button needs to know
    /// before it reads a position that would otherwise be the last one.
    Inside,
    /// Whether a screen element is taking the pointer this frame.
    ///
    /// The engine does not silently withhold input from gameplay when a menu is
    /// up: which scripts are gameplay is not something a host can know, and a
    /// rule that guesses is one that will guess wrong. So a gameplay script
    /// asks, in one line, and the answer is why a click on a pause button does
    /// not also fire the gun behind it.
    OverUi,
    /// Where the pointer is in the overlay's own units, across and up.
    ///
    /// `x` and `y` are viewport pixels, and how many pixels tall a window is
    /// is not something a scene knows — so a script could tell where the
    /// pointer was on the screen and not what it was pointing at. The overlay
    /// is two tall and centred on the origin, which is a space the scene
    /// authored against, so a game that knows how much world its camera frames
    /// can turn these into world coordinates and the engine does not have to
    /// guess at a camera on a script's behalf.
    OverlayX,
    OverlayY,
}

pub(crate) const POINTER_VALUES: &[(&str, PointerValue)] = &[
    ("x", PointerValue::X),
    ("y", PointerValue::Y),
    ("overlay_x", PointerValue::OverlayX),
    ("overlay_y", PointerValue::OverlayY),
    ("inside", PointerValue::Inside),
    ("over_ui", PointerValue::OverUi),
];

/// A value a script reads about the block under the pointer.
///
/// The cell is split into three numbers for the reason `Grid.position_x` and
/// `position_y` are: Decay has no vector value yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AimValue {
    /// Whether the pointer is on a block at all.
    ///
    /// Asked first, because every other value here reads zero when it is
    /// false and zero is a real cell. A builder script that skipped this would
    /// quietly stack blocks at the origin whenever the pointer left the world.
    Hit,
    /// The block under the pointer -- what removing removes.
    X,
    Y,
    Z,
    /// The empty cell against the side being looked at -- what placing fills.
    ///
    /// The whole of "click a face to attach a block": a cell alone cannot say
    /// which of its six neighbours was meant, so the host answers the
    /// neighbour rather than making every game work out the face itself.
    PlaceX,
    PlaceY,
    PlaceZ,
}

pub(crate) const AIM_VALUES: &[(&str, AimValue)] = &[
    ("hit", AimValue::Hit),
    ("x", AimValue::X),
    ("y", AimValue::Y),
    ("z", AimValue::Z),
    ("place_x", AimValue::PlaceX),
    ("place_y", AimValue::PlaceY),
    ("place_z", AimValue::PlaceZ),
];

/// What a script reads about the camera it is seen through.
///
/// The pan is the world-space distance the camera must move so that the ground
/// under the finger stays under the finger, for whatever drag is happening this
/// frame. Zero when nothing is being dragged, so a camera script can add it
/// every frame without asking first -- adding zero is the right answer when
/// nobody is dragging.
///
/// Three numbers rather than one because Decay has no vector value yet, the
/// same reason `Aim` splits a cell into three.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CameraValue {
    PanX,
    PanY,
    PanZ,
}

pub(crate) const CAMERA_VALUES: &[(&str, CameraValue)] = &[
    ("pan_x", CameraValue::PanX),
    ("pan_y", CameraValue::PanY),
    ("pan_z", CameraValue::PanZ),
];

/// What the person just did, as a gesture rather than as a button.
///
/// Each gesture has a question before its numbers, for the reason `Aim.hit`
/// does: nothing happened is the common case, and every coordinate reads zero
/// when it did, which is a real place on the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GestureValue {
    /// A press that arrived, stayed still and left again.
    Tapped,
    TapX,
    TapY,
    /// A press that stayed still and stayed down. True on the frame it is
    /// recognised, while the finger is still there -- waiting for the release
    /// would make holding indistinguishable from tapping slowly.
    Held,
    HoldX,
    HoldY,
    /// A press that is moving, true on every frame it moves.
    Dragging,
    /// How far it moved this frame, which is what panning a camera wants: the
    /// place it started is rarely the interesting part.
    DragX,
    DragY,
    /// Two fingers moving together or apart, as a ratio since the last frame.
    Pinching,
    Pinch,
}

pub(crate) const GESTURE_VALUES: &[(&str, GestureValue)] = &[
    ("tapped", GestureValue::Tapped),
    ("tap_x", GestureValue::TapX),
    ("tap_y", GestureValue::TapY),
    ("held", GestureValue::Held),
    ("hold_x", GestureValue::HoldX),
    ("hold_y", GestureValue::HoldY),
    ("dragging", GestureValue::Dragging),
    ("drag_x", GestureValue::DragX),
    ("drag_y", GestureValue::DragY),
    ("pinching", GestureValue::Pinching),
    ("pinch", GestureValue::Pinch),
];

/// A value a script reads about the viewport it is running in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ViewportValue {
    Aspect,
}

pub(crate) const VIEWPORT_VALUES: &[(&str, ViewportValue)] = &[("aspect", ViewportValue::Aspect)];

/// What a script reads from the steering stick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StickValue {
    /// How far it is pushed, -1 to 1, in screen axes.
    X,
    Y,
    /// Whether a finger is on it at all.
    ///
    /// Not the same as a zero reading, which is also what a thumb resting
    /// inside the dead zone gives: a game drawing the control wants to show it
    /// while it is held even when it is centred.
    Held,
    /// Where the thumb landed, for a game that draws the ring.
    AnchorX,
    AnchorY,
}

pub(crate) const STICK_VALUES: &[(&str, StickValue)] = &[
    ("x", StickValue::X),
    ("y", StickValue::Y),
    ("held", StickValue::Held),
    ("anchor_x", StickValue::AnchorX),
    ("anchor_y", StickValue::AnchorY),
];

/// A question about the fingers specifically.
///
/// Separate from `Pointer` because it answers something `Pointer` cannot: how
/// many there are, and where the second one is. A game that only needs "where
/// is the person pointing" never touches this.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TouchCall {
    X,
    Y,
}

pub(crate) const TOUCH_CALLS: &[(&str, TouchCall)] = &[("x", TouchCall::X), ("y", TouchCall::Y)];

/// How many fingers are down, as a value rather than a call.
pub(crate) const TOUCH_COUNT: &str = "count";
