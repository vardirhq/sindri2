//! Edits the inspector holds back until they are due.
//!
//! A field's [`ApplyMode`] says when an edit to it reaches the world. An
//! [`ApplyMode::Instant`] one goes as it is made. The others are held here,
//! as the value typed at the path it was typed at, and shown in the inspector
//! over what the world holds, so the field reads as edited while the scene
//! still shows what it was. A [`ApplyMode::Settled`] edit goes when the person
//! has stopped; a [`ApplyMode::Manual`] one when they press Apply.
//!
//! Held as paths rather than as whole components, so an undo, or a script,
//! changing another field of the same component meanwhile is not overwritten
//! by a stale copy when the held edit lands.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde_json::Value;
use sindri_core::{ApplyMode, ComponentSchemaRegistry, EntityId};

/// How long typing must pause before a settled edit applies.
pub const SETTLE_AFTER: Duration = Duration::from_millis(400);

/// One held change: the value at a path, or `None` for a key removed.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    mode: ApplyMode,
    value: Option<Value>,
}

/// Components by name, as an entity stores them.
pub type Components = BTreeMap<String, Value>;

/// What the person did to one component this frame, besides typing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked<'a> {
    /// Apply this component's manual edits.
    pub apply: Option<&'a str>,
    /// Throw this component's held edits away.
    pub revert: Option<&'a str>,
}

#[derive(Debug, Default)]
pub struct HeldEdits {
    /// Entity, then component, then field path.
    held: BTreeMap<EntityId, BTreeMap<String, BTreeMap<String, Held>>>,
    last_edit: Option<Instant>,
}

impl HeldEdits {
    /// `stored` with this entity's held edits laid over it: what the
    /// inspector shows.
    #[must_use]
    pub fn shown(&self, entity: EntityId, stored: &Components) -> Components {
        let mut shown = stored.clone();
        for (name, paths) in self.held.get(&entity).into_iter().flatten() {
            if let Some(payload) = shown.get_mut(name) {
                for (path, held) in paths {
                    set_at(payload, path, held.value.clone());
                }
            }
        }
        shown
    }

