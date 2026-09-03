//! Text as geometry: strings shaped from project-owned font bytes and turned
//! into one distance-field quad per glyph.
//!
//! Cosmic-text owns shaping, fallback and font matching. Sindri owns the stable
//! boundary around it: a frame carries logical font references and immutable
//! text instances, while a host binds bytes fetched through `sindri-assets`. No
//! system font is part of that contract, which keeps native and browser output
//! on the same face.
//!
//! Everything here is measured in the units the pass's camera draws — overlay
//! units for a HUD, world units for a canvas placed in the scene — and never in
//! viewport pixels. That is the difference from the screen-space text pass this
//! replaced: a string is flat geometry on the surface it was authored against,
//! so it pans, zooms and turns with that surface, and a viewport resizing does
//! not re-rasterise a single glyph.
//!
//! An instance is its words, its face and its size, and then a stack of options
//! that all default to nothing: a box to wrap and fit inside, spacing, weight
//! and case, an outline, a shadow, and how much of the string is shown. Each is
//! in `options.rs` with the reason it exists.

mod options;

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use glyphon::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
    cosmic_text::Align,
};
use thiserror::Error;

pub use options::{
    LineAlign, TextAlign, TextCase, TextFit, TextShadow, TextStroke, TextStyle, TextWrap,
};

use crate::{
    FIELD_PER_RASTER_PIXEL, GlyphAtlas, GlyphInstance, RASTER_EM, Texture2D, TextureError,
};

/// One laid-out string, in the units of the camera its pass is drawn through.
///
/// Built from the six things every string needs and then adjusted: every option
/// past [`TextInstance::new`] has a default that draws exactly what the string
/// drew before it existed.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInstance {
    text: String,
    font: String,
    position: [f32; 2],
    font_size: f32,
    line_height: f32,
    color: [f32; 4],
    /// Which end of the string `position` names, across and then down. With a
    /// box, which end of the *box* — and where the block sits inside it.
    align: [TextAlign; 2],
    /// The rect the text is laid out in, or zero on an axis with no bound.
    bounds: [f32; 2],
    wrap: TextWrap,
    line_align: LineAlign,
    letter_spacing: f32,
    style: TextStyle,
    case: TextCase,
    /// How many glyphs are drawn, for a reveal. `None` draws them all.
    visible: Option<usize>,
    outline: TextStroke,
    shadow: TextShadow,
    fit: Option<TextFit>,
}

/// Where a string of this size actually starts, given the point it was told to
/// sit at and which end of it that point names.
///
/// Its own function because it is the whole of what can be got wrong: a string
/// is laid out from its top-left, and every element around it is placed by its
/// centre.
#[must_use]
pub fn aligned_origin(instance: &TextInstance, size: [f32; 2]) -> [f32; 2] {
    let [across, down] = instance.align();
    [
        instance.position()[0] + across.start_after(size[0]),
        instance.position()[1] + down.top_above(size[1]),
    ]
}

/// How big a shaped string turned out, in raster pixels, across and down.
///
/// The width is the longest line rather than the box it was shaped in, which
/// would answer the same for every string.
fn laid_out(buffer: &Buffer, line_height: f32) -> [f32; 2] {
    let mut width = 0.0_f32;
    let mut lines = 0.0_f32;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        lines += 1.0;
    }
    [width, lines * line_height]
}

