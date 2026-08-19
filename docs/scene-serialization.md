# Scene serialization and round-tripping

A Sindri scene is a versioned JSON document. It is the format an editor saves, a runtime loads, and
a reviewer reads in a pull request, so it has to survive all three without drifting. This document
describes the guarantees the format makes.

## The round-trip contract

```rust
let document = SceneDocument::from_json(&text)?;   // parse and validate
let loaded = World::from_scene(&document)?;        // logical IDs -> runtime handles
// ... edit the world ...
let saved = loaded.world.to_scene()?;              // runtime handles -> logical IDs
let text = saved.to_canonical_json()?;             // canonical bytes
```

Loading and saving an unmodified scene reproduces the file byte for byte. Golden fixtures in
`crates/sindri-core/tests/fixtures` enforce this: they are stored in canonical form, so any change
to ordering or formatting fails the test suite instead of quietly rewriting everyone's scenes.
Regenerate them deliberately with:

```bash
SINDRI_UPDATE_SCENE_FIXTURES=1 cargo test --workspace
```

## Stable IDs are never regenerated

Serialized entities are identified by a project-authored `SceneEntityId`. Runtime `EntityId` handles
are generation-checked slots that change as entities are spawned and destroyed, and they are never
written to disk. `World::to_scene` writes back each entity's original ID, so saving does not
renumber a scene or invalidate references to it.

An entity spawned at runtime has no stable ID. Rather than inventing one during a save or dropping
the entity, `to_scene` reports `WorldError::UnstableEntity`. Tools that create entities call
`World::assign_missing_source_ids` first, which mints IDs in entity order and skips identities
already in use, so the same world always produces the same assignment.

## Canonical form

`SceneDocument::to_canonical_json` produces the one representation a document is stored as:

- entities sorted by stable ID;
- object keys sorted, including inside component payloads the engine does not interpret;
- absent names, parents, transforms, components, and editor sections omitted rather than written as
  `null` or `{}`;
- arrays of scalars kept on one line when they fit within 96 columns, so a three-component position
  is one reviewable line rather than five;
- two-space indentation and a trailing newline.

Serializing an already canonical document is a fixed point, so repeated saves stop producing diffs.

Document order carries no rendering meaning. Draw order comes from explicit render layers and from
where things are relative to the camera (see [transparent rendering](rendering-transparency.md)),
which is why sorting entities is safe: it keeps saves stable while entities are added, removed, and
reparented.

Scenes reject non-finite transform values. JSON has no `NaN` or `Infinity` literal, so a scene
containing one could not be read back; validation catches it at the point it is introduced instead.

## Editor-only metadata

Tooling state lives in namespaced `editor` sections — one on the document and one on each entity:

```json
{
  "format_version": 1,
  "metadata": { "name": "Room", "editor": { "grid_snap": 0.25 } },
  "entities": [
    { "id": "player", "editor": { "collapsed": true } }
  ]
}
```

Runtimes carry these sections through a load and save unchanged but never interpret them, so a scene
loads identically with or without them. Export pipelines drop them with
`SceneDocument::strip_editor_metadata`.

Unregistered component payloads get the same treatment for the same reason: an older tool preserves
data a newer one authored. See the [component schema registry](component-schema-registry.md).

## Versioning and migration

Every document declares `format_version`. The current format is `SCENE_FORMAT_VERSION`, and a
runtime rejects any other version rather than guessing at its meaning.

`SceneMigrator` existed before format version 2 did, so each real format change has been a
registration rather than a redesign:

```rust
let mut migrator = SceneMigrator::new();
migrator.register(1, 2, upgrade_v1_to_v2)?;
let document = SceneDocument::from_json_migrated(&text, &migrator)?;
```

Migration steps operate on raw JSON, because an old document cannot by definition be deserialized
into the current `SceneDocument`. The migrator enforces the properties that keep a chain honest:

- steps must move strictly forward, so a chain cannot loop;
- steps may not target a version this runtime does not support;
- one step per source version;
- the target version is stamped by the migrator, not by each step;
- a document newer than this runtime is rejected as such, and a document older than any registered
  step reports the missing migration by version.

Documents already at the current version pass through untouched, so a migrator with no registered
steps behaves exactly like plain parsing.
