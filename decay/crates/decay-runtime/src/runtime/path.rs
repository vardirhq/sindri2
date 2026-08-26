//! Reading and writing through a path, whether it is the host's or the
//! instance's.

use std::collections::HashMap;

use decay_ir::Path;

use crate::error::RuntimeError;
use crate::host::Host;
use crate::instance::Slot;
use crate::value::Value;

use super::{Frame, Runtime};

impl<H: Host> Runtime<'_, H> {
    /// Splits a path rooted at a value the script is holding into that value's
    /// reference and the rest of the path.
    ///
    /// `target.transform.position.x` where `target` holds a reference becomes
    /// `(target's id, transform.position.x)`, which is what lets one script say
    /// anything about another entity. A path rooted at anything else — a host
    /// global like `Input`, or `this` — is not a subject path and goes to the
    /// host whole.
    ///
    /// A root that names a local holding something *other* than a reference is
    /// an error rather than a fall-through to the host: `speed.transform` where
    /// `speed` is a number should say so, not report an unknown host path that
    /// mentions a local the host has never heard of.
    pub(super) fn subject_path(
        fields: &HashMap<String, Slot>,
        frame: &Frame,
        path: &Path,
    ) -> Result<Option<(u64, Path)>, RuntimeError> {
        if path.0.len() < 2 {
            return Ok(None);
        }
        let root = &path.0[0];
        let Some(slot) = frame.lookup(root).or_else(|| fields.get(root)) else {
            return Ok(None);
        };
        match slot.value {
            Value::Reference(id) => Ok(Some((id, Path(path.0[1..].to_vec())))),
            Value::Null => Err(RuntimeError::NullReference(path.dotted())),
            _ => Err(RuntimeError::NotAReference(root.clone())),
        }
    }

    pub(super) fn load_path(
        &mut self,
        fields: &HashMap<String, Slot>,
        frame: &Frame,
        path: &Path,
    ) -> Result<Value, RuntimeError> {
        if path.0.len() == 1 {
            let name = &path.0[0];
            if let Some(slot) = frame.lookup(name).or_else(|| fields.get(name)) {
                return Ok(slot.value.clone());
            }
        }
        if path.0.len() == 2
            && path.0[0] == "this"
            && let Some(slot) = fields.get(&path.0[1])
        {
            return Ok(slot.value.clone());
        }
        if let Some((subject, rest)) = Self::subject_path(fields, frame, path)? {
            return self
                .host
                .load(Some(subject), &rest)?
                .ok_or_else(|| RuntimeError::UnknownPath(rest.dotted()));
        }
        self.host
            .load(None, path)?
            .ok_or_else(|| RuntimeError::UnknownPath(path.dotted()))
    }

    pub(super) fn store_path(
        &mut self,
        fields: &mut HashMap<String, Slot>,
        frame: &mut Frame,
        path: &Path,
        value: Value,
    ) -> Result<(), RuntimeError> {
        if path.0.len() == 1 {
            let name = &path.0[0];
            if let Some(slot) = frame.lookup_mut(name) {
                if !slot.mutable {
                    return Err(RuntimeError::Immutable(name.clone()));
                }
                slot.value = value;
                return Ok(());
            }
            if let Some(slot) = fields.get_mut(name) {
                if !slot.mutable {
                    return Err(RuntimeError::Immutable(name.clone()));
                }
                slot.value = value;
                return Ok(());
            }
        }
        if path.0.len() == 2
            && path.0[0] == "this"
            && let Some(slot) = fields.get_mut(&path.0[1])
        {
            if !slot.mutable {
                return Err(RuntimeError::Immutable(path.dotted()));
            }
            slot.value = value;
            return Ok(());
        }
        if let Some((subject, rest)) = Self::subject_path(fields, frame, path)? {
            return if self.host.store(Some(subject), &rest, value)? {
                Ok(())
            } else {
                Err(RuntimeError::UnknownPath(rest.dotted()))
            };
        }
        if self.host.store(None, path, value)? {
            Ok(())
        } else {
            Err(RuntimeError::UnknownPath(path.dotted()))
        }
    }
}
