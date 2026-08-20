//! What a texture registry does when a texture is released.
//!
//! The bookkeeping is exact arithmetic, but a registry cannot be built without a
//! device — it generates its missing-texture fallback on creation — so this runs
//! beside the other tests that need an adapter rather than as a unit test that
//! could not compile without one.
//!
//! The property under test is the one that makes releasing safe at all: a handle
//! to a released texture must resolve to the missing checker, and must keep
//! resolving there after something else has taken the slot. Without that, the
//! failure is not a crash — it is a scene quietly drawing the wrong picture.

use sindri_gpu::{GpuContext, GpuRequestOptions};
use sindri_render::{Texture2D, TextureRegistry};

/// Set wherever a software adapter is installed on purpose. A GPU test that
/// skips on the machine that exists to run it is a check that quietly stopped
/// checking, so CI demands the adapter rather than hoping for it.
const REQUIRE_GPU: &str = "SINDRI_REQUIRE_GPU";

fn gpu() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();
    match pollster::block_on(GpuContext::request(
        &instance,
        None,
        &GpuRequestOptions::default(),
    )) {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            assert!(
                std::env::var_os(REQUIRE_GPU).is_none(),
                "{REQUIRE_GPU} is set but no adapter could be requested: {error}"
            );
            eprintln!("skipping: no GPU adapter ({error})");
            None
        }
    }
}

fn pixel(gpu: &GpuContext, label: &str, color: [u8; 4]) -> Texture2D {
    Texture2D::from_rgba8(&gpu.device, &gpu.queue, label, 1, 1, &color)
        .expect("a one pixel texture is valid")
}

/// The whole reason a texture handle carries a generation.
#[test]
fn a_released_texture_draws_as_missing_even_after_its_slot_is_reused() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut registry = TextureRegistry::new(&gpu.device, &gpu.queue);
    let green = registry.insert(pixel(&gpu, "green", [0, 255, 0, 255]));
    let blue = registry.insert(pixel(&gpu, "blue", [0, 0, 255, 255]));

    assert!(registry.remove(green), "the texture was there to release");
    assert!(
        !registry.remove(green),
        "and releasing it twice is not a second release"
    );
    assert!(
        std::ptr::eq(registry.get(green), registry.get(TextureRegistry::MISSING)),
        "a released handle has to resolve to the fallback"
    );

    // The slot comes back, which is the point of releasing at all.
    let red = registry.insert(pixel(&gpu, "red", [255, 0, 0, 255]));
    assert_eq!(red.index(), green.index(), "the freed slot was reused");
    assert_ne!(red, green, "but the handle for it is a different handle");
    assert_ne!(
        red.generation(),
        green.generation(),
        "which is what the generation is for"
    );
    assert!(
        std::ptr::eq(registry.get(green), registry.get(TextureRegistry::MISSING)),
        "the old handle must not draw whatever took its slot"
    );
    assert!(
        !std::ptr::eq(registry.get(red), registry.get(TextureRegistry::MISSING)),
        "and the new one draws the new texture"
    );
    assert!(
        !std::ptr::eq(registry.get(blue), registry.get(TextureRegistry::MISSING)),
        "releasing one texture leaves the others alone"
    );
}

/// A registry that only grew would hold every texture a session ever loaded,
/// which stopped being theoretical the moment hot reload made replacing one a
/// keystroke.
#[test]
fn releasing_and_inserting_in_turn_does_not_grow_the_registry() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut registry = TextureRegistry::new(&gpu.device, &gpu.queue);
    let held = registry.len();

    let mut current = registry.insert(pixel(&gpu, "first", [10, 20, 30, 255]));
    for round in 0..50 {
        assert!(registry.remove(current));
        current = registry.insert(pixel(&gpu, "again", [40, 50, 60, 255]));
        assert_eq!(
            registry.len(),
            held + 1,
            "round {round} left the registry holding more than it needs"
        );
    }
    assert_eq!(registry.ids().count(), held + 1);
}

/// The fallback is what every stale handle resolves to, so a registry without
/// one would have nothing to answer with.
#[test]
fn the_missing_texture_cannot_be_released() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut registry = TextureRegistry::new(&gpu.device, &gpu.queue);
    assert!(!registry.remove(TextureRegistry::MISSING));
    assert_eq!(registry.len(), 1);
    // Still the fallback checkerboard, at the size the registry generates it.
    assert_eq!(registry.get(TextureRegistry::MISSING).width(), 64);
}
