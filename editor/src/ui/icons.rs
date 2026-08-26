//! The editor's icon vocabulary.
//!
//! An icon is a word, and the same word has to mean the same thing everywhere:
//! a camera in the hierarchy, in the inspector's component header, and on the
//! viewport's camera control is one idea, and it was three separate `ICON_`
//! imports in three files that nothing stopped from drifting apart.
//!
//! Names here say what the icon *means*. Which glyph carries the meaning is one
//! edit in one place.

use egui_material_icons::{MaterialIcon, icons};

// What a thing is.
pub const CAMERA: MaterialIcon = icons::ICON_CAMERA_ALT;
pub const MESH: MaterialIcon = icons::ICON_VIEW_IN_AR;
pub const SPRITE: MaterialIcon = icons::ICON_IMAGE;
pub const UI_ELEMENT: MaterialIcon = icons::ICON_WEB_ASSET;
pub const TEXT: MaterialIcon = icons::ICON_TITLE;
pub const SCRIPT: MaterialIcon = icons::ICON_CODE;
pub const ANIMATION: MaterialIcon = icons::ICON_PLAY_ARROW;
pub const TILEMAP: MaterialIcon = icons::ICON_GRID_VIEW;
pub const GRID: MaterialIcon = icons::ICON_GRID_4X4;
pub const ENTITY: MaterialIcon = icons::ICON_DEPLOYED_CODE;
pub const WORLD: MaterialIcon = icons::ICON_ACCOUNT_TREE;
pub const FOLDER: MaterialIcon = icons::ICON_FOLDER;
pub const SCENE: MaterialIcon = icons::ICON_DESCRIPTION;
pub const AUDIO: MaterialIcon = icons::ICON_PLAY_ARROW;
/// A typeface, which is not a picture.
///
/// It shared the image glyph with a texture, so a project's fonts and its
/// sprite sheets were the same row with different words after them.
pub const FONT: MaterialIcon = icons::ICON_FONT_DOWNLOAD;
/// A file the editor has nothing particular to say about.
///
/// A sheet of paper, not the 3D box the hierarchy draws an entity with: a
/// `.txt` beside a scene is not an object in it, and drawing it as one made the
/// project browser claim something about the file that is not true.
pub const FILE: MaterialIcon = icons::ICON_DRAFT;
pub const TRANSFORM: MaterialIcon = icons::ICON_OPEN_WITH;
pub const LABEL: MaterialIcon = icons::ICON_LABEL;

// What a panel is.
pub const HIERARCHY: MaterialIcon = icons::ICON_ACCOUNT_TREE;
pub const INSPECTOR: MaterialIcon = icons::ICON_TUNE;
pub const PROJECT: MaterialIcon = icons::ICON_FOLDER;
pub const CONSOLE: MaterialIcon = icons::ICON_TERMINAL;

// What a control does.
pub const ADD: MaterialIcon = icons::ICON_ADD;
pub const REMOVE: MaterialIcon = icons::ICON_DELETE;
pub const SEARCH: MaterialIcon = icons::ICON_SEARCH;
pub const REFRESH: MaterialIcon = icons::ICON_REFRESH;
pub const MORE: MaterialIcon = icons::ICON_MORE_HORIZ;
pub const CLOSE: MaterialIcon = icons::ICON_CLOSE;
pub const UNDO: MaterialIcon = icons::ICON_UNDO;
pub const REDO: MaterialIcon = icons::ICON_REDO;
pub const PAUSE: MaterialIcon = icons::ICON_PAUSE;
pub const LIST_VIEW: MaterialIcon = icons::ICON_VIEW_LIST;
pub const GRID_VIEW: MaterialIcon = icons::ICON_GRID_VIEW;
pub const EXPANDED: MaterialIcon = icons::ICON_KEYBOARD_ARROW_DOWN;
pub const COLLAPSED: MaterialIcon = icons::ICON_KEYBOARD_ARROW_RIGHT;

// The viewport's own tools.
pub const SELECT: MaterialIcon = icons::ICON_SELECT;
pub const TRANSLATE: MaterialIcon = icons::ICON_MOVE;
pub const ROTATE: MaterialIcon = icons::ICON_ROTATE_RIGHT;
pub const SCALE: MaterialIcon = icons::ICON_SCALE;
pub const SNAP: MaterialIcon = icons::ICON_GRID_4X4;
pub const RESET_VIEW: MaterialIcon = icons::ICON_CAMERA_ALT;
pub const FOCUS: MaterialIcon = icons::ICON_CENTER_FOCUS_STRONG;

/// The icon a component type is drawn with, wherever it is named.
///
/// One table, so the hierarchy row for a sprite and the inspector header for
/// `sindri.sprite` cannot disagree about what a sprite looks like.
pub fn for_component(type_name: &str) -> MaterialIcon {
    match type_name {
        "sindri.camera" => CAMERA,
        "sindri.mesh" => MESH,
        "sindri.sprite" => SPRITE,
        "sindri.ui.image" => UI_ELEMENT,
        "sindri.ui.text" => TEXT,
        "sindri.script" => SCRIPT,
        "sindri.animation.sprite" => ANIMATION,
        "sindri.tilemap" => TILEMAP,
        "sindri.grid.navigation" | "sindri.grid.occupant" => GRID,
        _ => ENTITY,
    }
}

/// What an entity looks like in a list, from the first thing it carries that
/// says what it is.
///
/// The order is the priority: a UI element that also carries a sprite is a UI
/// element, because "this is on the screen rather than in the world" is the most
/// useful thing a row can say at a glance.
pub fn for_entity(carries: impl Fn(&str) -> bool) -> MaterialIcon {
    for type_name in [
        "sindri.camera",
        "sindri.mesh",
        "sindri.sprite",
        "sindri.ui.image",
        "sindri.ui.text",
    ] {
        if carries(type_name) {
            return for_component(type_name);
        }
    }
    ENTITY
}
