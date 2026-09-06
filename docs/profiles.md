# Profiles

A profile is reusable authored data that exists independently of scene
entities. It is Sindri's equivalent of a Unity `ScriptableObject`: one asset can
be referenced by many script components, edited once, loaded before gameplay,
and shipped with the project.

Orbital Last Stand is the forcing-function game for this capability. Its module
catalog uses one profile for 160 definitions and their generic effects, so a new
module is a data entry rather than another branch in the chooser or module
script.

## File format

Profiles use the `.profile.json` suffix and carry their own format version:

```json
{
  "format_version": 1,
  "name": "Module Catalog",
  "type": "module_catalog",
  "values": {
    "modules": [
      {
        "id": 0,
        "name": "Hot Core",
        "weight": 2.0,
        "enabled": true
      }
    ]
  }
}
```

`name` is an author-facing label. `type` is an optional game-defined category;
the engine does not turn it into a schema or attach behavior to it. `values` is
a JSON object whose values may be numbers, text, flags, groups, or lists. The
payload stays game-owned so enemy tuning, item definitions, dialogue, and module
catalogs do not require new engine components.

An unsupported format version or malformed JSON is rejected while the asset is
loaded. The runtime never guesses at a newer document.

## Editor authoring

The project browser recognizes profile files and can create one from a row's
context menu. Selecting a profile opens a structured inspector rather than a
raw text preview. It edits the profile name and type plus nested values, lists,
and groups; list entries can be appended, duplicated, or removed.

A Decay field declared as `@export let catalog: Profile` is drawn with a profile
asset picker. The picker stores the logical asset ID, not an operating-system
path, and marks a missing reference in the same way as other asset fields.

## Decay access

`Profile` is opaque in Decay. A script receives one through an exported field
and reads it through typed `Profiles.*` functions. Every read has an explicit
fallback, so missing optional data is intentional and the result type remains
known to the analyzer.

```rust
script UpgradeChooser {
    @export let catalog: Profile;

    fn show_first() {
        let count = Profiles.count(this.catalog, "modules");
        if count > 0.0 {
            let name = Profiles.text_at(
                this.catalog, "modules", 0.0, "name", "Unknown"
            );
            let weight = Profiles.number_at(
                this.catalog, "modules", 0.0, "weight", 1.0
            );
        }
    }
}
```

Scalar reads are `number`, `text`, and `flag`. `count` returns the length of a
top-level list. The corresponding `number_at`, `text_at`, and `flag_at` calls
read a named field from one object in that list. `name` and `kind` return the
document's author label and `type` respectively.

Profiles are read-only during a run. Per-run state belongs in script fields,
the shared game board, or saves rather than in project assets.

## Loading and export

The declared `Profile` type is also the dependency edge. Hosts inspect compiled
script exports, load referenced profiles through the ordinary asset pipeline,
and pass them into each script frame. Static export records them as the
`profile` asset kind, and native and browser hosts decode the same document.

That restriction is deliberate: a string that merely looks like a profile path
would be invisible to the editor picker and export gatherer, and would fail only
when a frame happened to use it.
