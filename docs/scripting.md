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
| `this.ui_image.tint.{r,g,b,a}` | `f32` | yes | yes |
| `this.ui_image.layer` | `f32` | yes | yes |
| `this.shape.fill.{r,g,b,a}` | `f32` | yes | yes |
| `this.shape.stroke.{r,g,b,a}` | `f32` | yes | yes |
| `this.shape.count` | `f32` | yes | yes |
| `this.shape.stroke_width` | `f32` | yes | yes |
| `this.shape.sweep_start` | `f32` | yes | yes |
| `this.shape.sweep_turns` | `f32` | yes | yes |
| `this.shape.dashes` | `f32` | yes | yes |
| `this.shape.layer` | `f32` | yes | yes |

`sprite` is the thing in the world and `ui_image` is the thing on the viewport:
two components, so two paths. A script says which it means rather than writing
a tint that lands wherever the entity happened to be drawn — and an entity is
only ever one of the two, so on any given entity exactly one of these paths has
anything behind it.

`shape` is the world-space drawn shape, and it carries more than a tint because
there is more to drive. A sprite has one colour; a shape has a fill and a
stroke, and they animate separately — an enemy that flashes its outline without
lighting up its interior is one write. `count` changes a polygon's authored side
count at runtime, so one reusable shape can become a triangle, quad, hexagon, or
other regular polygon without swapping assets. `sweep_turns` is the reason the
group is worth having at all: a cooldown ring, a charge meter and a boss's
health arc are each a single float the script already holds, where drawing the
same thing from a sprite would need a frame of art per step. `stroke_width` is
how something pulses without changing size, and `dashes` is how a marker reads
as scanning.

An authored polygon may also carry up to eight explicit 2D vertices. Decay does
not expose those as a nested array path; it uses a bounded call instead:

```rust
World.set_shape_point(0, 0.0, 1.0);
World.set_shape_point(1, 0.8, -0.6);
World.set_shape_point(2, -0.8, -0.6);
```

`World.set_shape_point(index, x, y)` writes one vertex on the script entity's
`sindri.shape`. `index` must be a whole number from 0 through 7. The bound is an
engine/rendering contract, not an arbitrary Decay limit: the shape renderer packs
the authored vertices into a fixed per-instance payload that stays inside the
conservative WebGPU vertex-attribute budget. A call on an entity without a
`sindri.shape` is an error rather than a no-op.

The math host also includes `exp(x)` in addition to the existing trigonometric
and scalar helpers. It is used by frame-rate-independent interpolation such as
`1 - exp(-speed * dt)`, which Orbital Last Stand uses for the reference Strider
heading response.

Either path reaches into the entity's stored payload — `sindri.sprite` or
`sindri.ui.image` — rather than through the typed view, because a component is a
`Deserialize`-only view over a payload and the payload is what gets written
back: going through the view would mean rebuilding and reserializing it, which
is how a field the view does not know about gets dropped. A number written where
the payload held an integer is rounded back to one, so touching a layer does not
change a scene byte for byte.

An entity with no sprite is not an error at compile time — the surface says a
script *may* reach one, not that every entity has one — and a write says so
plainly at runtime.

### Other entities

A script can hold a reference to an entity, which is the thing that lets it say
anything about a world beyond itself.

`this.entity` is a script's reference to the entity it runs on. It reads and
cannot be written — a script gets to name another entity, not to reassign which
one it is running on — and through it every path below is the same path the
table above lists, reaching the same numbers.

| Path | Type | Read | Write |
| --- | --- | --- | --- |
| `this.entity.transform.position.{x,y,z}` | `f32` | yes | yes |
| `this.entity.transform.scale.{x,y,z}` | `f32` | yes | yes |
| `this.entity.transform.rotation_z` | `f32` | yes | yes |
| `this.entity.sprite.tint.{r,g,b,a}` | `f32` | yes | yes |
| `this.entity.sprite.layer` | `f32` | yes | yes |
| `this.entity.ui_image.tint.{r,g,b,a}` | `f32` | yes | yes |
| `this.entity.ui_image.layer` | `f32` | yes | yes |
| `this.entity.shape.fill.{r,g,b,a}` | `f32` | yes | yes |
| `this.entity.shape.stroke.{r,g,b,a}` | `f32` | yes | yes |
| `this.entity.shape.count` | `f32` | yes | yes |
| `this.entity.shape.stroke_width` | `f32` | yes | yes |
| `this.entity.shape.sweep_start` | `f32` | yes | yes |
| `this.entity.shape.sweep_turns` | `f32` | yes | yes |
| `this.entity.shape.dashes` | `f32` | yes | yes |
| `this.entity.shape.layer` | `f32` | yes | yes |

