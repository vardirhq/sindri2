//! `sindri.ui.shape`: a drawn shape on the overlay, evaluated rather than
//! painted.
//!
//! The authoring end of [`sindri_render::Shape`]. A sprite covers what an artist
//! drew; this covers what the scene already knows — a panel's border, a cooldown
//! ring, a dashed marker, the grid behind a menu — none of which should be a
//! texture, because each is a number and a texture would have to be re-exported
//! every time the number changed.
//!
//! Overlay only, like [`super::UiImageComponent`], and for the same reason: it
//! is anchored to a corner of the viewport rather than placed in the world. A
//! world-space shape is a separate component and is not here yet.
//!
//! As with the text options, the stored spellings live in `sindri-scene` rather
//! than in `sindri-render`, which takes no serde.

use serde::Deserialize;
use sindri_render::{Shape, ShapeBlend, ShapeInstance};

use crate::UiAnchor;

use super::transparent;

/// What the shape is, before its modifiers.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiShapeKind {
    /// A rectangle filling the element, optionally with rounded corners. The
    /// default because it is the panel and the button, which is most of a UI.
    #[default]
    Rect,
    /// An ellipse inscribed in the element — a circle in a square one.
    Ellipse,
    /// A regular polygon with a vertex at the top.
    Polygon,
    /// A lattice of lines. Takes a stroke and ignores a fill, having no inside.
    Grid,
}

impl UiShapeKind {
    /// Every kind, in the order a chooser should offer them.
    pub const ALL: [Self; 4] = [Self::Rect, Self::Ellipse, Self::Polygon, Self::Grid];

    /// The name this kind is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Polygon => "polygon",
            Self::Grid => "grid",
        }
    }
}

/// How a shape meets what is already behind it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiShapeBlend {
    /// Drawn over what is behind it. Paint.
    #[default]
    Over,
    /// Added to it. Light — which is what a glow, a muzzle flash and a
    /// shockwave are, and what makes two of them overlapping brighter than
    /// either.
    Add,
}

impl UiShapeBlend {
    /// Every mode, in the order a chooser should offer them.
    pub const ALL: [Self; 2] = [Self::Over, Self::Add];

    /// The name this mode is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Over => "over",
            Self::Add => "add",
        }
    }

    /// Which renderer blend this is.
    #[must_use]
    pub(crate) const fn to_render(self) -> ShapeBlend {
        match self {
            Self::Over => ShapeBlend::Over,
            Self::Add => ShapeBlend::Add,
        }
    }
}

/// A shape drawn on the overlay.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiShapeComponent {
    #[serde(default)]
    pub kind: UiShapeKind,
    /// How many sides a polygon has, or how many cells a grid is across.
    ///
    /// One number for both because a shape has only one, and a `sides` a grid
    /// ignored beside a `cells` a polygon ignored is two fields that are each
    /// wrong most of the time.
    #[serde(default = "default_count")]
    pub count: f32,
    /// The colour inside. Transparent by default, because most of what this
    /// draws is an outline and a filled panel is the exception.
    #[serde(default = "transparent")]
    pub fill: [f32; 4],
    #[serde(default = "transparent")]
    pub stroke: [f32; 4],
    /// How wide the outline is, as a fraction of the element's own size.
    ///
    /// A fraction rather than overlay units, so a shape scaled up is the same
    /// drawing larger rather than the same drawing with thinner lines.
    #[serde(default)]
    pub stroke_width: f32,
    /// How much a rectangle's corners are rounded, as a fraction of its size.
    #[serde(default)]
    pub corner_radius: f32,
    /// How many evenly spaced dashes the outline is broken into. Zero is solid.
    #[serde(default)]
    pub dashes: f32,
    /// How much of its share of the way round each dash covers.
    #[serde(default = "half")]
    pub dash_duty: f32,
    /// Where the outline starts, as a fraction of the way round from the top.
    #[serde(default)]
    pub sweep_start: f32,
    /// How much of the way round the outline is drawn. A full turn is one.
    ///
    /// This is a cooldown, a charge meter and a progress ring: a script writes
    /// the fraction and the shape draws that much of itself.
    #[serde(default = "full")]
    pub sweep_turns: f32,
    #[serde(default)]
    pub blend: UiShapeBlend,
    #[serde(default)]
    pub anchor: UiAnchor,
    /// The explicit override on draw order within the overlay.
    #[serde(default)]
    pub layer: i32,
}

const fn default_count() -> f32 {
    6.0
}

const fn half() -> f32 {
    0.5
}

const fn full() -> f32 {
    1.0
}

impl UiShapeComponent {
    /// The drawable this stored shape is.
    ///
    /// The single place a stored shape becomes a drawn one, so the frame, the
    /// editor's pick box and anything else that has to agree about what is on
    /// screen all read the same answer.
    #[must_use]
    pub fn instance(&self, model: glam::Mat4) -> ShapeInstance {
        let kind = match self.kind {
            UiShapeKind::Rect => Shape::Rect,
            UiShapeKind::Ellipse => Shape::Ellipse,
            UiShapeKind::Polygon => Shape::Polygon { sides: self.count },
            UiShapeKind::Grid => Shape::Grid { cells: self.count },
        };
        ShapeInstance::filled(model, kind, self.fill)
            .with_stroke(self.stroke_width.max(0.0), self.stroke)
            .with_corner_radius(self.corner_radius)
            .dashed(self.dashes.max(0.0), self.dash_duty)
            .swept(self.sweep_start, self.sweep_turns)
    }

    /// Which renderer blend this shape's batch uses.
    #[must_use]
    pub(crate) const fn blend(&self) -> ShapeBlend {
        self.blend.to_render()
    }
}

impl sindri_core::SceneComponent for UiShapeComponent {
    const TYPE_NAME: &'static str = "sindri.ui.shape";
}
