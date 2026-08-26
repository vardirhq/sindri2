//! One tick of the scripts a world holds.

use std::collections::BTreeMap;

use decay_ir::{IrContainer, lower_with_environment};
use decay_runtime::{Runtime, ScriptInstance, Value};
use sindri_core::{EntityId, World};
use sindri_platform::InputState;

use crate::{
    Blackboard, ScriptComponent, ScriptContext, ScriptFailure, WorldHost, audio_host::AudioCommand,
};

use super::environment::environment;
use super::sources::{START, ScriptSources, UPDATE};
use super::{Compiled, Running};

#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    programs: &mut BTreeMap<String, Compiled>,
    running: &mut BTreeMap<EntityId, Running>,
    blackboard: &mut Blackboard,
    audio: &mut Vec<AudioCommand>,
    world: &mut World,
    sources: &ScriptSources,
    input: &InputState,
    entity: EntityId,
    component: &ScriptComponent,
    delta_seconds: f32,
) -> Result<Vec<String>, ScriptFailure> {
    ensure_compiled(programs, sources, entity, component)?;
    let compiled = &programs[&component.source];

    let container = compiled
        .program
        .containers
        .iter()
        .find(|container| container.name == component.script)
        .ok_or_else(|| ScriptFailure::UnknownScript {
            entity,
            asset: component.source.clone(),
            script: component.script.clone(),
        })?;

    let elapsed_seconds = running
        .get(&entity)
        .map_or(0.0, |current| current.elapsed_seconds)
        + delta_seconds;
    let context = ScriptContext {
        input,
        delta_seconds,
        elapsed_seconds,
    };
    let mut runtime = Runtime::new(
        &compiled.program,
        WorldHost::new(world, entity, context, blackboard, audio),
    );

    let started = match running.get(&entity) {
        Some(current)
            if current.source == component.source && current.script == component.script =>
        {
            false
        }
        _ => {
            let mut instance = runtime.instantiate(&component.script).map_err(|error| {
                ScriptFailure::runtime(entity, &component.script, START, &error)
            })?;
            apply_properties(&mut instance, container, entity, component)?;
            running.insert(
                entity,
                Running {
                    elapsed_seconds,
                    source: component.source.clone(),
                    script: component.script.clone(),
                    instance,
                },
            );
            true
        }
    };

    let Some(current) = running.get_mut(&entity) else {
        return Ok(Vec::new());
    };
    current.elapsed_seconds = elapsed_seconds;

    if started && container.functions.iter().any(|f| f.name == START) {
        runtime
            .call_instance(&mut current.instance, START, vec![])
            .map_err(|error| ScriptFailure::runtime(entity, &component.script, START, &error))?;
    }

    if container.functions.iter().any(|f| f.name == UPDATE) {
        runtime
            .call_instance(
                &mut current.instance,
                UPDATE,
                vec![Value::Number(f64::from(delta_seconds))],
            )
            .map_err(|error| ScriptFailure::runtime(entity, &component.script, UPDATE, &error))?;
    }

    Ok(runtime.into_host().take_printed())
}

pub(super) fn ensure_compiled(
    programs: &mut BTreeMap<String, Compiled>,
    sources: &ScriptSources,
    entity: EntityId,
    component: &ScriptComponent,
) -> Result<(), ScriptFailure> {
    let Some(source) = sources.get(&component.source) else {
        return Err(ScriptFailure::MissingSource {
            entity,
            asset: component.source.clone(),
        });
    };
    if programs
        .get(&component.source)
        .is_some_and(|compiled| compiled.source == source)
    {
        return Ok(());
    }

    let lowered = lower_with_environment(source, &environment());
    let program = lowered.program.ok_or_else(|| ScriptFailure::Compile {
        asset: component.source.clone(),
        diagnostics: lowered
            .analysis
            .diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "{}:{}: {}",
                    diagnostic.line, diagnostic.column, diagnostic.message
                )
            })
            .collect(),
    })?;
    programs.insert(
        component.source.clone(),
        Compiled {
            source: source.to_owned(),
            program,
        },
    );
    Ok(())
}

pub(super) fn apply_properties(
    instance: &mut ScriptInstance,
    container: &IrContainer,
    entity: EntityId,
    component: &ScriptComponent,
) -> Result<(), ScriptFailure> {
    let refuse = |property: &str, reason: &str| ScriptFailure::Property {
        entity,
        script: component.script.clone(),
        property: property.to_owned(),
        reason: reason.to_owned(),
    };

    for (name, value) in &component.properties {
        let Some(field) = container.fields.iter().find(|field| field.name == *name) else {
            return Err(refuse(name, "the script declares no such field"));
        };
        if !field.exported {
            return Err(refuse(name, "the field is not @export"));
        }
        let value = to_value(value).ok_or_else(|| {
            refuse(
                name,
                &format!("{value} is not a number, string, or boolean"),
            )
        })?;
        instance
            .set_field(name, value)
            .map_err(|error| refuse(name, &format!("{error:?}")))?;
    }
    Ok(())
}

pub(super) fn to_value(value: &serde_json::Value) -> Option<Value> {
    Some(match value {
        serde_json::Value::Number(number) => Value::Number(number.as_f64()?),
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::String(value) => Value::String(value.clone()),
        serde_json::Value::Null => Value::Null,
        _ => return None,
    })
}