impl TextInstance {
    /// A string with every option at its default.
    pub fn new(
        text: impl Into<String>,
        font: impl Into<String>,
        position: [f32; 2],
        font_size: f32,
        line_height: f32,
        color: [f32; 4],
        align: [TextAlign; 2],
    ) -> Result<Self, TextError> {
        if !position.into_iter().all(f32::is_finite) {
            return Err(TextError::NonFinitePosition(position));
        }
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(TextError::InvalidFontSize(font_size));
        }
        if !line_height.is_finite() || line_height <= 0.0 {
            return Err(TextError::InvalidLineHeight(line_height));
        }
        if !color.into_iter().all(f32::is_finite) {
            return Err(TextError::NonFiniteColor(color));
        }
        Ok(Self {
            text: text.into(),
            font: font.into(),
            position,
            font_size,
            line_height,
            color,
            align,
            bounds: [0.0, 0.0],
            wrap: TextWrap::None,
            line_align: LineAlign::Follow,
            letter_spacing: 0.0,
            style: TextStyle::default(),
            case: TextCase::AsWritten,
            visible: None,
            outline: TextStroke::default(),
            shadow: TextShadow::default(),
            fit: None,
        })
    }

    /// The rect the text is laid out in, and how it breaks inside it.
    ///
    /// Zero on an axis means unbounded there, which is what a HUD reading wants:
    /// a score has no box and should not be wrapped or shrunk to fit one. A
    /// width is what wrapping needs; a height is what auto-sizing needs.
    #[must_use]
    pub fn in_box(mut self, bounds: [f32; 2], wrap: TextWrap) -> Self {
        self.bounds = bounds.map(|side| if side.is_finite() { side.max(0.0) } else { 0.0 });
        self.wrap = wrap;
        self
    }

    /// Where each line sits across the box, when that differs from where the
    /// whole block sits.
    #[must_use]
    pub const fn with_line_align(mut self, line_align: LineAlign) -> Self {
        self.line_align = line_align;
        self
    }

    /// Extra space after every glyph, in the units the text is placed in.
    #[must_use]
    pub fn with_letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = if spacing.is_finite() { spacing } else { 0.0 };
        self
    }

    /// The weight and slant asked of the face.
    #[must_use]
    pub const fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Whether the words are shaped as written, upper, or lower.
    #[must_use]
    pub const fn with_case(mut self, case: TextCase) -> Self {
        self.case = case;
        self
    }

    /// Draws only the first `glyphs` of the string, for a reveal.
    ///
    /// Counted in glyphs rather than characters, because glyphs are what a quad
    /// is made of and a ligature has no half. For a typewriter effect over Latin
    /// text the two are the same thing.
    #[must_use]
    pub const fn with_visible_glyphs(mut self, glyphs: usize) -> Self {
        self.visible = Some(glyphs);
        self
    }

    /// A stroke around the glyphs.
    #[must_use]
    pub const fn with_outline(mut self, outline: TextStroke) -> Self {
        self.outline = outline;
        self
    }

    /// A copy drawn behind the text, offset and softened.
    #[must_use]
    pub const fn with_shadow(mut self, shadow: TextShadow) -> Self {
        self.shadow = shadow;
        self
    }

    /// Picks the largest size in `fit` that keeps the text inside its box.
    ///
    /// Needs a box to fit in: with no bounds there is nothing to be too big for,
    /// and the authored size stands.
    #[must_use]
    pub const fn fitted(mut self, fit: TextFit) -> Self {
        self.fit = Some(fit);
        self
    }

    /// Which end of the string its position names, across and then down.
    #[must_use]
    pub const fn align(&self) -> [TextAlign; 2] {
        self.align
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn font(&self) -> &str {
        &self.font
    }

    pub const fn position(&self) -> [f32; 2] {
        self.position
    }

    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    pub const fn line_height(&self) -> f32 {
        self.line_height
    }

    pub const fn color(&self) -> [f32; 4] {
        self.color
    }

    /// The rect the text is laid out in, zero on an unbounded axis.
    #[must_use]
    pub const fn bounds(&self) -> [f32; 2] {
        self.bounds
    }

    /// Whether this string is laid out within a width.
    ///
    /// Both halves are needed: a width with no wrap mode does not bound the
    /// layout, and a wrap mode with no width has nothing to wrap to.
    pub(super) fn wraps(&self) -> bool {
        self.bounds[0] > 0.0 && self.wrap != TextWrap::None
    }

    /// How many of the units this string is placed in one raster pixel covers,
    /// at a given font size.
    ///
    /// Glyphs are baked once at [`RASTER_EM`] and every quad is a scaled copy,
    /// so this single number is what turns a shaped layout into geometry.
    fn units_per_raster_pixel(size: f32) -> f32 {
        size / RASTER_EM
    }
}

