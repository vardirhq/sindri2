//! Short-lived visual flecks that are not entities.
//!
//! `docs/effect-scaling.md` is the measurement that put this here. An entity per
//! fleck costs about 5 ms a frame at eight thousand of them — a third of a 60 Hz
//! budget — and over half of that is re-reading each one's component payload
//! through `serde_json`, every entity, every frame. The same population as plain
//! values in a pool costs 0.018 ms.
//!
//! So a fleck is not an entity. It has no identity a script can hold, no
//! components, no place in the hierarchy, and nothing can collide with it. That
//! is the whole trade: everything an entity is *for* is given up, and what comes
//! back is two orders of magnitude.
//!
//! What a fleck looks like is authored, because the parameters of a burst are a
//! designer's decision and a wall of arguments in a script is not. An entity
//! carries `sindri.effect.burst`, and a script fires it.

mod burst;
mod pool;

pub use burst::EffectBurstComponent;
pub use pool::{Effects2d, Fleck};