| Call | Returns |
| --- | --- |
| `World.find(name)` | `Entity`, or `null` |
| `World.exists(entity)` | `bool` |
| `World.despawn(entity)` | nothing |
| `World.spawn(prefab)` | `Entity` |
| `World.set_parent(entity, parent)` | nothing |
| `World.set_shape_point(index, x, y)` | nothing |
| `World.set_property(entity, name, value)` | nothing |
| `World.property_number(entity, name, fallback)` | `f32` |
| `World.with_tag(tag)` | `Array<Entity>` |
| `World.has_tag(entity, tag)` | `bool` |
| `World.set_active(entity, on)` | nothing |
| `World.is_active(entity)` | `bool` |

`World.set_shape_point` is deliberately different from the other `World.*`
calls: it operates on the current script entity rather than taking an entity
argument. This keeps the bounded authored-vertex surface narrow instead of
turning arbitrary component-array mutation into a general host feature.

An `Entity` is opaque. A script can hold one in a `var` or a field, pass it,
compare it, and reach through it to the same transform and sprite paths it
reaches on itself; it cannot build one, do arithmetic on one, or read the number
inside. The engine packs a runtime handle — a slot and a generation — into that
number, and **it is never serialized**: an `@export` of an entity is not
authorable, and the inspector shows one as empty rather than as a number nobody
can act on. See `docs/FEASIBILITY.md` on why runtime handles are not scene IDs.

A script names itself with `this.entity`, so it can pass itself to something
that takes an entity — or leave itself on the board for another script to pick
up. Reaching through it is the same as reaching directly, and that redundancy is
the point: it makes the two forms one rule rather than two.

**A reference outlives what it names**, which is exactly what generation checking
is for. `World.exists` is how a script holding one across frames asks before
using it; reaching through a stale reference is an error naming the path, not a
silent no-op, because a script holding a dead handle is a bug in the script.
Reaching through `null` is likewise an error. `World.despawn(null)` is not,
because `World.despawn(World.find("gone"))` is a reasonable thing to write.

`World.find` matches on the name a scene gave an entity — what an author typed
and can see in the hierarchy — and takes the first match. Two entities with one
name is an authoring mistake for the editor to catch, not something for this to
invent a rule about. A runtime-spawned entity has no scene ID, which is the
other reason the lookup is by name.

**Despawning is not undoable**, and no write a script makes is: a script's
transform writes produce no undo entry either, and play mode restores the world
from the snapshot it took when Play was pressed. `ROADMAP.md` keeps routing this
through `WorldCommand` as an open item rather than pretending it is done.

### Making one

`World.spawn` creates the entities a prefab describes and answers with its root.
A prefab is an authored reusable definition — one root and everything under it,
in the same document shape a scene uses. `docs/prefabs.md` is what one is.

**It takes a `Prefab`, not text.** A `Prefab` is opaque, like an `Entity`: a
script cannot build one, and the only way to hold one is for the scene to have
authored it into an `@export` field.

```rust
script Spawner {
    @export let bullet: Prefab;
    @export let speed: f32 = 400.0;

    fn update(dt: f32) {
        let shot = World.spawn(this.bullet);
        shot.transform.position.x = this.transform.position.x;
        World.set_property(shot, "speed", this.speed);
    }
}
```

That is a deliberate restriction and it buys four things a string literal could
not: the editor draws an asset picker for the field, the reference resolves
against the project like every other asset, the document loads before the frame
that needs it, and a reference naming nothing is refused when the project is
read rather than on the frame it is spawned. A prefab named in a script's source
would be invisible to all of it.

