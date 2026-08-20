//! Decay, bound to a Sindri world.
//!
//! This is the only crate that knows both halves. `decay-*` knows a language and
//! nothing about entities; the engine knows entities and nothing about scripts.
//! Here a `sindri.script` component names a source and a container, and a
//! [`WorldHost`] gives the language's symbolic paths — `this.transform.position.x`
//! is four strings to the IR — a meaning in terms of a real entity.
//!
//! The direction of the dependency is the point, and it is one way. Nothing
//! under `decay/` may depend on a `sindri-*` crate, and no other engine crate
//! depends on this one. Decay stays a language that could be lifted out and
//! replaced for exactly as long as that holds; see `docs/decay-direction.md` for
//! why that reversibility is what makes committing to it affordable.
//!
//! What this crate deliberately does not do: **any I/O**. It never opens a
//! `.decay` file. Sources arrive through [`ScriptSources`], filled by whoever
//! owns the asset pipeline, exactly as textures arrive at `sindri-scene`
//! through `TextureBindings` rather than being loaded there. That keeps every
//! test here filesystem-free and browser-free, and keeps one answer to how an
//! asset is fetched instead of two.
//!
//! It also does not know what a frame is, or when to run. It moves scripts on by
//! a delta the caller decides — the editor's transport decides what a frame is
//! worth, the same way it does for sprite animation.

mod component;
mod error;
mod host;
mod scripts;
mod surface;

pub use component::ScriptComponent;
pub use error::ScriptFailure;
pub use host::WorldHost;
pub use scripts::{ScriptSources, Scripts, environment, referenced_sources};
