//! Rasterised glyphs, packed into one texture so text can be drawn as quads.
//!
//! Text used to be its own render pass in its own coordinate system: glyphon
//! drew strings straight to the target in viewport pixels, which meant text was
//! the one drawn thing that did not go through a camera. It could not rotate
//! with the surface it sat on, could not be hidden by anything in front of it,
//! and was re-rasterised every time a viewport zoomed.
//!
//! This is the other half of the fix. A glyph is baked once and kept in an
//! atlas; a string becomes one textured quad per glyph, in the same units and
//! through the same camera as every other quad in the frame. Text is geometry
//! after this, so a canvas turned in the scene turns its labels with it.
//!
//! What is *in* the atlas is a signed distance field rather than a coverage
//! mask, which is what makes one bake serve every size: a mask is only correct
//! at the size it was rasterised, while a field says where the edge is and lets
//! the shader find it exactly. It is also what makes an outline and a soft
//! shadow a pair of thresholds rather than a second bake. Colour glyphs — an
//! emoji face — have no edge to measure and are kept as the bitmap they are;
//! the slot says which kind it is.

// An atlas is small integers and the fractions of it they name, so texel counts
// become f32 all over this module. Every value fits a mantissa several times
// over — the atlas caps out at 4096 — and spelling out the conversion at each
// site would bury the packing arithmetic it exists to serve.
#![allow(clippy::cast_precision_loss)]

mod field;
mod instance;
mod render;

use std::collections::HashMap;

use glyphon::{CacheKey, FontSystem, SwashCache, SwashContent};

pub use field::EDGE;
use field::{SPREAD, signed_distance_field};
pub use instance::GlyphInstance;
pub use render::{GlyphDrawError, GlyphRenderer};

use crate::{Texture2D, TextureError, TextureFilter, UvRect};

/// The em size every glyph is rasterised at.
///
/// One size for the whole atlas, so a label animating its size does not fill
/// the atlas with near-identical copies of the same letters. Sixty-four is
/// enough detail for the field to describe a letterform faithfully, while
/// keeping a full Latin set inside one small texture.
pub const RASTER_EM: f32 = 64.0;

/// What one raster pixel is worth in the field's stored units.
///
/// The field runs from nothing to one across twice its spread, so this is what
/// turns an outline or a shadow measured on the glyph into the number the
/// shader compares a sample against. Exported because the size an author asks
/// for is in overlay units, and only the caller knows what one of those is worth
/// in raster pixels.
pub const FIELD_PER_RASTER_PIXEL: f32 = 1.0 / (2.0 * SPREAD as f32);

/// A gap between packed glyphs, in texels.
///
/// Without it a smooth sampler reaching just past a glyph's rect picks up its
/// neighbour, which shows up as flecks of the wrong letter along the edges of
/// scaled-up text. One is enough because the field's own spread already
/// surrounds every glyph with texels of its own.
const PADDING: u32 = 1;

/// The atlas starts here and doubles as it fills.
///
/// Larger than a coverage mask needed: every glyph now carries the field's
/// spread on all four sides, so a capital at a 64-pixel em packs as roughly
/// sixty texels square rather than forty.
const INITIAL_SIZE: u32 = 512;

/// Past this an atlas has stopped being the right structure, and a frame that
/// asked for more glyphs than this drops the ones that did not fit rather than
/// growing without limit.
const MAX_SIZE: u32 = 4096;

/// How many times [`GlyphAtlas::fill`] will refill a growing atlas.
///
/// The atlas doubles from its smallest to its largest in three steps, so a frame
/// whose text needs the whole thing settles well inside this. Bounded rather
/// than looping until stable because "until stable" is a promise the atlas
/// cannot keep: past [`MAX_SIZE`] it stops growing and starts refusing glyphs,
/// and a frame asking for more than it can hold would spin.
const FILL_ATTEMPTS: usize = 6;

/// Where one glyph sits in the atlas, and where it sits relative to its pen.
///
/// The rect is the glyph *plus* the field's spread on every side, and so is the
/// size: a quad drawn to the glyph's own outline would clip off the outer half
/// of its own antialiasing, and all of an outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphSlot {
    /// The part of the atlas this glyph occupies.
    pub uv: UvRect,
    /// Where the drawn rect sits relative to the pen, in raster pixels, in the
    /// directions swash reports them: `left` of the pen position, and `top`
    /// *above* the baseline. The two run in opposite directions, which is why
    /// they are kept as given rather than folded into one offset here.
    pub offset: [f32; 2],
    /// The drawn rect's size in raster pixels.
    pub size: [f32; 2],
    /// Whether the atlas holds this glyph's own colours rather than a field.
    ///
    /// An emoji face rasterises to a picture, which has no edge to measure and
    /// nothing for a tint to say. Kept as a flag on the slot rather than a
    /// second atlas, because a string can mix the two freely.
    pub colored: bool,
}