An exported prefab field the scene never filled in is `null`, and spawning it is
an error saying so — not a spawn that quietly makes nothing.

**Overrides are ordinary writes.** The call answers with a reference, and
everything above is reachable through it: position, scale, rotation, tint,
layer. `World.set_parent` puts it under something, or at the root when given
`null`. Nothing here takes a bag of JSON.

**A starting value for a script is `World.set_property`.** It is the one thing
the paths above cannot do: a script's own fields are not on the surface, because
the analyzer cannot know which container another entity runs. So a per-instance
starting value is set the way the *scene* sets one — by authoring the property
the instance will be built from — and it is refused, named, in exactly the cases
the scene's own properties are: a field the script does not declare, a field
that is not `@export`, a value Decay has no type for.

It is also refused once that entity's script is running, because properties are
applied when an instance is built and a later write would land in the payload
and change nothing a script could see.

`World.property_number` reads that authored starting value back. It does not
reach into another running script's private mutable fields: it reads the
property the scene or spawner supplied, which makes an immutable per-instance
fact such as a projectile's damage available to the entity it hits. The
fallback is required and is returned when the entity has no script, no property
by that name, or a property that is not numeric.

**A spawned script starts in the same pass.** A bullet created during an update
moves during that update rather than standing still for a frame. It cannot start
during the call that created it — building an instance runs the container's
field initializers, which is Decay code, and the world is already lent to the
call in progress — so the pass finishes and then starts what it made. A script
started that way may spawn in turn, and those rounds are bounded: a cascade that
does not settle after eight is reported rather than taking the frame with it.

**Spawning is bounded.** One pass may create 4096 entities. Decay's operation
budget already stops a loop that never ends, but it stops it after a million
instructions — long after a spawn loop with a mistaken bound has put a hundred
thousand entities in the world and taken the editor with it. The limit is the
same protection stated in the units the mistake is made in.

Spawned entities carry no stable scene ID. A prefab's identities name entities
*inside the prefab*: two instances carrying them would collide on every one, and
a scene saved with the collision would refuse to load.

### Groups of entities

A game that spawns hundreds of enemies cannot hold a reference to each of them.
`World.with_tag` is how it asks for them all at once, and `Array<Entity>` — see
`decay/LANGUAGE.md` — is what it gets back.

`World.has_tag(entity, tag)` asks the corresponding question about one active
entity without walking the world. Collision code should use it to distinguish a
projectile from the other half of a contact; querying the whole projectile group
once per collision would turn a dense combat frame into a quadratic walk.

```rust
script Sweep {
    @export let enemy: String = "enemy";

    fn update(dt: f32) {
        for enemy in World.with_tag(this.enemy) {
            enemy.transform.position.y -= 40.0 * dt;
        }
    }
}
```

**By tag, not by name.** `World.find` matches the name a scene gave *one*
entity. A game whose enemies are "Scout 41" through "Scout 300" has three
hundred authored names and still no way to say "the enemies". A tag says what
an entity *is*, and `sindri.tags` is where one is authored.

**Active only**, which is the filter every other walk of the world uses: an
entity switched off — or whose parent is — takes no part in rendering,
stepping, scripting or picking, and a query that answered with one would be the
odd one out.

**In world order**, which is deterministic and is the same order twice for the
same world. It is not an order a game should depend on for *meaning* — it is
allocation order, and reusing a freed slot changes it — but a run is
reproducible and a test is stable.

**It is a snapshot of handles.** The collection does not change when the world
does. An entity despawned while a script is part way through walking one leaves
a handle that no longer names anything, which is exactly the case `World.exists`
is for; reaching through it is an error naming the path, as it is anywhere else.
That is the safe behaviour rather than a silent skip, because a script walking a
group it is also destroying should say which it meant.

**Bounded at 8192.** A query walks the world, so its cost is the world's size
whatever the answer; the bound is on what a script then holds and walks. Past
it the call is refused rather than truncated: a query that quietly returned the
first eight thousand enemies would be a game that quietly stopped hitting some
of them.

**Asking is not free.** One call walks every entity in the world. A script that
asks once per frame is fine; a script that asks once per bullet per frame is
quadratic, and the operation budget will eventually say so in a way that names
the script rather than the pattern. Ask once and hold the answer for the frame.

