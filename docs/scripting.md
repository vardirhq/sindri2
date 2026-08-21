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

### The entity a script is on

| Path | Type | Read | Write |
| --- | --- | --- | --- |
| `this.transform.position.{x,y,z}` | `f32` | yes | yes |
| `this.transform.scale.{x,y,z}` | `f32` | yes | yes |
| `this.transform.rotation_z` | `f32` | yes | yes |
| `this.sprite.tint.{r,g,b,a}` | `f32` | yes | yes |
| `this.sprite.layer` | `f32` | yes | yes |

A sprite path reaches into the entity's stored `sindri.sprite` payload rather
than through the typed view, because a component is a `Deserialize`-only view
over a payload and the payload is what gets written back — going through the
view would mean rebuilding and reserializing it, which is how a field the view
does not know about gets dropped. A number written where the payload held an
integer is rounded back to one, so touching a layer does not change a scene byte
for byte.

An entity with no sprite is not an error at compile time — the surface says a
script *may* reach one, not that every entity has one — and a write says so
plainly at runtime.

### The keyboard

| Call | Returns |
| --- | --- |
| `Input.axis(negative, positive)` | `f32`, one of -1, 0, 1 |
| `Input.is_down(key)` | `bool` |
| `Input.just_pressed(key)` | `bool` |
| `Input.just_released(key)` | `bool` |

Keys are named physically, by where they are rather than what they type, so a
binding survives a change of layout: `"W"`, `"ArrowLeft"`, `"Space"`,
`"Digit1"`, `"ShiftLeft"`. Matching ignores case, because the name is typed by a
person. `sindri_platform::Key::ALL` is the list.

**A name nothing answers to is refused**, not read as never-held. A control that
silently does nothing is a bug report nobody can reproduce.

Holding two opposing keys gives an axis of zero, so opposed movement keys cannot
cancel into whichever the operating system reported last. A press is not a hold:
`just_pressed` is true for one frame however long the key stays down, and the
operating system's key repeat is not a second press.

### The frame

| Path | Type |
| --- | --- |
| `Time.delta` | `f32` |
| `Time.elapsed` | `f32` |

`delta` is what `update` receives as its argument, offered again so a function
that is not `update` can reach it. `elapsed` is per script instance rather than
per world: a script attached later has not been running as long.

### The board scripts share

| Call | Returns |
| --- | --- |
| `Game.get(name, fallback)` | `f32` |
| `Game.set(name, value)` | nothing |

The smallest thing that lets two scripts cooperate. Decay has no value that can
hold an entity, so a script cannot name another one — but it can leave a number
under a name, and another can read it. That is enough for a player to publish
where it is, a collectible to notice, and a score to be counted by nobody in
particular.

The fallback on `get` is **not optional**, because a note nobody has left yet is
the ordinary case on the first frame. A `get` that silently answered zero would
make a mistyped name read as a legitimate value, which is the failure this whole
surface is arranged to avoid.

The board is runtime state and goes when a run does — stopping and playing again
does not begin with the last game's score.

This is deliberately a stopgap, and its shape admits it: names are strings and
nothing checks them. Typed cross-entity access is the better answer and needs a
Decay value that can hold an entity. The board is here because it is small and
it unblocks a game, and a game is what tells us which of the bigger answers is
worth building.

### Saying something

`print(anything)` puts a line in the host's log, tagged with the entity that
said it. It takes any type because Decay has no conversions and `+` does not
concatenate, so a `print` that only took text could not report a value.

### Maths

`abs`, `sqrt`, `sin`, `cos`, `min`, `max`. That is the entire standard library.
Decay has no modules and no imports, so each is a bare global name, and every one
added is a name a script can no longer use for its own.

### Why this list can be trusted

**Every path above is typed, so a misspelling is a compile error.**
`this.transfrom` and `this.transform.position.w` are both refused with a line
number when the script compiles, rather than failing on the first frame with a
path name and no idea where it came from.

That holds because the tables above are not written twice. `surface.rs` is the
single description; the analyzer's `Environment` and `WorldHost`'s accessors are
both derived from it, and a test walks every path the analyzer would accept and
asserts the host answers it. A path accepted by one and not the other is the
worst failure available here — a clean compile followed by a runtime error — and
it cannot be shipped.

**And this document is checked against that description.**
`crates/sindri-decay/tests/documented_surface.rs` parses the tables above and
asserts they name exactly what a script can reach — no more, no less. A surface
that grew without the documentation growing with it fails the build, because a
list of what a script can do is believed, and one that is quietly wrong is worse
than none at all.

### What is deliberately absent

Three absences, each for a reason worth stating.

**No full 3D rotation**: a gameplay script should not be asked to assemble a
quaternion by hand, and offering a third of a 3D rotation API is worse than
offering none.

**No spawning, despawning, or naming another entity.** These wait on the
language rather than on the host: naming another entity means holding one, and
Decay has no value beyond numbers, booleans, strings, and null. An entity handle
is a value type, and adding one is a language decision — see
`decay/LANGUAGE.md`. The board above is what stands in until then.

**No mouse, and no `vec2`.** The same reason for `vec2`: it is a value type.
The mouse is simply not needed yet by anything, and the acceptance list did not
ask for it.

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

## Where this surface came from

Not invented. `docs/2d-inventory.md` read the legacy engine's
`examples/scripted_asteroids/scripts/player.lua` — real gameplay rather than an
imagined sample — and wrote down the whole surface it needed. That list is the
acceptance criteria, and it is now met: authored properties, lifecycle,
transform, sprite, input, and `print`.

Two items on it are met differently. `vec2(x, y)` and
`sprite:set_tint([f32; 4])` both pass a value around; Decay has no such value,
so a tint is four typed numbers — `this.sprite.tint.r` — in the same shape as
`position.x`. That is a better fit for a property panel than an array anyway.

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

- No spawning, despawning, or access to another entity, which wait on Decay
  having a value that can hold one.
- Decay's only numeric type is spelled `f32` and holds an `f64`; `WorldHost` is
  the one place the two meet and the one place that narrows.
- The script instance is created on first sight and lost when the world is
  reloaded. There is no state migration across a hot reload of the source: a
  changed file recompiles, and the running instance keeps its fields.
