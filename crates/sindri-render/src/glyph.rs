//! Rasterised glyphs, packed into one texture so text can be drawn as quads.
//!
//! Text used to be its own render pass in its own coordinate system: glyphon
//! drew strings straight to the target in viewport pixels, which meant text was
//! the one drawn thing that did not go through a camera. It could not rotate
//! with the surface it sat on, could not be hidden by anything in front of it,
//! and was re-rasterised every time a viewport zoomed.
//!
//! This is the other half of the fix. A glyph is rasterised once at a generous
//! fixed em size and kept in an atlas; a string becomes one textured quad per
//! glyph, in the same units and through the same camera as every other quad in
//! the frame. Text is geometry after this, so a canvas turned in the scene turns
//! its labels with it.
//!
//! A fixed raster size and scaled quads is the same trade a bitmap font makes:
//! sharp at the size it was baked and progressively softer away from it. It is
//! deliberately not signed distance fields yet — those change what is *in* the
//! atlas and what samples it, not where the quads are, so they can land later
//! without moving this boundary.

// An atlas is small integers and the fractions of it they name, so texel counts
// become f32 all over this module. Every value fits a mantissa several times
// over — the atlas caps out at 4096 — and spelling out the conversion at each
// site would bury the packing arithmetic it exists to serve.
#![allow(clippy::cast_precision_loss)]

use std::collections::HashMap;

use glyphon::{CacheKey, FontSystem, SwashCache, SwashContent};

use crate::{Texture2D, TextureError, TextureFilter, UvRect};

/// The em size every glyph is rasterised at.
///
/// One size for the whole atlas, so a label animating its size does not fill
/// the atlas with near-identical copies of the same letters. Sixty-four is
/// enough detail for text drawn a good deal larger than it was baked — a title
/// filling a phone screen — while keeping a full Latin set inside one small
/// texture.
pub const RASTER_EM: f32 = 64.0;

/// A transparent border around each glyph, in texels.
///
/// Without it a smooth sampler reaching just past a glyph's edge picks up its
/// neighbour, which shows up as flecks of the wrong letter along the edges of
/// scaled-up text.
const PADDING: u32 = 1;

/// The atlas starts here and doubles as it fills.
const INITIAL_SIZE: u32 = 256;

/// Past this an atlas has stopped being the right structure, and a frame that
/// asked for more glyphs than this drops the ones that did not fit rather than
/// growing without limit.
const MAX_SIZE: u32 = 4096;

/// Where one glyph sits in the atlas, and where it sits relative to its pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphSlot {
    /// The part of the atlas this glyph occupies.
    pub uv: UvRect,
    /// Where the image sits relative to the pen, in raster pixels, exactly as
    /// swash reports it: `left` of the pen position, and `top` *above* the
    /// baseline. The two run in opposite directions, which is why they are kept
    /// as given rather than folded into one offset here.
    pub offset: [f32; 2],
    /// The image's size in raster pixels.
    pub size: [f32; 2],
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
        }
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
        let width = image.placement.width;
        let height = image.placement.height;
        if width == 0 || height == 0 {
            return None;
        }
        let texels = match image.content {
            // One coverage byte per texel. White with that coverage as alpha,
            // so the sprite tint is what colours the glyph — which is why one
            // atlas serves every colour of text in the frame.
            SwashContent::Mask => image
                .data
                .iter()
                .flat_map(|&coverage| [255, 255, 255, coverage])
                .collect::<Vec<u8>>(),
            SwashContent::Color => image.data.clone(),
            // Subpixel coverage is three channels of a mask meant for a known
            // pixel grid, which a scaled quad does not have. Never requested,
            // and refused rather than mangled if a face returns one anyway.
            SwashContent::SubpixelMask => return None,
        };
        let [x, y] = self.allocate(width, height)?;
        self.blit(&texels, x, y, width, height);
        self.dirty = true;
        let size = self.size as f32;
        Some(GlyphSlot {
            uv: UvRect::new(
                x as f32 / size,
                y as f32 / size,
                width as f32 / size,
                height as f32 / size,
            )
            .ok()?,
            offset: [image.placement.left as f32, image.placement.top as f32],
            size: [width as f32, height as f32],
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
    /// The glyphs come back on the next frame that asks for them, and the swash
    /// cache still holds their images, so the cost is a blit each rather than a
    /// second rasterisation. The frame that grew the atlas is the one frame that
    /// can be missing glyphs — and it is the frame that was already missing the
    /// ones that would not fit.
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
    use super::{GlyphAtlas, INITIAL_SIZE};

    #[test]
    fn a_new_atlas_is_empty_and_starts_small() {
        let atlas = GlyphAtlas::new();
        assert_eq!(atlas.size(), INITIAL_SIZE);
        assert_eq!(atlas.glyph_count(), 0);
    }

    /// Shelf packing has to move down a row when one fills, and grow when the
    /// rows run out. Exercised through `allocate` alone because rasterising
    /// needs a font and a GPU and this arithmetic needs neither.
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
}