### Flecks that are not entities

| Call | Returns |
| --- | --- |
| `Effects.burst(entity)` | `f32` — how many flecks were made |
| `Effects.burst_at(entity, x, y)` | `f32` |
| `Effects.live()` | `f32` |

```rust
script Bullet {
    fn update(dt: f32) {
        for hit in Physics.sensor_entered() {
            Effects.burst(this.entity);
            World.despawn(hit);
            World.despawn(this.entity);
        }
    }
}
```

**A fleck is not an entity.** It has no identity a script can hold, no
components, no place in the hierarchy, and nothing can collide with it. That is
the entire trade, and it was made on a measurement rather than a hunch:
`docs/effect-scaling.md` puts eight thousand flecks-as-entities at 5.25 ms a
frame — a third of a 60 Hz budget — against 0.018 ms for the same population as
plain values. Over half the entity cost was re-reading each one's component
payload, every entity, every frame.

**What a burst looks like is authored, not argued.** How many, how fast, how big,
what colour and how long are a designer's numbers, and a call that named all of
them would be one nobody could read. An entity carries `sindri.effect.burst`, and
a script fires it:

```rust
// The scene: count 24, speed 6, spread 0.6, lifetime 0.4, tint orange.
Effects.burst_at(this.explosion, enemy.transform.position.x,
                 enemy.transform.position.y);
```

`burst` throws at the entity's own position, which is read at the moment of the
call — so the usual shape, firing a burst and then despawning what fired it,
works. `burst_at` throws somewhere else, which is what an explosion where
something *used to be* needs.

**Flecks draw their own random directions from their own stream**, never the
run's. A fleck drawn from the gameplay stream would shift every number after it,
so turning an explosion up would change which enemies spawned — and a seeded run
has to mean the same run whatever it looked like.

**The pool is bounded and says when it overflows.** Past its capacity the oldest
fleck makes way for the newest, because the newest action is the one someone is
looking at. `burst` answers with how many flecks it actually made, so a game that
wants to turn itself down can see that it should.

**A host running no effects refuses these**, rather than accepting flecks nobody
will ever see.

### What a game remembers

| Call | Returns |
| --- | --- |
| `Save.number(key, fallback)` | `f32` |
| `Save.set_number(key, value)` | nothing |
| `Save.flag(key, fallback)` | `bool` |
| `Save.set_flag(key, value)` | nothing |
| `Save.has(key)` | `bool` |
| `Save.clear()` | nothing |
| `Save.is_new()` | `bool` |
| `Save.is_damaged()` | `bool` |
| `Save.is_from_newer()` | `bool` |

```rust
script Progress {
    fn start() {
        this.best = Save.number("best_wave", 0.0);
    }

    fn on_run_ended(wave: f32) {
        if wave > this.best {
            Save.set_number("best_wave", wave);
        }
    }
}
```

**A save is a flat key/value document, not a tree.** Decay holds numbers, truths
and text and nothing else, so a structure a script could not build is a structure
nothing could write. A game that wants `settings.volume` and `progress.best_wave`
writes those keys, and the file stays something a person can read and repair.

**A fallback rather than an optional**, because every caller has one — a starting
score, a default volume — and a save is mostly read on the run where nothing has
been stored yet. Reading a key that holds the wrong kind of value gives the
fallback too: a script asking the wrong question should not get a
plausible-looking answer that lets the mistake run.

**Nothing here writes to a disk.** How often someone's storage is touched is a
decision about their machine, and a script asking for a write every frame would
make that decision badly on everyone's behalf. Writes go to a store in memory;
the host puts it somewhere, on its own schedule and before it stops. Writing the
same value again is not a change, so a game that stores its volume every frame
does not keep a disk busy.

**Three ways a save can be absent, and they are not the same.**

- `is_new` — nothing has been stored. A first run.
- `is_damaged` — something was stored and could not be read. Worth telling
  someone about *before* their progress is written over, which is the whole
  reason it is separate from the first.
- `is_from_newer` — what is stored was written by a format this build does not
  know. It is left alone rather than guessed at and rewritten.
