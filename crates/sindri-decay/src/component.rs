use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;
use sindri_core::SceneComponent;

/// A Decay script attached to an entity as `sindri.decay.script`.
///
/// ```json
/// "sindri.decay.script": {
///   "source": "scripts/spin.decay",
///   "script": "Spin",
///   "properties": { "turns_per_second": 0.5 }
/// }
/// ```
///
/// `source` and `script` are separate because one file may declare several
/// containers, and naming the container in the scene is what lets a shared
/// library of scripts exist without a file per behaviour.
///
/// `properties` are the authored values for the container's `@export` fields.
/// They live in the scene rather than in the script because that is the whole
/// distinction: the script says a speed exists and what it defaults to, the
/// scene says what *this* entity's speed is. It is also the capability that
/// made a typed language worth having — see `docs/decay-direction.md`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ScriptComponent {
    /// The `.decay` source, as a logical asset ID.
    pub source: String,
    /// Which `script` container in that source drives this entity.
    pub script: String,
    /// Authored values for the container's exported fields.
    #[serde(default)]
    pub properties: BTreeMap<String, Value>,
    /// Whether this script runs. A disabled script is still authored, still
    /// inspectable, and still saved — it simply does not tick, which is what
    /// an author wants while narrowing down which script is misbehaving.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

const fn enabled_by_default() -> bool {
    true
}

impl SceneComponent for ScriptComponent {
    const TYPE_NAME: &'static str = "sindri.decay.script";
}

#[cfg(test)]
mod tests {
    use sindri_core::SceneComponent;

    use super::ScriptComponent;

    #[test]
    fn script_has_the_decay_namespace() {
        assert_eq!(ScriptComponent::TYPE_NAME, "sindri.decay.script");
    }
}
