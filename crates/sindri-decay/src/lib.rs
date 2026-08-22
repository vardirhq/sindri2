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
//! `.decay` file or an audio device. Sources arrive through [`ScriptSources`],
//! and audio calls become [`AudioCommand`] values for the platform host to
//! perform. That keeps every test here filesystem-free and browser-free.
//!
//! It also does not know what a frame is, or when to run. It moves scripts on by
//! a delta the caller decides — the editor's transport decides what a frame is
//! worth, the same way it does for sprite animation.

mod audio_host;
mod blackboard;
mod component;
mod error;
mod exports;
mod host;
mod report;
mod scripts;
mod surface;

pub use audio_host::{AudioCommand, WorldHost, drain_audio_commands};
pub use blackboard::Blackboard;
pub use component::ScriptComponent;
/// A value a Decay script holds, re-exported so a host can name one without
/// depending on the language crates directly.
pub use decay_runtime::Value as ScriptValue;
pub use error::ScriptFailure;
pub use exports::ScriptExport;
pub use host::ScriptContext;
pub use report::{ScriptMessage, ScriptReport};
pub use scripts::{ScriptSources, Scripts, environment, referenced_sources};