    /// The components of `entity` with edits waiting for Apply.
    #[must_use]
    pub fn waiting(&self, entity: EntityId) -> BTreeSet<String> {
        self.held
            .get(&entity)
            .into_iter()
            .flatten()
            .filter(|(_, paths)| paths.values().any(|held| held.mode == ApplyMode::Manual))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Whether anything waits to settle, so a host keeps asking for frames
    /// until it has.
    #[must_use]
    pub fn settling(&self) -> bool {
        self.held
            .values()
            .flat_map(BTreeMap::values)
            .flat_map(BTreeMap::values)
            .any(|held| held.mode == ApplyMode::Settled)
    }

    /// Whether settled edits may apply now: nothing is being dragged, and
    /// the person has left the field or paused long enough.
    #[must_use]
    pub fn settled(&self, now: Instant, pointer_down: bool, field_focused: bool) -> bool {
        !pointer_down
            && (!field_focused
                || self
                    .last_edit
                    .is_none_or(|last| now.duration_since(last) >= SETTLE_AFTER))
    }

    /// Sorts this frame's edits by how each applies, and answers with what
    /// the world should now hold for `entity`.
    ///
    /// `stored` is what the world holds, `shown` what the inspector began
    /// the frame showing, and `edited` what it ended it with. Instant edits
    /// go into the answer; the rest are held, and go into it once due:
    /// settled ones when `settled`, a component's manual ones when
    /// `asked.apply` names it.
    #[allow(clippy::too_many_arguments)]
    pub fn sort(
        &mut self,
        registry: &ComponentSchemaRegistry,
        entity: EntityId,
        stored: &Components,
        shown: &Components,
        edited: &Components,
        settled: bool,
        asked: Asked<'_>,
        now: Instant,
    ) -> Components {
        let mut target = stored.clone();
        let mut typed = false;
        for (name, after) in edited {
            let (Some(before), Some(payload)) = (shown.get(name), target.get_mut(name)) else {
                continue;
            };
            let mut changes = Vec::new();
            differences(before, after, String::new(), &mut changes);
            for (path, value) in changes {
                match registry.apply_mode(name, &path) {
                    ApplyMode::Instant => set_at(payload, &path, value),
                    mode => {
                        self.hold(entity, name, path, Held { mode, value });
                        self.last_edit = Some(now);
                        typed = true;
                    }
                }
            }
        }
        // An edit made this frame has not been stopped on yet, whatever the
        // focus says: the frame a dropdown's choice lands in is the frame
        // before it counts as left.
        let settled = settled && !typed;
        let Some(components) = self.held.get_mut(&entity) else {
            return target;
        };
        if let Some(name) = asked.revert {
            components.remove(name);
        }
        for (name, paths) in components.iter_mut() {
            let Some(payload) = target.get_mut(name) else {
                // The component went, and its held edits with it.
                paths.clear();
                continue;
            };
            let applying = asked.apply == Some(name.as_str());
            paths.retain(|path, held| {
                let due = match held.mode {
                    ApplyMode::Instant => true,
                    ApplyMode::Settled => settled,
                    ApplyMode::Manual => applying,
                };
                if due {
                    set_at(payload, path, held.value.clone());
                    return false;
                }
                // Typed back to what is stored: nothing is waiting.
                get_at(payload, path) != held.value.as_ref()
            });
        }
        components.retain(|_, paths| !paths.is_empty());
        if components.is_empty() {
            self.held.remove(&entity);
        }
        target
    }

    /// Settles every edit held for an entity other than `except`, now that
    /// the person has moved on from it: for each such entity, what it
    /// stores and what it should store instead. `stored` reads an entity's
    /// components; one that has gone has its edits forgotten.
    pub fn settle_others(
        &mut self,
        except: Option<EntityId>,
        stored: impl Fn(EntityId) -> Option<Components>,
    ) -> Vec<(EntityId, Components, Components)> {
        let mut settled = Vec::new();
        for (entity, components) in &mut self.held {
            if Some(*entity) == except {
                continue;
            }
            let Some(before) = stored(*entity) else {
                components.clear();
                continue;
            };
            let mut after = before.clone();
            for (name, paths) in components.iter_mut() {
                let payload = after.get_mut(name);
                let mut payload = payload;
                paths.retain(|path, held| {
                    if held.mode != ApplyMode::Settled {
                        return true;
                    }
                    if let Some(payload) = payload.as_deref_mut() {
                        set_at(payload, path, held.value.clone());
                    }
                    false
                });
            }
            components.retain(|_, paths| !paths.is_empty());
            if after != before {
                settled.push((*entity, before, after));
            }
        }
        self.held.retain(|_, components| !components.is_empty());
        settled
    }

    /// Forgets what was held for entities that no longer exist.
    pub fn retain(&mut self, alive: impl Fn(EntityId) -> bool) {
        self.held.retain(|entity, _| alive(*entity));
    }

    fn hold(&mut self, entity: EntityId, name: &str, path: String, held: Held) {
        let paths = self
            .held
            .entry(entity)
            .or_default()
            .entry(name.to_owned())
            .or_default();
        // An edit inside one already held changes that one: the held value
        // is what the field shows, and it is what the edit was made to.
        let holder = paths
            .keys()
            .find(|holder| path.starts_with(&format!("{holder}.")))
            .cloned();
        if let Some(holder) = holder {
            let within = path[holder.len() + 1..].to_owned();
            let outer = paths.get_mut(&holder).expect("found above");
            if let Some(value) = &mut outer.value {
                set_at(value, &within, held.value);
            }
            outer.mode = strictest(outer.mode, held.mode);
            return;
        }
        // One that replaces edits held inside it supersedes them.
        paths.retain(|inner, _| !inner.starts_with(&format!("{path}.")));
        paths.insert(path, held);
    }
}

/// Manual over settled, so an edit that would be held until Apply is.
const fn strictest(left: ApplyMode, right: ApplyMode) -> ApplyMode {
    match (left, right) {
        (ApplyMode::Manual, _) | (_, ApplyMode::Manual) => ApplyMode::Manual,
        (ApplyMode::Settled, _) | (_, ApplyMode::Settled) => ApplyMode::Settled,
        _ => ApplyMode::Instant,
    }
}

/// Every place `after` differs from `before`, as the deepest path that
/// holds the whole of each difference.
fn differences(
    before: &Value,
    after: &Value,
    path: String,
    out: &mut Vec<(String, Option<Value>)>,
) {
    if before == after {
        return;
    }
    let join = |key: &str| {
        if path.is_empty() {
            key.to_owned()
        } else {
            format!("{path}.{key}")
        }
    };
    match (before, after) {
        (Value::Object(old), Value::Object(new)) => {
            for (key, value) in new {
                match old.get(key) {
                    Some(was) => differences(was, value, join(key), out),
                    None => out.push((join(key), Some(value.clone()))),
                }
            }
            for key in old.keys().filter(|key| !new.contains_key(*key)) {
                out.push((join(key), None));
            }
        }
        (Value::Array(old), Value::Array(new)) if old.len() == new.len() && !path.is_empty() => {
            for (index, (was, value)) in old.iter().zip(new).enumerate() {
                differences(was, value, join(&index.to_string()), out);
            }
        }
        _ if path.is_empty() => {}
        _ => out.push((path, Some(after.clone()))),
    }
}

fn get_at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |here, segment| match here {
        Value::Array(items) => items.get(segment.parse::<usize>().ok()?),
        _ => here.get(segment),
    })
}

/// Writes `new` at `path`, or removes the key there for `None`. A path whose
/// parent is gone, a list item removed meanwhile, is left alone.
fn set_at(value: &mut Value, path: &str, new: Option<Value>) {
    let (parent, last) = match path.rsplit_once('.') {
        Some((parent, last)) => (Some(parent), last),
        None => (None, path),
    };
    let holder = match parent {
        Some(parent) => {
            let mut here = &mut *value;
            for segment in parent.split('.') {
                let next = match here {
                    Value::Array(items) => segment
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| items.get_mut(index)),
                    Value::Object(fields) => fields.get_mut(segment),
                    _ => None,
                };
                let Some(next) = next else {
                    return;
                };
                here = next;
            }
            here
        }
        None => value,
    };
    match (holder, new) {
        (Value::Object(fields), Some(new)) => {
            fields.insert(last.to_owned(), new);
        }
        (Value::Object(fields), None) => {
            fields.remove(last);
        }
        (Value::Array(items), Some(new)) => {
            if let Some(item) = last
                .parse::<usize>()
                .ok()
                .and_then(|index| items.get_mut(index))
            {
                *item = new;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