/// Every glyph the frames drawn so far have needed, in one texture.
pub struct GlyphAtlas {
    pixels: Vec<u8>,
    size: u32,
    /// Shelf packing state: where the next glyph goes and how tall its row is.
    pen: [u32; 2],
    row_height: u32,
    /// `None` for a glyph with no image of its own — a space, or one the face
    /// declined to rasterise. Cached either way so it is asked for once.
    slots: HashMap<CacheKey, Option<GlyphSlot>>,
    /// Set when `pixels` has changed since the texture was last built.
    dirty: bool,
    texture: Option<Texture2D>,
    /// Bumped every time the texture is rebuilt, so a renderer caching a bind
    /// group against this atlas can tell that the texture it named is gone.
    generation: u64,
    /// Bumped every time the atlas grows, which is every time the rects it has
    /// already handed out stop being where their glyphs are.
    epoch: u64,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphAtlas {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: vec![0; (INITIAL_SIZE * INITIAL_SIZE * 4) as usize],
            size: INITIAL_SIZE,
            pen: [PADDING, PADDING],
            row_height: 0,
            slots: HashMap::new(),
            dirty: true,
            texture: None,
            generation: 0,
            epoch: 0,
        }
    }

    /// Places every glyph in `keys`, and says whether the atlas settled.
    ///
    /// The invariant this exists to keep: growing the atlas repacks it, so a
    /// rect handed out before a grow stops being where its glyph is. A caller
    /// placing a frame's worth of glyphs therefore cannot read rects in the same
    /// pass that fills the atlas — *and* cannot trust one fill, because a fill
    /// that itself triggered a grow left behind the glyphs it had already
    /// placed. So it fills again from the grown atlas until nothing moves.
    ///
    /// `false` means it never settled: the text asked for more glyphs than the
    /// atlas can hold even at its largest. The caller draws what it has rather
    /// than spinning, which is the same answer the atlas gives to one glyph it
    /// cannot fit.
    pub fn fill(
        &mut self,
        fonts: &mut FontSystem,
        swash: &mut SwashCache,
        keys: &[CacheKey],
    ) -> bool {
        for _ in 0..FILL_ATTEMPTS {
            let epoch = self.epoch;
            for key in keys.iter().copied() {
                self.slot(fonts, swash, key);
            }
            if self.epoch == epoch {
                return true;
            }
        }
        false
    }

    /// Where this glyph is in the atlas, rasterising it on first sight.
    ///
    /// `None` means the glyph draws nothing: a space has no image, and neither
    /// does a glyph the atlas has no room left for.
    pub fn slot(
        &mut self,
        fonts: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
    ) -> Option<GlyphSlot> {
        if let Some(slot) = self.slots.get(&key) {
            return *slot;
        }
        let slot = self.rasterise(fonts, swash, key);
        self.slots.insert(key, slot);
        slot
    }

    /// Where this glyph is, without baking it if it is not there.
    ///
    /// The reading half of [`Self::slot`], and the one a caller building quads
    /// wants: asking for a glyph that is missing would bake it, which can grow
    /// the atlas, which moves every rect already collected. It is also the only
    /// way to observe whether a fill really placed something — through `slot`,
    /// every glyph resolves, because asking is what puts it there.
    #[must_use]
    pub fn placed(&self, key: CacheKey) -> Option<GlyphSlot> {
        self.slots.get(&key).copied().flatten()
    }

    /// The texture a batch of glyph quads samples, built if the atlas moved,
    /// and which build of it that is.
    ///
    /// Rebuilt whole rather than patched: a new glyph is a rare event once a
    /// scene has been on screen for a frame, and a whole upload cannot leave
    /// the texture disagreeing with the UV rects handed out beside it.
    ///
    /// The build number comes back with the texture rather than being asked for
    /// separately, because the two only agree either side of this call: read
    /// before, it names the build about to be replaced, and a caller keying a
    /// bind group on it would hand the same name to two different textures.
    pub fn texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(&Texture2D, u64), TextureError> {
        if self.dirty || self.texture.is_none() {
            self.texture = Some(Texture2D::from_rgba8_filtered(
                device,
                queue,
                "Sindri glyph atlas",
                self.size,
                self.size,
                &self.pixels,
                TextureFilter::Smooth,
            )?);
            self.dirty = false;
            self.generation += 1;
        }
        Ok((
            self.texture
                .as_ref()
                .expect("the atlas texture was just built"),
            self.generation,
        ))
    }

    /// Which packing the rects handed out so far belong to.
    ///
    /// Changes whenever the atlas grows, because growing repacks it.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// How wide and tall the atlas is, in texels.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
    }

    /// How many distinct glyphs the atlas has been asked for.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.slots.len()
    }

    fn rasterise(
        &mut self,
        fonts: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
    ) -> Option<GlyphSlot> {
        let image = swash.get_image_uncached(fonts, key)?;
        let ink = [image.placement.width, image.placement.height];
        if ink[0] == 0 || ink[1] == 0 {
            return None;
        }
        let (texels, colored) = match image.content {
            // A field, padded by its own spread on every side. White RGB with
            // the distance in alpha: the shader reads the distance and the
            // instance's colour does the rest, which is why one atlas serves
            // every colour and weight of text in the frame.
            SwashContent::Mask => (
                signed_distance_field(&image.data, ink[0], ink[1])
                    .into_iter()
                    .flat_map(|distance| [255, 255, 255, distance])
                    .collect::<Vec<u8>>(),
                false,
            ),
            // An emoji face is a picture. There is no edge to measure, so it is
            // stored as it came and drawn as it is.
            SwashContent::Color => (image.data.clone(), true),
            // Subpixel coverage is three channels of a mask meant for a known
            // pixel grid, which a scaled quad does not have. Never requested,
            // and refused rather than mangled if a face returns one anyway.
            SwashContent::SubpixelMask => return None,
        };
        // A field's rect is the glyph plus its spread; a picture's is itself.
        let bleed = if colored { 0 } else { SPREAD };
        let width = ink[0] + bleed * 2;
        let height = ink[1] + bleed * 2;

        let [x, y] = self.allocate(width, height)?;
        self.blit(&texels, x, y, width, height);
        self.dirty = true;
        let size = self.size as f32;
        let bleed = bleed as f32;
        Some(GlyphSlot {
            uv: UvRect::new(
                x as f32 / size,
                y as f32 / size,
                width as f32 / size,
                height as f32 / size,
            )
            .ok()?,
            offset: [
                image.placement.left as f32 - bleed,
                image.placement.top as f32 + bleed,
            ],
            size: [width as f32, height as f32],
            colored,
        })
    }

    /// Reserves a rectangle, growing the atlas if the shelves are full.
    fn allocate(&mut self, width: u32, height: u32) -> Option<[u32; 2]> {
        loop {
            let step = |v: u32, extra: u32| v.checked_add(extra + PADDING);
            if step(self.pen[0], width)? <= self.size {
                if step(self.pen[1], height)? <= self.size {
                    let at = self.pen;
                    self.pen[0] += width + PADDING;
                    self.row_height = self.row_height.max(height);
                    return Some(at);
                }
            } else if step(self.pen[1], self.row_height)? <= self.size {
                // The row is full but there is height left: start a new shelf
                // and try the same glyph against it.
                self.pen = [PADDING, self.pen[1] + self.row_height + PADDING];
                self.row_height = 0;
                continue;
            }
            self.grow()?;
        }
    }

    /// Doubles the atlas, dropping what was in it.
    ///
    /// Dropped rather than copied because the shelves are laid out against the
    /// old width: copying the pixels would leave every UV rect pointing at half
    /// the texture, and every glyph drawn at half the size it should be.
    ///
    /// The glyphs are asked for again rather than moved, which is why
    /// [`Self::epoch`] exists: a caller part way through placing a frame's
    /// glyphs has to notice and start over, or it draws the letters it collected
    /// before the grow at the coordinates of whatever landed there after it.
    fn grow(&mut self) -> Option<()> {
        let size = self.size.checked_mul(2)?;
        if size > MAX_SIZE {
            return None;
        }
        self.pixels = vec![0; (size * size * 4) as usize];
        self.size = size;
        self.pen = [PADDING, PADDING];
        self.row_height = 0;
        self.dirty = true;
        self.epoch += 1;
        self.slots.clear();
        Some(())
    }

    fn blit(&mut self, texels: &[u8], x: u32, y: u32, width: u32, height: u32) {
        for row in 0..height {
            let source = (row * width * 4) as usize;
            let target = (((y + row) * self.size + x) * 4) as usize;
            let bytes = (width * 4) as usize;
            self.pixels[target..target + bytes].copy_from_slice(&texels[source..source + bytes]);
        }
    }
}

