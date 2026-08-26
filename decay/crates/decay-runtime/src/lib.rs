//! Small interpreter for Decay IR.
//!
//! The runtime deliberately knows nothing about Sindri. Host integrations
//! provide path loads/stores and callable functions through [`Host`].
//!
//! `value` is what an expression evaluates to, `host` is the seam an
//! integration reaches through, `instance` is a script attached to something,
//! and `runtime` is the loop that runs one.

mod error;
mod host;
mod instance;
mod runtime;
mod value;

#[cfg(test)]
mod tests;

pub use decay_ir::Path;
pub use error::RuntimeError;
pub use host::{EmptyHost, Host};
pub use instance::ScriptInstance;
pub use runtime::{DEFAULT_CALL_DEPTH_LIMIT, Runtime};
pub use value::Value;
