//! The same encoded images decode to the same pixels on every target.
//!
//! An asset pipeline whose whole promise is that one scene file works from disk
//! and from static web hosting has to mean it about the bytes too. A texture
//! that decodes natively and not in the browser build — or worse, decodes to
//! something slightly different — turns "the same scene" into a claim nobody
//! checked, and it would be found by someone looking at a picture rather than by
//! a test.
//!
//! So this runs on both. One body, two attributes: `#[test]` natively and
//! `#[wasm_bindgen_test]` under `wasm32-unknown-unknown`, where the runner
//! executes it in Node. The corpus is embedded rather than read from disk,
//! because what is under test is the decoder and not the source.
//!
//! The corpus is deliberately awkward: every colour type PNG defines, sixteen
//! bits per channel, an interlaced encoding, and a JPEG. Those are the paths
//! where a decoder's feature set can differ between builds, and a two-by-two
//! image is small enough to write every expected pixel down.

use sindri_assets::{
    AssetBytes, AssetDecoder, FontAssetDecoder, TextureAsset, TextureAssetDecoder,
};
use sindri_core::{AssetId, AssetLoadErrorKind};

#[cfg(not(target_arch = "wasm32"))]
use core::prelude::v1::test as compatibility_test;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as compatibility_test;

const RGBA8: &[u8] = include_bytes!("fixtures/decode/rgba8.png");
const INTERLACED_RGBA8: &[u8] = include_bytes!("fixtures/decode/interlaced_rgba8.png");
const RGB8: &[u8] = include_bytes!("fixtures/decode/rgb8.png");
const GRAY8: &[u8] = include_bytes!("fixtures/decode/gray8.png");
const PALETTE8: &[u8] = include_bytes!("fixtures/decode/palette8.png");
const RGB16: &[u8] = include_bytes!("fixtures/decode/rgb16.png");
const JPEG: &[u8] = include_bytes!("fixtures/decode/solid.jpg");
const INTER: &[u8] = include_bytes!("../../../game/assets/fonts/Inter.ttf");

/// What every two-by-two fixture that carries colour decodes to, in the order
/// `TextureAsset` packs them: top left, top right, bottom left, bottom right.
const CORNERS: [u8; 16] = [
    255, 0, 0, 255, // red
    0, 255, 0, 255, // green
    0, 0, 255, 255, // blue
    255, 255, 255, 128, // white, half transparent
];

/// The same corners with no alpha channel in the encoding, so the fourth pixel
/// is a colour rather than a transparency.
const OPAQUE_CORNERS: [u8; 16] = [
    255, 0, 0, 255, //
    0, 255, 0, 255, //
    0, 0, 255, 255, //
    16, 32, 48, 255,
];

fn decode(name: &str, bytes: &[u8]) -> TextureAsset {
    let id = AssetId::new(name).expect("fixture names are valid asset IDs");
    TextureAssetDecoder
        .decode(AssetBytes::new(id, bytes.to_vec()))
        .unwrap_or_else(|error| panic!("{name} did not decode: {error}"))
}

fn assert_pixels(name: &str, bytes: &[u8], width: u32, height: u32, expected: &[u8]) {
    let asset = decode(name, bytes);
    assert_eq!((asset.width(), asset.height()), (width, height), "{name}");
    assert_eq!(asset.rgba8(), expected, "{name} decoded to other pixels");
}

/// Truecolour with alpha, which is the shape everything else is widened to.
#[compatibility_test]
fn eight_bit_rgba_decodes_to_its_own_pixels() {
    assert_pixels("rgba8.png", RGBA8, 2, 2, &CORNERS);
}

/// An encoding with no alpha channel has to arrive opaque rather than
/// transparent, which is the difference between a sprite and nothing.
#[compatibility_test]
fn an_encoding_without_alpha_arrives_opaque() {
    assert_pixels("rgb8.png", RGB8, 2, 2, &OPAQUE_CORNERS);
    assert_pixels(
        "gray8.png",
        GRAY8,
        2,
        2,
        &[
            0, 0, 0, 255, //
            85, 85, 85, 255, //
            170, 170, 170, 255, //
            255, 255, 255, 255,
        ],
    );
}

/// A palette is an indirection the decoder has to resolve, not a colour type a
/// GPU understands.
#[compatibility_test]
fn a_palette_is_expanded_to_colours() {
    assert_pixels(
        "palette8.png",
        PALETTE8,
        2,
        2,
        &[
            255, 0, 0, 255, //
            0, 255, 0, 255, //
            0, 0, 255, 255, //
            255, 0, 0, 255,
        ],
    );
}

/// Sixteen bits per channel has to come back as eight, and the fixture's values
/// are chosen so any sane narrowing agrees: each is a byte repeated twice.
#[compatibility_test]
fn sixteen_bits_per_channel_narrow_to_eight() {
    assert_pixels("rgb16.png", RGB16, 2, 2, &OPAQUE_CORNERS);
}

/// Interlacing is a different arrangement of the same image, so it must decode
/// to the same pixels as the progressive encoding of it.
#[compatibility_test]
fn an_interlaced_encoding_is_the_same_image() {
    assert_pixels("interlaced_rgba8.png", INTERLACED_RGBA8, 2, 2, &CORNERS);
    assert_eq!(
        decode("interlaced_rgba8.png", INTERLACED_RGBA8).rgba8(),
        decode("rgba8.png", RGBA8).rgba8(),
        "the two encodings of one image disagreed"
    );
}

/// JPEG is lossy, so this asks whether the colour survived rather than whether
/// the bytes did. A decoder that produced the wrong picture would miss by far
/// more than a rounding step.
#[compatibility_test]
fn a_jpeg_decodes_to_the_colour_it_encodes() {
    let asset = decode("solid.jpg", JPEG);
    assert_eq!((asset.width(), asset.height()), (4, 4));
    for (index, pixel) in asset.rgba8().chunks_exact(4).enumerate() {
        for (channel, expected) in pixel.iter().zip([221, 60, 40, 255]) {
            assert!(
                i32::from(*channel).abs_diff(expected) <= 4,
                "pixel {index} came back as {pixel:?}"
            );
        }
    }
}

/// Bytes that are not an image are an error naming the asset, on every target.
/// A decoder that panicked here would take a browser tab down with it.
#[compatibility_test]
fn bytes_that_are_not_an_image_are_an_error() {
    let id = AssetId::new("broken.png").expect("a valid asset ID");
    let error = TextureAssetDecoder
        .decode(AssetBytes::new(id, b"not an image at all".to_vec()))
        .expect_err("nonsense is not a texture");
    assert_eq!(error.id().as_str(), "broken.png");
    assert!(
        matches!(
            error.kind(),
            AssetLoadErrorKind::InvalidData | AssetLoadErrorKind::UnsupportedFormat
        ),
        "{error}"
    );
}

/// Font validation is CPU-only too, so the same project face can be discovered
/// from bytes before either a native or browser renderer binds it.
#[compatibility_test]
fn a_project_font_declares_the_same_family_on_every_target() {
    let id = AssetId::new("fonts/Inter.ttf").expect("a valid asset ID");
    let font = FontAssetDecoder
        .decode(AssetBytes::new(id, INTER.to_vec()))
        .expect("the shipped font decodes");
    assert_eq!(font.family(), "Inter");
    assert_eq!(font.bytes(), INTER);
}