#[cfg(test)]
mod tests {
    use glyphon::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};

    use super::{GlyphAtlas, INITIAL_SIZE, RASTER_EM};

    #[test]
    fn a_new_atlas_is_empty_and_starts_small() {
        let atlas = GlyphAtlas::new();
        assert_eq!(atlas.size(), INITIAL_SIZE);
        assert_eq!(atlas.glyph_count(), 0);
    }

    /// Shelf packing has to move down a row when one fills, and grow when the
    /// rows run out. Exercised through `allocate` alone because rasterising
    /// needs a font and this arithmetic does not.
    #[test]
    fn allocation_fills_rows_then_shelves_then_grows() {
        let mut atlas = GlyphAtlas::new();
        let wide = INITIAL_SIZE / 2;
        let first = atlas.allocate(wide, 16).expect("fits");
        let second = atlas.allocate(wide, 16).expect("starts a new row");
        assert_eq!(first[1], second[1] - 17, "{first:?} {second:?}");

        // Filling the atlas with tall rows forces a grow rather than a refusal.
        for _ in 0..64 {
            atlas.allocate(wide, INITIAL_SIZE / 4);
        }
        assert!(atlas.size() > INITIAL_SIZE);
    }

    /// A lot of distinct glyphs, shaped for real, as cache keys.
    fn many_glyphs(fonts: &mut FontSystem) -> Vec<glyphon::CacheKey> {
        let mut buffer = Buffer::new(fonts, Metrics::new(RASTER_EM, RASTER_EM));
        buffer.set_size(None, None);
        buffer.set_text(
            concat!(
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "!#$%&()*+,-./:;<=>?@[]^_{|}~",
                "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞß",
                "àáâãäåæçèéêëìíîïðñòóôõöøùúûüýþÿ",
                "ĀāĂăĄąĆćĈĉĊċČčĎďĐđĒēĔĕĖėĘęĚěĜĝĞğĠġĢģĤĥĦħĨĩĪīĬĭĮįİıĲĳĴĵĶķ",
                "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩαβγδεζηθικλμνξοπρστυφχψω",
            ),
            &Attrs::new(),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(fonts, false);
        buffer
            .layout_runs()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .map(|glyph| glyph.physical((0.0, 0.0), 1.0).cache_key)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn loaded_fonts() -> FontSystem {
        let mut fonts = FontSystem::new();
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../game/assets/fonts/Inter.ttf"
        ))
        .expect("the companion game's font is in the repository");
        fonts.db_mut().load_font_data(bytes);
        fonts
    }

    /// Every glyph is in the atlas afterwards, however much it grew getting
    /// there.
    ///
    /// This is the bug that reached a rendered picture: growing repacks the
    /// atlas, and a single fill leaves behind whatever it had already placed
    /// before the grow. It showed as one row of a text specimen rendering as a
    /// single stray letter while every row around it was perfect.
    ///
    /// Checked with `placed` rather than `slot`, and that is the whole test:
    /// `slot` bakes what it cannot find, so asking it is what puts the glyph
    /// there and every key resolves however broken the fill was. The first
    /// version of this test did exactly that and passed against the bug.
    #[test]
    fn a_fill_places_every_glyph_however_much_the_atlas_grows() {
        let mut fonts = loaded_fonts();
        let mut swash = SwashCache::new();
        let keys = many_glyphs(&mut fonts);
        assert!(keys.len() > 150, "the specimen is meant to be large");

        let mut atlas = GlyphAtlas::new();
        assert!(
            atlas.fill(&mut fonts, &mut swash, &keys),
            "the atlas should settle for a string this size"
        );
        assert!(
            atlas.size() > INITIAL_SIZE,
            "this many glyphs should have grown it, or the test proves nothing"
        );
        for key in keys {
            assert!(
                atlas.placed(key).is_some(),
                "a glyph placed during the fill is missing from the settled atlas"
            );
        }
    }

    /// One pass is not enough, which is why `fill` loops.
    ///
    /// Stated as a test of its own so the loop cannot be quietly removed as
    /// redundant: a single sweep leaves the atlas holding only what it took
    /// after its last grow.
    #[test]
    fn one_sweep_is_not_enough_to_place_them_all() {
        let mut fonts = loaded_fonts();
        let mut swash = SwashCache::new();
        let keys = many_glyphs(&mut fonts);

        let mut atlas = GlyphAtlas::new();
        for key in keys.iter().copied() {
            atlas.slot(&mut fonts, &mut swash, key);
        }
        let missing = keys
            .iter()
            .filter(|key| atlas.placed(**key).is_none())
            .count();
        assert!(
            missing > 0,
            "a single sweep over {} glyphs should have lost the ones placed \
             before the last grow",
            keys.len()
        );
    }
}
