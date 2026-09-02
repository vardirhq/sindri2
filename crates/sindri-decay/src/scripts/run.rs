//! One tick of the scripts a world holds.

use std::collections::{BTreeMap, BTreeSet};

use decay_ir::{IrContainer, lower_with_environment};
use decay_runtime::{Runtime, ScriptInstance, Value};
use sindri_core::{EntityId, World};
use sindri_platform::InputState;

use crate::{
    Blackboard, Physics2d, PrefabSources, ScriptComponent, ScriptContext, ScriptFailure, WorldHost,
    audio_host::AudioCommand, host::Spawning,
};

use super::environment::environment;
use super::sources::{START, ScriptSources, UPDATE};
use super::{Compiled, Running};

/// Everything a tick needs that is not about which entity it is for.
///
/// Grouped rather than passed one by one because the list had grown past what
/// a reader can hold, and every caller passes the same things.
pub(super) struct TickWorld<'a> {
    pub(super) programs: &'a mut BTreeMap<String, Compiled>,
    pub(super) running: &'a mut BTreeMap<EntityId, Running>,
    pub(super) blackboard: &'a mut Blackboard,
    pub(super) audio: &'a mut Vec<AudioCommand>,
    pub(super) world: &'a mut World,
    pub(super) sources: &'a ScriptSources,
    pub(super) prefabs: &'a PrefabSources,
    pub(super) input: &'a InputState,
    /// Which entities have a script instance.
    ///
    /// Owned and updated as the pass runs rather than snapshotted before it.
    /// A snapshot was wrong for the entity being instantiated in the very tick
    /// that reads it: on its first frame a script could author its own
    /// property, change nothing, and be told nothing.
    pub(super) started: BTreeSet<EntityId>,
    /// What every call in this pass has created so far.
    pub(super) spawned: Vec<EntityId>,
    /// The physics a script may read and drive, when the host runs any.
    pub(super) physics: Option<Physics2d<'a>>,
}

pub(super) fn tick(
    at: &mut TickWorld<'_>,
    entity: EntityId,
    component: &ScriptComponent,
    delta_seconds: f32,
) -> Result<Vec<String>, ScriptFailure> {
    ensure_compiled(at.programs, at.sources, entity, component)?;
    let compiled = &at.programs[&component.source];

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

    let elapsed_seconds = at
        .running
        .get(&entity)
        .map_or(0.0, |current| current.elapsed_seconds)
        + delta_seconds;
    let context = ScriptContext {
        input: at.input,
        delta_seconds,
        elapsed_seconds,
    };

    let fresh = !at.running.get(&entity).is_some_and(|current| {
        current.source == component.source && current.script == component.script
    });
    // Recorded before any of this script's code runs, because by then the
    // instance either exists or is being built, and either way its authored
    // properties have been decided.
    at.started.insert(entity);

    let mut runtime = Runtime::new(
        &compiled.program,
        WorldHost::new(
            &mut *at.world,
            entity,
            context,
            &mut *at.blackboard,
            Spawning {
                prefabs: at.prefabs,
                started: &at.started,
                spawned: &mut at.spawned,
            },
            // Reborrowed per tick rather than moved: every script in the pass
            // reads the same frame's events and drives the same world, and one
            // taking physics away from the rest would make which script ran
            // first decide what the others could do.
            at.physics.as_mut().map(|physics| Physics2d {
                world: &mut *physics.world,
                events: physics.events,
            }),
            &mut *at.audio,
        ),
    );

    if fresh {
        let mut instance = runtime
            .instantiate(&component.script)
            .map_err(|error| ScriptFailure::runtime(entity, &component.script, START, &error))?;
        apply_properties(&mut instance, container, entity, component)?;
        at.running.insert(
            entity,
            Running {
                elapsed_seconds,
                source: component.source.clone(),
                script: component.script.clone(),
                instance,
            },
        );
    }

    let Some(current) = at.running.get_mut(&entity) else {
        return Ok(Vec::new());
    };
    current.elapsed_seconds = elapsed_seconds;

    if fresh && container.functions.iter().any(|f| f.name == START) {
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
