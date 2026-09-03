//! Turning a glyph's coverage into a signed distance field.
//!
//! A coverage mask says how much of each texel the glyph covers, which is only
//! meaningful at the size it was rasterised. A distance field says how far each
//! texel is from the glyph's edge, which is meaningful at every size — so one
//! bake draws a caption and a title, and a shader can find the edge exactly
//! rather than blurring towards it. It is also what makes an outline and a soft
//! shadow a threshold rather than a second bake.
//!
//! The transform is exact rather than approximate: Felzenszwalb and
//! Huttenlocher's algorithm gives true squared Euclidean distances in two
//! linear passes per axis. An approximate one — chamfer, dead reckoning — is
//! cheaper and shows up as faint lumps along a long diagonal stroke, which is
//! the one thing a distance field exists to avoid.

/// How far from a glyph's edge the field still says something, in raster
/// pixels.
///
/// The outline and shadow budget: a stroke wider than this runs out of field to
/// stand on. Eight at a 64-pixel em is an eighth of the em, which is heavier
/// than any outline a UI wants, and it costs sixteen texels on each side of
/// every glyph in the atlas.
pub(super) const SPREAD: u32 = 8;

/// The value the glyph's edge is stored as.
///
/// Halfway, so the field has the same room either side of the edge and a shader
/// can test against one constant. Everything else here is measured from it.
pub const EDGE: f32 = 0.5;