/// One glyph batch, ready to draw through the pass's own camera.
pub struct GlyphQuads<'a> {
    /// The atlas every quad in `instances` samples.
    pub atlas: &'a Texture2D,
    /// Which build of that atlas this is, for a renderer caching bind groups.
    pub generation: u64,
    pub instances: Vec<GlyphInstance>,
}

/// Glyph shaping, rasterisation and the atlas they land in, shared by every
/// viewport.
pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: GlyphAtlas,
    /// Logical asset reference to the family declared inside its bytes.
    fonts: BTreeMap<String, String>,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            atlas: GlyphAtlas::new(),
            fonts: BTreeMap::new(),
        }
    }

    /// Makes validated font bytes available under the scene reference.
    pub fn bind_font(
        &mut self,
        reference: impl Into<String>,
        family: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Option<String> {
        self.font_system.db_mut().load_font_data(bytes);
        self.fonts.insert(reference.into(), family.into())
    }

    pub fn unbind_font(&mut self, reference: &str) -> Option<String> {
        self.fonts.remove(reference)
    }

    pub fn clear_bindings(&mut self) {
        self.fonts.clear();
    }

    pub fn has_font(&self, reference: &str) -> bool {
        self.fonts.contains_key(reference)
    }

    /// How much of its own units one string covers, across and down.
    ///
    /// `None` for an unbound font, which is what [`Self::quads`] does with one
    /// too: a string whose face never arrived is not drawn, so it covers nothing
    /// and there is nothing to hit-test against.
    ///
    /// The width is the widest laid-out line and the height is the lines it
    /// actually used — the block, not the box it was allowed. Independent of any
    /// viewport, because the answer is a size in the scene now rather than a
    /// rectangle on a screen, which is what lets an editor's pick box be built
    /// once and stay right through a zoom.
    ///
    /// This exists because an editor cannot otherwise say where a string is.
    /// Every other drawn thing has a size in the scene; a string's is decided by
    /// glyph layout inside this module, and a guessed box picks the wrong thing
    /// near its edges. So the answer comes from the same shaping the frame is
    /// drawn from.
    pub fn measure(&mut self, instance: &TextInstance) -> Option<[f32; 2]> {
        let family = self.fonts.get(instance.font()).cloned()?;
        let laid = self.lay_out(instance, &family);
        Some(laid.block)
    }

    /// Where a string sits and how much room it takes, in its own units.
    ///
    /// The words' own extent when the string has no box, and the box itself when
    /// it has one: a label given a rect occupies that rect whether or not its
    /// words fill it, which is what an editor draws a handle around and what a
    /// click has to land in.
    ///
    /// Answered as a centre and a size, because that is the shape every other
    /// element in the scene is described by.
    pub fn rect(&mut self, instance: &TextInstance) -> Option<([f32; 2], [f32; 2])> {
        let family = self.fonts.get(instance.font()).cloned()?;
        let laid = self.lay_out(instance, &family);
        let (corner, outer) = outer_corner(instance, laid.block);
        Some((
            [corner[0] + outer[0] * 0.5, corner[1] - outer[1] * 0.5],
            outer,
        ))
    }

    /// One instance laid out, the only place that is decided.
    ///
    /// Shared by drawing and measuring rather than written twice, because two
    /// copies is exactly how a pick box ends up disagreeing with the picture it
    /// is meant to be over.
    fn lay_out(&mut self, instance: &TextInstance, family: &str) -> LaidOut {
        let size = self.fitted_size(instance, family);
        let buffer = self.shape(instance, family, size);
        let scale = TextInstance::units_per_raster_pixel(size);
        let [width, height] = laid_out(&buffer, instance.line_height / scale);
        LaidOut {
            buffer,
            wrapped: instance.wraps(),
            scale,
            block: [width * scale, height * scale],
        }
    }

    /// The font size this string is actually drawn at.
    ///
    /// The authored one unless it was told to fit a box, in which case the
    /// largest size in range whose block fits. A bisection rather than a walk
    /// down from the maximum: a dozen shapes settles a range to within a
    /// thousandth of it, and a walk is unbounded in how many it takes.
    fn fitted_size(&mut self, instance: &TextInstance, family: &str) -> f32 {
        let Some(fit) = instance.fit else {
            return instance.font_size;
        };
        // Nothing to be too big for. A fit with no box would shrink text to its
        // minimum for no reason anyone could see.
        if instance.bounds[0] <= 0.0 && instance.bounds[1] <= 0.0 {
            return instance.font_size;
        }
        let fits = |renderer: &mut Self, size: f32| {
            let buffer = renderer.shape(instance, family, size);
            let scale = TextInstance::units_per_raster_pixel(size);
            // The line height is a share of the authored size, so it has to
            // travel with the trial size or a shrunk block keeps its old leading.
            let leading = instance.line_height / instance.font_size * size;
            let [width, height] = laid_out(&buffer, leading / scale);
            (instance.bounds[0] <= 0.0 || width * scale <= instance.bounds[0])
                && (instance.bounds[1] <= 0.0 || height * scale <= instance.bounds[1])
        };
        if fits(self, fit.max) {
            return fit.max;
        }
        let (mut small, mut large) = (fit.min, fit.max);
        for _ in 0..12 {
            let middle = f32::midpoint(small, large);
            if fits(self, middle) {
                small = middle;
            } else {
                large = middle;
            }
        }
        small
    }

    /// The buffer for one string at one size.
    ///
    /// Always shaped at [`RASTER_EM`], whatever size the instance asked for: the
    /// atlas holds one baked copy of each glyph, and a string shaped at its own
    /// size would key glyphs that are not in it. The requested size becomes the
    /// scale everything else is measured against, so it reaches the layout as a
    /// wrap width and a line height rather than as an em.
    fn shape(&mut self, instance: &TextInstance, family: &str, size: f32) -> Buffer {
        let scale = TextInstance::units_per_raster_pixel(size);
        // The authored leading is a share of the authored size; at a fitted size
        // it keeps the same share.
        let leading = instance.line_height / instance.font_size * size;
        let mut buffer = Buffer::new(
            &mut self.font_system,
            Metrics::new(RASTER_EM, leading / scale),
        );
        // A width to wrap in, in the raster pixels the buffer is laid out in.
        // Without one nothing wraps, whatever mode was chosen — which is right:
        // a HUD reading has no box and breaking it would be worse than
        // overflowing.
        let wrap_width = instance.wraps().then(|| instance.bounds[0] / scale);
        buffer.set_size(wrap_width, None);
        buffer.set_wrap(match instance.wrap {
            TextWrap::None => Wrap::None,
            TextWrap::Word => Wrap::WordOrGlyph,
            TextWrap::Glyph => Wrap::Glyph,
        });

        let mut attrs = Attrs::new().family(Family::Name(family));
        if instance.style.bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if instance.style.italic {
            attrs = attrs.style(Style::Italic);
        }
        if instance.letter_spacing != 0.0 {
            // Cosmic-text adds this to a glyph's advance *in ems*, not in the
            // pixels the buffer is otherwise laid out in — advances are kept
            // normalised and multiplied by the font size later. Handing it a
            // pixel count moves each letter that many ems along, which puts the
            // whole string off the side of the screen with one glyph left
            // visible. As a share of the em it is just the authored distance
            // over the authored size.
            attrs = attrs.letter_spacing(instance.letter_spacing / size);
        }
        buffer.set_text(
            &instance.case.applied(&instance.text),
            &attrs,
            Shaping::Advanced,
            None,
        );

        // Line alignment is per line rather than per buffer in cosmic-text, and
        // it only means anything with a width to align within.
        if let Some(align) = line_alignment(instance)
            && wrap_width.is_some()
        {
            for line in &mut buffer.lines {
                line.set_align(Some(align));
            }
        }
        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer
    }

    /// Shapes one ordered text pass into glyph quads, and the atlas they sample.
    ///
    /// An unbound font skips its string. Asset diagnostics name it separately;
    /// silently choosing a machine font here would make a browser and a desktop
    /// disagree while both appeared to work.
    ///
    /// `None` when nothing is drawn — every string had an unbound font, or none
    /// of them put a mark anywhere.
    pub fn quads(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[TextInstance],
    ) -> Result<Option<GlyphQuads<'_>>, TextError> {
        let resolved: Vec<(&TextInstance, String)> = instances
            .iter()
            .filter_map(|instance| {
                self.fonts
                    .get(instance.font())
                    .cloned()
                    .map(|family| (instance, family))
            })
            .collect();
        if resolved.is_empty() {
            return Ok(None);
        }
        let laid: Vec<LaidOut> = resolved
            .iter()
            .map(|(instance, family)| self.lay_out(instance, family))
            .collect();

        // Fill the atlas before reading it, and let it settle before reading it:
        // growing repacks the atlas, so a rect handed out before a grow stops
        // being where its glyph is. `fill` owns that invariant, which is why the
        // quads below are built in a second pass over the same layout.
        let keys: Vec<_> = laid
            .iter()
            .flat_map(|laid| laid.buffer.layout_runs())
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .map(|glyph| glyph.physical((0.0, 0.0), 1.0).cache_key)
                    .collect::<Vec<_>>()
            })
            .collect();
        self.atlas
            .fill(&mut self.font_system, &mut self.swash_cache, &keys);

        let mut quads = Vec::new();
        for (laid, (instance, _)) in laid.iter().zip(resolved.iter()) {
            self.push_string(&mut quads, instance, laid);
        }
        if quads.is_empty() {
            return Ok(None);
        }
        let (atlas, generation) = self.atlas.texture(device, queue)?;
        Ok(Some(GlyphQuads {
            atlas,
            generation,
            instances: quads,
        }))
    }

    /// One string's quads: its shadow first, then its letters over it.
    fn push_string(
        &mut self,
        quads: &mut Vec<GlyphInstance>,
        instance: &TextInstance,
        laid: &LaidOut,
    ) {
        let origin = layout_origin(instance, laid);
        // An outline and a shadow's softness are authored on the text and read
        // by the shader against the stored field, so this is where they cross
        // over: a raster pixel is worth a fixed slice of the field, and the
        // scale says how many raster pixels a unit of the scene is.
        let field = |units: f32| units / laid.scale * FIELD_PER_RASTER_PIXEL;
        // The authored width is how far the stroke reaches *out* from the edge,
        // which is what the shader's threshold measures, so it crosses over
        // whole rather than halved.
        let outline_width = field(instance.outline.width);

        if instance.shadow.is_drawn() {
            self.push_glyphs(
                quads,
                instance,
                laid,
                |glyph| {
                    glyph
                        .with_softness(field(instance.shadow.softness))
                        // The shadow is the letter's shape in one flat colour, so
                        // its outline is drawn in the same colour rather than left
                        // as a rim of the face's.
                        .with_outline(outline_width, instance.shadow.color)
                },
                [
                    origin[0] + instance.shadow.offset[0],
                    origin[1] + instance.shadow.offset[1],
                ],
                instance.shadow.color,
            );
        }

        let outline = instance.outline;
        self.push_glyphs(
            quads,
            instance,
            laid,
            move |glyph| {
                if outline.is_drawn() {
                    glyph.with_outline(outline_width, outline.color)
                } else {
                    glyph
                }
            },
            origin,
            instance.color,
        );
    }

    /// Every drawn glyph of one string, placed from `origin` and coloured
    /// `face`, with `adjust` applied to each.
    fn push_glyphs(
        &mut self,
        quads: &mut Vec<GlyphInstance>,
        instance: &TextInstance,
        laid: &LaidOut,
        adjust: impl Fn(GlyphInstance) -> GlyphInstance,
        origin: [f32; 2],
        face: [f32; 4],
    ) {
        let scale = laid.scale;
        let mut drawn = 0_usize;
        for run in laid.buffer.layout_runs() {
            for glyph in run.glyphs {
                if instance.visible.is_some_and(|limit| drawn >= limit) {
                    return;
                }
                drawn += 1;
                let physical = glyph.physical((0.0, 0.0), 1.0);
                // Pen offsets within a shaped line, so a few hundred at most and
                // exact in an f32 many times over.
                #[allow(clippy::cast_precision_loss)]
                let (pen_x, pen_y) = (physical.x as f32, physical.y as f32);
                // Read, never bake. Baking here could grow the atlas, and
                // growing repacks it — every quad already pushed would then be
                // drawing whatever landed at its coordinates afterwards. The
                // atlas was filled before this pass began; a glyph missing now
                // is one it had no room for, and is skipped.
                let Some(slot) = self.atlas.placed(physical.cache_key) else {
                    continue;
                };
                // Raster pixels from the string's top-left corner, down and to
                // the right, which is the space cosmic-text lays out in.
                let left = pen_x + slot.offset[0];
                let top = run.line_y.round() + pen_y - slot.offset[1];
                let size = [slot.size[0] * scale, slot.size[1] * scale];
                let centre = [
                    origin[0] + (left + slot.size[0] * 0.5) * scale,
                    origin[1] - (top + slot.size[1] * 0.5) * scale,
                ];
                let model = Mat4::from_translation(Vec3::new(centre[0], centre[1], 0.0))
                    * Mat4::from_scale(Vec3::new(size[0], size[1], 1.0));
                let quad = GlyphInstance::new(model, slot.uv, face);
                let quad = if slot.colored {
                    quad.colored()
                } else {
                    adjust(quad)
                };
                quads.push(quad);
            }
        }
    }

    /// How many distinct glyphs are baked into the atlas.
    ///
    /// A host's only window onto how much text has cost it.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.atlas.glyph_count()
    }
}

