//! The hosts the runtime tests run against.

use std::collections::HashMap;

use decay_semantic::{Environment, Type};

use crate::{Host, Path, RuntimeError, Value};

#[derive(Default)]
pub(super) struct TestHost {
    values: HashMap<String, Value>,
}

impl Host for TestHost {
    fn load(&mut self, _subject: Option<u64>, path: &Path) -> Result<Option<Value>, RuntimeError> {
        Ok(self.values.get(&path.dotted()).cloned())
    }
    fn store(
        &mut self,
        _subject: Option<u64>,
        path: &Path,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        self.values.insert(path.dotted(), value);
        Ok(true)
    }
    fn call(
        &mut self,
        _subject: Option<u64>,
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        if path.dotted() == "Input.axis" {
            assert_eq!(args.len(), 2);
            return Ok(Some(Value::Number(0.5)));
        }
        Ok(None)
    }
}

/// Counts what reaches the host, so a call that should not happen can be
/// asserted absent rather than merely unobserved.
#[derive(Default)]
pub(super) struct CountingHost {
    pub(super) probes: usize,
}

impl Host for CountingHost {
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
        path: &Path,
        _args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        if path.dotted() == "Probe.ready" {
            self.probes += 1;
            return Ok(Some(Value::Bool(true)));
        }
        Ok(None)
    }
}

pub(super) fn probing_environment() -> Environment {
    let mut environment = Environment::new();
    environment.add_value("Probe", Type::Named("Probe".to_owned()));
    environment
}
