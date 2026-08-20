# Scripting

How a Decay script reaches a Sindri world, and what it is allowed to touch.

This is the contract for `sindri-decay`. The language itself is documented in
`decay/README.md`, and why it exists at all in `docs/decay-direction.md`.

## The shape of it

```text
decay/                         the language: syntax, semantics, IR, runtime
  |                            knows nothing about entities
crates/sindri-decay/           the binding: this crate, and only this crate,
  |                            knows both halves
editor/src/scripts.rs          the host: fetches sources, decides what a frame
                               is worth, restores the world when play stops
```

The dependency runs one way only. **Nothing under `decay/` may depend on a
`sindri-*` crate, and no engine crate depends on `sindri-decay`** — the editor
does, as a host. `decay` is also `exclude`d from the engine workspace in the
root `Cargo.toml`: a path dependency alone does not keep a nested workspace
separate, because cargo makes the path dependencies of a member into members
too, which silently gave the decay crates the engine's version and lints
instead of their own.

That isolation is the whole insurance policy on having written a language. It
is what makes the decision reversible, and it is worth more than any feature
that would cost it.

## `sindri.script`

```json
"sindri.script": {
  "source": "scripts/spin.decay",
  "script": "Spin",
  "properties": { "turns_per_second": 0.25 },
  "enabled": true
}
```

`source` is a logical `AssetId`, resolved against the scene's directory like any
other asset. `script` names which container in that file drives this entity, so
one file may hold a library of behaviours rather than being one behaviour.

`properties` are the authored values for the container's `@export` fields. They
are in the scene rather than the script because that is exactly the distinction:
the script says a speed exists and what it defaults to, the scene says what
*this* entity's speed is. This is also the capability that justified a typed
language over an embedded dynamic one — a property panel needs a declared,
named, typed field it can draw without executing anything.

A property is **refused rather than ignored** in every failing case: a field the
script does not declare, a field that is not `@export`, or a value Decay has no
type for. An authored number that silently goes nowhere is precisely the failure
this component exists to make visible.

`enabled` defaults to true. A disabled script is still authored, still
inspectable, and still saved — it simply does not tick, which is what an author
wants while narrowing down which script is misbehaving.

## What a script can reach

Everything, in one table. Decay knows paths, not engine concepts:
`this.transform.position.x` arrives at the runtime as four strings the IR never
interprets, and `WorldHost` is the only place that gives them a meaning.

| Path | Read | Write |
| --- | --- | --- |
| `this.transform.position.{x,y,z}` | yes | yes |
| `this.transform.scale.{x,y,z}` | yes | yes |
| `this.transform.rotation_z` | yes | yes |

Functions: `abs`, `sqrt`, `sin`, `cos`, `min`, `max`. That is the entire
standard library. Decay has no modules and no imports, so each is a bare global
name, and every one added is a name a script can no longer use for its own.

Two absences are deliberate. There is **no full 3D rotation**: a gameplay script
should not be asked to assemble a quaternion by hand, and offering a third of a
3D rotation API is worse than offering none. There is **no input, no time, no
spawning, and no other entity** — those are the scripting host's job, and the
host does not exist yet.

The **Z lock is honoured**. A script is a write path like any other, and one
that could ignore the lock would be the hole that makes the lock worthless.

## Lifecycle

`start()` runs once, after the authored properties are applied and before the
first update. `update(dt)` runs once a frame with the frame's delta. Both are
optional; a container declaring neither simply does nothing.

Both signatures are exact — `start` takes no parameters, `update` takes one.
One way of saying a thing beats two that can disagree, and an arity mismatch
reports itself clearly.

## Where the state lives

The same split `docs/scene-extraction.md` describes for sprite animation, for
the same reason. A clip and its timing are authored, so they are in the scene;
the frame it has reached is not, so it is not.

A script's fields drift as it runs. `Scripts` holds the live instances beside
the world rather than in it, so watching a scene play is never an unsaved change
to the file it came from.

## Failure is per script

`Scripts::advance` returns every failure rather than stopping at the first. One
script must not be able to silence the others: in the editor that would mean a
typo in one object freezing every other, and the author looking for the wrong
bug entirely.

A script that failed this frame **keeps its instance**. A runtime error is not a
reason to discard the state an author is trying to inspect, and restarting it
would hide the failure behind a fresh `start` sixty times a second.

## No I/O, anywhere in the crate

`sindri-decay` never opens a `.decay` file. It has no more business doing so
than `sindri-core` does, and staying out of it is what lets every test in the
crate run with no filesystem and no browser.

Sources arrive through `ScriptSources`, filled by whoever owns the asset
pipeline — exactly as textures arrive at `sindri-scene` through
`TextureBindings` rather than being loaded there. In the editor that is
`editor/src/scripts.rs`, which drives `AssetLoader<TextAssetDecoder>` and gets
hot reload out of the same `AssetWatch` the textures use: a changed file is
fetched again, and `Scripts` recompiles when the text it holds stops matching.

## Play, and putting the world back

Scripts write to the world, which sprite animation never did. So the editor
snapshots the world when Play is pressed and restores it on Stop.

The snapshot rather than the authored document, deliberately: a scene edited and
then played must come back to the edit, or pressing Play would quietly discard
unsaved work. Undo history is left alone — a script moving something is not an
action the author took, so it was never on the history, and putting the world
back does not change what undo means.

## Known gaps

- Member types are `Unknown` to the analyzer, so a misspelled component field is
  a runtime `UnknownPath` rather than a compile error. Typed host members are
  the next thing that would change that.
- Decay's only numeric type is spelled `f32` and holds an `f64`; `WorldHost` is
  the one place the two meet and the one place that narrows.
- The script instance is created on first sight and lost when the world is
  reloaded. There is no state migration across a hot reload of the source: a
  changed file recompiles, and the running instance keeps its fields.