/// One string, shaped and measured.
struct LaidOut {
    buffer: Buffer,
    /// Whether the buffer was given a width to lay out within.
    ///
    /// It decides where the glyph coordinates are measured from, which is the
    /// one thing that cannot be worked out by looking at them: given a width,
    /// cosmic-text positions each line inside it — a centred line already
    /// carries half the slack — so the quads are placed from the box's edge.
    /// Without one, lines start at zero and the quads are placed from the
    /// block's own edge.
    wrapped: bool,
    /// Units of the scene per raster pixel, at the size it was drawn.
    scale: f32,
    /// What the words cover, in the units the text is placed in.
    block: [f32; 2],
}

/// The top-left corner of the room a string is given, and how big that room is.
///
/// Without a box the room is the words themselves. With one it is the box, and
/// the box is what the anchor places — which is what makes "top left, box 0.8
/// wide" mean the same thing for a paragraph as for the panel behind it.
///
/// One function because the frame, the editor's handle and the pick box all
/// have to agree about it, and three copies only have to disagree once.
fn outer_corner(instance: &TextInstance, block: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let [across, down] = instance.align();
    let bounds = instance.bounds();
    let outer = [
        if bounds[0] > 0.0 { bounds[0] } else { block[0] },
        if bounds[1] > 0.0 { bounds[1] } else { block[1] },
    ];
    (
        [
            instance.position()[0] + across.start_after(outer[0]),
            instance.position()[1] + down.top_above(outer[1]),
        ],
        outer,
    )
}

