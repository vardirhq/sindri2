//! The seam a host reaches the runtime through.
//!
//! The runtime knows nothing about Sindri. Path loads and stores and
//! callable functions all arrive through this trait, which is why this
//! crate compiles with no engine in the tree.

use decay_ir::Path;

use crate::error::RuntimeError;
use crate::value::Value;

/// Everything outside the language, across three methods.
///
/// Each takes a `subject`: `None` for a path the script wrote from a root the
/// host owns (`this.transform.position.x`, `Input.axis(...)`), and `Some(id)`
/// for one rooted at a value a script is holding — `target.transform.position.x`
/// where `target` came from the host earlier. The path passed alongside a
/// subject is the part *after* the root, so a host answers
/// `transform.position.x` for whichever thing the subject names.
///
/// Three methods and not six because the subject is an argument rather than a
/// mode: a host that ignores it simply refuses every subject, which is what
/// [`EmptyHost`] does and what a host without references should do.
pub trait Host {
    fn load(&mut self, subject: Option<u64>, path: &Path) -> Result<Option<Value>, RuntimeError>;
    fn store(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        value: Value,
    ) -> Result<bool, RuntimeError>;
    fn call(
        &mut self,
        subject: Option<u64>,
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError>;
}

#[derive(Debug, Default)]
pub struct EmptyHost;

impl Host for EmptyHost {
    fn load(&mut self, _subject: Option<u64>, _path: &Path) -> Result<Option<Value>, RuntimeError> {
        Ok(None)
    }
    fn store(
        &mut self,
        _subject: Option<u64>,
        _path: &Path,
        _value: Value,
    ) -> Result<bool, RuntimeError> {
        Ok(false)
    }
    fn call(
        &mut self,
        _subject: Option<u64>,
        _path: &Path,
        _args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        Ok(None)
    }
}