/// A glyph's coverage, padded and turned into a field.
///
/// `coverage` is `width * height` bytes as swash rasterised them. The result is
/// `(width + 2 * SPREAD) * (height + 2 * SPREAD)` bytes, each the distance from
/// that texel to the glyph's edge: [`EDGE`] on the edge itself, rising towards
/// one inside the glyph and falling towards zero outside it.
#[allow(clippy::cast_precision_loss)]
pub(super) fn signed_distance_field(coverage: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pad = SPREAD as usize;
    let (w, h) = (width as usize + pad * 2, height as usize + pad * 2);

    // The mask, padded. Everything outside the glyph's own box is empty, which
    // is what the padding is for: a stroke touching the edge of its box still
    // has field around it to be outlined in.
    let mut alpha = vec![0.0_f32; w * h];
    for row in 0..height as usize {
        for column in 0..width as usize {
            alpha[(row + pad) * w + column + pad] =
                f32::from(coverage[row * width as usize + column]) / 255.0;
        }
    }

    let inside = |value: f32| value >= 0.5;
    // Two transforms: how far each outside texel is from the glyph, and how far
    // each inside texel is from the outside. Subtracting one from the other is
    // what makes the field signed.
    let outward = distance_to(&alpha, w, h, inside);
    let inward = distance_to(&alpha, w, h, |value| !inside(value));

    let mut field = vec![0_u8; w * h];
    for (index, texel) in field.iter_mut().enumerate() {
        let value = alpha[index];
        // A texel the glyph half-covers *is* the edge, and its coverage says
        // where in the texel the edge falls. Reading it back is what keeps the
        // field sub-texel accurate; a purely thresholded transform quantises
        // every edge to a whole texel and shows it as a faint stipple along
        // shallow curves.
        let signed = if value > 0.0 && value < 1.0 {
            value - 0.5
        } else if inside(value) {
            inward[index]
        } else {
            -outward[index]
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            *texel =
                ((EDGE + signed / (2.0 * SPREAD as f32)).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    field
}

/// The distance from every texel to the nearest one `seed` accepts.
///
/// Squared distances are carried through both passes and rooted once at the
/// end, which is the whole reason the transform is exact and linear.
#[allow(clippy::cast_precision_loss)]
fn distance_to(alpha: &[f32], w: usize, h: usize, seed: impl Fn(f32) -> bool) -> Vec<f32> {
    // `f32::MAX` would overflow the parabola arithmetic below; a distance no
    // texel in this glyph can reach is enough and stays finite.
    let unreachable = ((w + h) * (w + h)) as f32;
    let mut squared: Vec<f32> = alpha
        .iter()
        .map(|&value| if seed(value) { 0.0 } else { unreachable })
        .collect();

    let mut column = vec![0.0_f32; h];
    for x in 0..w {
        for (y, value) in column.iter_mut().enumerate() {
            *value = squared[y * w + x];
        }
        let transformed = lower_envelope(&column);
        for (y, value) in transformed.iter().enumerate() {
            squared[y * w + x] = *value;
        }
    }
    for y in 0..h {
        let transformed = lower_envelope(&squared[y * w..y * w + w]);
        squared[y * w..y * w + w].copy_from_slice(&transformed);
    }
    squared.iter().map(|value| value.sqrt()).collect()
}

/// One dimension of the distance transform: the lower envelope of the parabolas
/// rooted at each sample.
///
/// Felzenszwalb and Huttenlocher, *Distance Transforms of Sampled Functions*.
/// The envelope is built left to right by intersecting each new parabola with
/// the ones already in it, then read off in a second sweep — linear in the
/// number of samples, and exact.
#[allow(clippy::cast_precision_loss)]
fn lower_envelope(row: &[f32]) -> Vec<f32> {
    let n = row.len();
    let mut out = vec![0.0_f32; n];
    if n == 0 {
        return out;
    }
    // Which sample roots each piece of the envelope, and where the pieces meet.
    let mut roots = vec![0_usize; n];
    let mut boundaries = vec![0.0_f32; n + 1];
    let mut piece = 0_usize;
    boundaries[0] = f32::NEG_INFINITY;
    boundaries[1] = f32::INFINITY;

    for q in 1..n {
        let qf = q as f32;
        let mut meeting;
        loop {
            let pf = roots[piece] as f32;
            meeting =
                (qf.mul_add(qf, row[q]) - pf.mul_add(pf, row[roots[piece]])) / (2.0 * (qf - pf));
            if meeting > boundaries[piece] || piece == 0 {
                break;
            }
            // This parabola hides the last one entirely; drop it and retry.
            piece -= 1;
        }
        piece += 1;
        roots[piece] = q;
        boundaries[piece] = meeting;
        boundaries[piece + 1] = f32::INFINITY;
    }

    let mut piece = 0_usize;
    for (q, value) in out.iter_mut().enumerate() {
        let qf = q as f32;
        while boundaries[piece + 1] < qf {
            piece += 1;
        }
        let pf = roots[piece] as f32;
        *value = (qf - pf).mul_add(qf - pf, row[roots[piece]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{EDGE, SPREAD, signed_distance_field};

    /// A solid block: the middle is well inside, the padding is well outside,
    /// and the two are separated by the edge rather than by a step.
    #[test]
    fn a_solid_shape_reads_as_inside_and_its_padding_as_outside() {
        let side = 16_u32;
        let field = signed_distance_field(&vec![255; (side * side) as usize], side, side);
        let width = (side + SPREAD * 2) as usize;
        let at = |x: u32, y: u32| f32::from(field[y as usize * width + x as usize]) / 255.0;

        let middle = at(SPREAD + side / 2, SPREAD + side / 2);
        assert!(middle > EDGE, "the middle of a block is inside: {middle}");
        let corner = at(0, 0);
        assert!(corner < EDGE, "the padding is outside: {corner}");
        // Far enough out the field saturates rather than wrapping round.
        assert!(corner.abs() < 0.05, "{corner}");
    }

    /// The field falls off with distance rather than switching, which is the
    /// whole of what makes it usable at a size it was not baked at.
    #[test]
    fn the_field_falls_away_with_distance_from_the_edge() {
        let side = 24_u32;
        let field = signed_distance_field(&vec![255; (side * side) as usize], side, side);
        let width = (side + SPREAD * 2) as usize;
        let at = |x: u32, y: u32| f32::from(field[y as usize * width + x as usize]) / 255.0;

        let row = SPREAD + side / 2;
        let near = at(SPREAD - 1, row);
        let far = at(SPREAD - 6, row);
        assert!(near < EDGE && far < near, "{near} then {far}");
        let just_in = at(SPREAD + 1, row);
        let well_in = at(SPREAD + 6, row);
        assert!(
            just_in > EDGE && well_in > just_in,
            "{just_in} then {well_in}"
        );
    }
}