#[cfg(test)]
impl LaidOut {
    /// A layout with only the parts placement reads, for testing placement
    /// without a font.
    fn measured(block: [f32; 2], wrapped: bool) -> Self {
        Self {
            buffer: Buffer::new_empty(Metrics::new(1.0, 1.0)),
            wrapped,
            scale: 1.0,
            block,
        }
    }
}

/// The corner the glyph coordinates of one laid-out string are measured from.
///
/// Down is always the same: the room the string was given, and then the block
/// hung inside it by the vertical alignment. Cosmic-text stacks lines from the
/// top of the buffer and does nothing else vertically.
///
/// Across depends on whether the buffer had a width. Without one, lines start at
/// zero and the block is placed by its own measured width — the way an
/// unbounded label has always been placed. With one, cosmic-text has already
/// positioned each line inside that width, so a centred line carries half the
/// slack in its own glyph coordinates; placing the block by its width as well
/// would add that slack a second time and push the words off to one side. This
/// is exactly what it looked like: a centred hint sitting a little right of
/// centre, by half of what its box had spare.
fn layout_origin(instance: &TextInstance, laid: &LaidOut) -> [f32; 2] {
    let [across, down] = instance.align();
    let (corner, outer) = outer_corner(instance, laid.block);
    [
        if laid.wrapped {
            corner[0]
        } else {
            corner[0] + across.inset(outer[0], laid.block[0])
        },
        corner[1] - down.inset(outer[1], laid.block[1]),
    ]
}

