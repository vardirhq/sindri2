//! Entering a function: instantiating, dispatching, and the depth limit.

use std::collections::HashMap;

use decay_ir::{IrContainer, IrFunction};

use crate::error::RuntimeError;
use crate::host::Host;
use crate::instance::{ScriptInstance, Slot};
use crate::value::Value;

use super::{Frame, Runtime};

impl<H: Host> Runtime<'_, H> {
    pub fn instantiate(&mut self, container_name: &str) -> Result<ScriptInstance, RuntimeError> {
        let container = self.find_container(container_name)?.clone();
        let fields = self.initialize_fields(&container)?;
        Ok(ScriptInstance {
            container_name: container_name.to_owned(),
            fields,
        })
    }

    pub fn call(
        &mut self,
        container_name: &str,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let mut instance = self.instantiate(container_name)?;
        self.call_instance(&mut instance, function_name, args)
    }

    pub fn call_instance(
        &mut self,
        instance: &mut ScriptInstance,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let container = self.find_container(&instance.container_name)?.clone();
        self.call_in_container(&container, &mut instance.fields, function_name, args)
    }

    pub(super) fn find_container(&self, name: &str) -> Result<&IrContainer, RuntimeError> {
        self.program
            .containers
            .iter()
            .find(|container| container.name == name)
            .ok_or_else(|| RuntimeError::ContainerNotFound(name.to_owned()))
    }

    pub(super) fn initialize_fields(
        &mut self,
        container: &IrContainer,
    ) -> Result<HashMap<String, Slot>, RuntimeError> {
        let mut fields = HashMap::new();
        for field in &container.fields {
            let value = if let Some(initializer) = &field.initializer {
                let mut frame = Frame::new(HashMap::new());
                self.execute_instructions(container, &mut fields, &mut frame, initializer)?
            } else {
                Value::Null
            };
            fields.insert(
                field.name.clone(),
                Slot {
                    value,
                    mutable: field.mutable,
                },
            );
        }
        Ok(fields)
    }

    pub(super) fn call_in_container(
        &mut self,
        container: &IrContainer,
        fields: &mut HashMap<String, Slot>,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let function = container
            .functions
            .iter()
            .find(|function| function.name == function_name)
            .ok_or_else(|| RuntimeError::FunctionNotFound(function_name.to_owned()))?;
        if function.params.len() != args.len() {
            return Err(RuntimeError::Arity {
                function: function_name.to_owned(),
                expected: function.params.len(),
                found: args.len(),
            });
        }
        let locals = function
            .params
            .iter()
            .cloned()
            .zip(args)
            .map(|(name, value)| {
                (
                    name,
                    Slot {
                        value,
                        mutable: false,
                    },
                )
            })
            .collect();
        let mut frame = Frame::new(locals);

        // Counted here rather than around `execute_instructions`, because a
        // field initializer and a function body both run instructions and only
        // one of them is a call.
        if self.depth >= self.call_depth_limit {
            return Err(RuntimeError::CallDepthExceeded {
                function: function_name.to_owned(),
                limit: self.call_depth_limit,
            });
        }
        self.depth += 1;
        let result = self.execute_function(container, fields, function, &mut frame);
        self.depth -= 1;
        result
    }

    pub(super) fn execute_function(
        &mut self,
        container: &IrContainer,
        fields: &mut HashMap<String, Slot>,
        function: &IrFunction,
        frame: &mut Frame,
    ) -> Result<Value, RuntimeError> {
        self.execute_instructions(container, fields, frame, &function.instructions)
    }
}
