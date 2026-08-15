//! What the demo scene's colours must look like once they reach an image.
//!
//! Two images render this scene: the headless capture, and the editor's viewport
//! inside a screenshot of the whole window. They have to agree, and the only way
//! to know they do is to look at the pixels — a colour-space mistake compiles,
//! lints, validates, renders, and passes every other test while producing the
//! wrong picture.
//!
//! So the expectation lives here once and both images are held to it, rather
//! than each capture carrying its own idea of what orange is.

use std::collections::BTreeMap;

/// Colours the demo scene authors that must survive the round trip to an image.
pub const AUTHORED_COLORS: [(&str, [u8; 3]); 2] = [
    ("checkerboard orange", [240, 114, 43]),
    ("checkerboard navy", [18, 34, 55]),
];

/// Per-channel slack, generous enough for texture filtering, a software
/// rasteriser, and window compositing, but far tighter than a colour-space
/// mistake, which moves channels by 40 to 70.
pub const CHANNEL_TOLERANCE: i32 = 16;

/// Each colour must cover at least this many pixels per thousand.
///
/// Low enough that a scene occupying part of a window still passes, high enough
/// that a stray antialiased edge cannot stand in for a whole surface.
pub const MINIMUM_SHARE_PER_THOUSAND: usize = 5;

fn is_near(pixel: &[u8], expected: [u8; 3]) -> bool {
    pixel
        .iter()
        .zip(expected)
        .take(3)
        .all(|(actual, expected)| {
            (i32::from(*actual) - i32::from(expected)).abs() <= CHANNEL_TOLERANCE
        })
}

/// Reports the most common colours in the image, to make a mismatch diagnosable.
fn dominant_colors(pixels: &[u8]) -> Vec<([u8; 3], usize)> {
    let mut counts: BTreeMap<[u8; 3], usize> = BTreeMap::new();
    for pixel in pixels.chunks_exact(4) {
        // Quantise so near-identical shades group together.
        let key = [pixel[0] & !7, pixel[1] & !7, pixel[2] & !7];
        *counts.entry(key).or_default() += 1;
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.truncate(5);
    ranked
}

/// Fails when an image is not the colour the scene authored.
///
/// `pixels` is tightly packed RGBA8.
pub fn verify_authored_colors(pixels: &[u8]) -> Result<(), String> {
    let total = pixels.len() / 4;
    for (name, expected) in AUTHORED_COLORS {
        let found = pixels
            .chunks_exact(4)
            .filter(|pixel| is_near(pixel, expected))
            .count();
        if found * 1000 < total * MINIMUM_SHARE_PER_THOUSAND {
            let dominant = dominant_colors(pixels);
            return Err(format!(
                "expected {name} {expected:?} to cover at least \
                 {MINIMUM_SHARE_PER_THOUSAND} pixels per thousand, but only {found} of {total} \
                 pixels are within {CHANNEL_TOLERANCE} per channel.\n\
                 The most common colours were {dominant:?}.\n\
                 A whole-image shift like this usually means a colour target is \
                 not sRGB, or that a texture is sampled through a view whose \
                 colour space disagrees with whoever reads it."
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(color: [u8; 3], count: usize) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(count * 4);
        for _ in 0..count {
            pixels.extend_from_slice(&[color[0], color[1], color[2], 255]);
        }
        pixels
    }

    fn scene_of(orange: [u8; 3], navy: [u8; 3]) -> Vec<u8> {
        let mut pixels = filled(orange, 500);
        pixels.extend(filled(navy, 500));
        pixels
    }

    #[test]
    fn an_image_holding_the_authored_colours_passes() {
        assert_eq!(
            verify_authored_colors(&scene_of([240, 114, 43], [18, 34, 55])),
            Ok(())
        );
    }

    /// The exact failure this check exists for: every channel decoded one time
    /// too many, which is what an sRGB texture sampled as sRGB by a reader that
    /// decodes again produces.
    #[test]
    fn a_missing_srgb_encode_is_caught() {
        let error = verify_authored_colors(&scene_of([221, 43, 6], [4, 6, 11]))
            .expect_err("a doubly decoded image is not the authored colour");
        assert!(error.contains("checkerboard orange"), "{error}");
    }

    #[test]
    fn a_colour_present_only_as_a_thin_edge_does_not_count() {
        // 1 pixel in 1000 is below the required share, so an antialiased sliver
        // cannot stand in for a surface that should be there.
        let mut pixels = filled([240, 114, 43], 1);
        pixels.extend(filled([18, 34, 55], 999));
        assert!(verify_authored_colors(&pixels).is_err());
    }

    #[test]
    fn small_differences_within_tolerance_still_pass() {
        let nudged = i32::from(u8::try_from(CHANNEL_TOLERANCE).expect("tolerance fits a byte"));
        let orange = [
            u8::try_from(240 - nudged).expect("stays in range"),
            u8::try_from(114 + nudged).expect("stays in range"),
            u8::try_from(43 + nudged).expect("stays in range"),
        ];
        assert_eq!(
            verify_authored_colors(&scene_of(orange, [18, 34, 55])),
            Ok(())
        );
    }
}
