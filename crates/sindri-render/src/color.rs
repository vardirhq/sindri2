use thiserror::Error;

/// The colour format every Sindri colour target uses.
///
/// Colour handling is a round trip that only closes if both halves agree.
/// Source pixels are authored in sRGB, so [`crate::Texture2D`] uploads them in
/// this format and sampling decodes them to linear. Shaders then work in
/// linear, so a colour target must re-encode on write — which only an sRGB
/// format does.
///
/// A target that skips the encode stores linear values as though they were
/// sRGB, which crushes everything dark and saturated: an orange of
/// `(240, 114, 43)` lands as `(224, 44, 5)` and a navy of `(18, 34, 55)` as
/// `(1, 3, 9)`. The image still renders, so nothing fails — it is simply the
/// wrong colour. Every offscreen and in-editor target shares this constant so
/// the two halves cannot drift apart.
///
/// Swapchains are the exception: their format is negotiated with the surface,
/// so [`crate::require_srgb_target`] checks the negotiated one instead.
pub const COLOR_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Rejects a colour target that would skip the sRGB encode.
///
/// Hosts that negotiate their format with a surface call this rather than
/// assuming they were given an sRGB one.
pub fn require_srgb_target(format: wgpu::TextureFormat) -> Result<(), ColorSpaceError> {
    if format.is_srgb() {
        Ok(())
    } else {
        Err(ColorSpaceError::NonSrgbTarget(format))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ColorSpaceError {
    #[error(
        "colour target format {0:?} is not sRGB, so linear shader output would be stored without \
         being encoded and every colour would render too dark"
    )]
    NonSrgbTarget(wgpu::TextureFormat),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_target_format_encodes_srgb() {
        assert!(
            COLOR_TARGET_FORMAT.is_srgb(),
            "a linear colour target silently darkens every rendered colour"
        );
    }

    #[test]
    fn non_srgb_targets_are_rejected_by_name() {
        assert_eq!(require_srgb_target(COLOR_TARGET_FORMAT), Ok(()));
        assert_eq!(
            require_srgb_target(wgpu::TextureFormat::Rgba8Unorm),
            Err(ColorSpaceError::NonSrgbTarget(
                wgpu::TextureFormat::Rgba8Unorm
            ))
        );
        assert_eq!(
            require_srgb_target(wgpu::TextureFormat::Bgra8Unorm),
            Err(ColorSpaceError::NonSrgbTarget(
                wgpu::TextureFormat::Bgra8Unorm
            ))
        );
    }
}
