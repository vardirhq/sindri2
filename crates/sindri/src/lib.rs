//! Public facade for the Sindri game engine.
//!
//! Feature crates will be re-exported here as they become stable. Keeping a
//! facade prevents games from depending on Sindri's internal crate layout.

pub use sindri_core as core;

pub mod prelude {
    pub use sindri_core::prelude::*;
}