/// Which alignment cosmic-text should give each line, if any.
///
/// `None` leaves the line laid out from its start, which is what an unwrapped
/// string wants and what cosmic-text does with no alignment set.
fn line_alignment(instance: &TextInstance) -> Option<Align> {
    match instance.line_align {
        LineAlign::Follow => match instance.align()[0] {
            TextAlign::Start => None,
            TextAlign::Middle => Some(Align::Center),
            TextAlign::End => Some(Align::Right),
        },
        LineAlign::Left => Some(Align::Left),
        LineAlign::Center => Some(Align::Center),
        LineAlign::Right => Some(Align::Right),
        LineAlign::Justify => Some(Align::Justified),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum TextError {
    #[error("text position must be finite, got {0:?}")]
    NonFinitePosition([f32; 2]),
    #[error("font size must be finite and greater than zero, got {0}")]
    InvalidFontSize(f32),
    #[error("line height must be finite and greater than zero, got {0}")]
    InvalidLineHeight(f32),
    #[error("text color must be finite, got {0:?}")]
    NonFiniteColor([f32; 4]),
    #[error("could not build the glyph atlas: {0}")]
    Atlas(#[from] TextureError),
}

#[cfg(test)]
mod tests {
    use super::{LaidOut, TextAlign, TextError, TextInstance, TextWrap, layout_origin};

    fn instance(align: [TextAlign; 2]) -> TextInstance {
        TextInstance::new("hi", "font.ttf", [0.0, 0.0], 0.1, 0.12, [1.0; 4], align)
            .expect("a finite instance")
    }

    /// Where a block of this size would be placed, without shaping anything.
    ///
    /// `wrapped` is the one thing the placement cannot read off the block, so
    /// the tests below say it: it is whether cosmic-text was given a width and
    /// has therefore already positioned the lines inside it.
    fn placed(instance: &TextInstance, block: [f32; 2], wrapped: bool) -> [f32; 2] {
        layout_origin(instance, &LaidOut::measured(block, wrapped))
    }

    #[test]
    fn text_instances_refuse_values_a_gpu_cannot_place() {
        assert!(matches!(
            TextInstance::new(
                "hello",
                "font.ttf",
                [0.0, 0.0],
                0.0,
                0.1,
                [1.0; 4],
                [TextAlign::Start; 2]
            ),
            Err(TextError::InvalidFontSize(0.0))
        ));
        assert!(matches!(
            TextInstance::new(
                "hello",
                "font.ttf",
                [f32::NAN, 0.0],
                0.06,
                0.07,
                [1.0; 4],
                [TextAlign::Start; 2]
            ),
            Err(TextError::NonFinitePosition(_))
        ));
    }

    /// With no box the block is placed by its own size, exactly as it was
    /// before boxes existed.
    #[test]
    fn a_string_with_no_box_is_placed_by_its_own_size() {
        let centred = instance([TextAlign::Middle; 2]);
        let origin = placed(&centred, [0.4, 0.1], false);
        assert!((origin[0] + 0.2).abs() < 1.0e-6, "{origin:?}");
        assert!((origin[1] - 0.05).abs() < 1.0e-6, "{origin:?}");
    }

    /// With a box, the box takes the place the block used to and the block sits
    /// inside it — so a paragraph and the panel behind it agree about where
    /// "top left" is.
    #[test]
    fn a_box_is_placed_by_the_anchor_and_the_words_sit_inside_it() {
        let boxed = instance([TextAlign::Start; 2]).in_box([1.0, 0.5], TextWrap::Word);
        // Anchored at its start, the box's top-left is the point itself, and a
        // block aligned to the start sits in that corner.
        let origin = placed(&boxed, [0.4, 0.1], false);
        assert!(origin[0].abs() < 1.0e-6, "{origin:?}");
        assert!(origin[1].abs() < 1.0e-6, "{origin:?}");

        let centred = instance([TextAlign::Middle; 2]).in_box([1.0, 0.5], TextWrap::Word);
        let origin = placed(&centred, [0.4, 0.1], false);
        // The box spans -0.5..0.5, and a 0.4-wide block centred in it starts at
        // -0.2 — the same place it would without a box, which is the point:
        // a box changes what wraps, not where a centred label sits.
        assert!((origin[0] + 0.2).abs() < 1.0e-6, "{origin:?}");
        assert!((origin[1] - 0.05).abs() < 1.0e-6, "{origin:?}");

        // Anchored to the top of its box, a short block hangs from the top edge
        // rather than floating in the middle.
        let top =
            instance([TextAlign::Middle, TextAlign::Start]).in_box([1.0, 0.5], TextWrap::Word);
        let origin = placed(&top, [0.4, 0.1], false);
        assert!(origin[1].abs() < 1.0e-6, "{origin:?}");
    }

    /// An unbounded axis is not a box, however the other one is set.
    #[test]
    fn a_box_with_no_width_still_measures_the_words_across() {
        let tall = instance([TextAlign::Start; 2]).in_box([0.0, 0.5], TextWrap::Word);
        assert!(tall.bounds()[0].abs() < f32::EPSILON);
        assert!((tall.bounds()[1] - 0.5).abs() < f32::EPSILON);
        let origin = placed(&tall, [0.4, 0.1], false);
        assert!(origin[0].abs() < 1.0e-6, "{origin:?}");
    }

    /// A wrapped line is already positioned inside its box, so the block must
    /// not be positioned again.
    ///
    /// The bug this is here for put a centred hint half its box's slack to the
    /// right of centre — visible, but only just, and only against something
    /// else that was centred properly.
    #[test]
    fn a_wrapped_block_is_placed_from_its_box_rather_than_from_its_words() {
        let centred = instance([TextAlign::Middle; 2]).in_box([1.0, 0.5], TextWrap::Word);
        // The box spans -0.5..0.5, so wrapped glyph coordinates are measured
        // from -0.5 whatever the words came out as.
        let origin = placed(&centred, [0.4, 0.1], true);
        assert!((origin[0] + 0.5).abs() < 1.0e-6, "{origin:?}");
        // Unwrapped, the same block is centred by its own width instead.
        let origin = placed(&centred, [0.4, 0.1], false);
        assert!((origin[0] + 0.2).abs() < 1.0e-6, "{origin:?}");
    }

    /// A bound that is not a number is no bound, rather than a box of NaN that
    /// every later comparison quietly fails.
    #[test]
    fn a_bound_that_is_not_a_number_is_no_bound() {
        let odd = instance([TextAlign::Start; 2]).in_box([f32::NAN, -3.0], TextWrap::Word);
        assert!(odd.bounds().iter().all(|side| side.abs() < f32::EPSILON));
    }
}
