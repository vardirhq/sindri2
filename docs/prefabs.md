# Prefabs

An authored reusable entity definition, and the thing `World.spawn` creates.

## Why there is one

A script could find another entity, reach through it, check whether it still
existed, and remove it. It could not make one. That was not an oversight —
`docs/scripting.md` recorded it as a real dependency: creating an entity means
saying *what* to create, and the engine had nothing to say it with.

So a game could only ever have the entities its scene was authored with. Every
enemy, bullet, pickup, and effect had to be placed by hand before the run began.
`docs/orbital-last-stand-audit.md` found this first among the reasons a
real-time action game could not be authored through the editor and Decay.

## What one is

A scene fragment with exactly one root.

```json
{
  "format_version": 1,
  "metadata": { "name": "Bullet" },
  "entities": [
    {
      "id": "bullet",
      "name": "Bullet",
      "transform_3d": { "position": [0.0, 0.0, 0.0] },
      "components": {
        "sindri.script": { "source": "scripts/bullet.decay", "script": "Bullet" },
        "sindri.sprite": { "texture": "textures/bullet.png", "layer": 3 }
      }
    }
  ]
}
```

That is a `SceneEntity`, unchanged, and deliberately: a prefab reuses the
scene's entity shape, its component payloads, its identities, its canonical
serialization, and its validation. **It is not a second document format.** A
separate shape for "a subtree of entities" would be a copy of the scene format
that drifts from it, and a prefab that could not hold a component a scene can
hold would be a trap discovered late.

The rules a scene's entities obey are in `scene::graph`, shared by both, rather
than written twice — the second copy of a validator is the one that stops
matching the first.

### The one rule a prefab adds

**Exactly one root.** A prefab with several is refused when it is read.
`World.spawn` answers with a reference, and a document with two roots would
make it answer with one of them and leave the other attached to nothing an
author can name.

### Its own format version

`PREFAB_FORMAT_VERSION` is not `SCENE_FORMAT_VERSION`. The two documents share
an entity shape and will share its migrations, but they are separate files with
separate histories: a prefab gaining a document-level field is not a reason to
step every scene in a project. A version this runtime does not understand is
refused rather than guessed at, exactly as a scene's is.

## Spawning one

From Decay, `World.spawn(prefab)`. The full contract — that a prefab reference
is a typed `Prefab` rather than text, how overrides work, when a spawned
script's `start` runs, and what the bounds are — is in `docs/scripting.md`,
because it is a statement about the scripting surface.

From Rust, `World::spawn_prefab` returns a `SpawnedPrefab`: the root, every
entity created, and which authored identity each became.

### Instances have no stable identity

Spawned entities carry no `source_id`. A prefab's identities name entities
*inside the prefab*; two instances carrying them would collide on every one, and
a scene saved with the collision would refuse to load.

`World::assign_missing_source_ids` remains how a runtime entity earns a stable
identity. That is a decision about persisting a world, not about spawning, and
keeping the two apart is why saving a world full of bullets is a thing you ask
for rather than a thing that happens.

### Editor-only state does not come along

A prefab's `editor` sections describe the prefab in the editor — what is folded,
what is selected. An instance in a running world has no use for them, and
carrying them would put a fold state on every bullet.

### Nothing is half-created

The document is validated before anything reaches the world. A prefab with
several roots, a missing parent, or a non-finite transform spawns nothing at
all rather than leaving a partial subtree behind.

## Loading one

`.prefab.json`, resolved against the scene's directory like every other asset,
and delivered by the same text pipeline the scripts use — a prefab is JSON, and
the pipeline has no reason to know more than that. A document that will not
parse is reported once, when it loads, rather than on the frame a script spawns
it.

Which prefabs a scene needs is answered by the *declared type* of each script's
exported fields: a field declared `Prefab` is a prefab reference, and one
declared `String` is text however much it looks like a path. That is what makes
a scene's prefabs loadable before the first frame instead of discovered when a
script spawns.

Because the declared types are not known until a source has compiled, the set is
asked for every frame rather than once at open — which is also what makes a
prefab authored a moment ago load a moment later.

## What is not here yet

- **No instance link.** A spawned entity does not remember which prefab it came
  from, so editing a prefab does not update instances of it in an open scene.
  That is a real feature and a larger one: it needs a notion of which values on
  an instance are overrides and which are inherited.
- **No editor authoring.** Nothing yet makes a prefab out of a selected entity,
  and the inspector draws a `Prefab` field as the string it is stored as rather
  than as an asset picker. Both are queued behind the capability itself.
- **No nested prefabs.** A prefab's entities are entities, not references to
  other prefabs.
