//! A counter no two sets of bindings can share.

use std::sync::atomic::{AtomicU64, Ordering};

/// Handed out to every set of bindings that changes, anywhere in the process.
///
/// Per-instance counting is not enough, and the difference is a stale frame. A
/// host that reloads its atlas by *replacing* its bindings rather than mutating
/// them would otherwise hand the renderer a fresh object whose count happens to
/// equal the old one's -- two binds is two binds -- and anything comparing
/// counts would conclude nothing had changed and go on drawing from rects that
/// no longer describe the texture.
///
/// Zero is reserved for bindings nothing has ever been bound into. Two of those
/// really are interchangeable: both resolve everything as missing.
static CLOCK: AtomicU64 = AtomicU64::new(0);

/// A generation nothing else has had.
pub(crate) fn next() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed).saturating_add(1)
}
