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
lighting up its interior is one write. `sweep_turns` is the reason the group is
worth having at all: a cooldown ring, a charge meter and a boss's health arc are
each a single float the script already holds, where drawing the same thing from
a sprite would need a frame of art per step. `stroke_width` is how something
pulses without changing size, and `dashes` is how a marker reads as scanning.

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
| `World.set_property(entity, name, value)` | nothing |
| `World.property_number(entity, name, fallback)` | `f32` |
| `World.with_tag(tag)` | `Array<Entity>` |
| `World.has_tag(entity, tag)` | `bool` |
| `World.set_active(entity, on)` | nothing |
| `World.is_active(entity)` | `bool` |

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
- `is_from_newer` — a newer build wrote it. Its values are not loaded, because a
  reader that guessed at a format it does not know would corrupt the newer save
  the moment it wrote back.

```rust
fn start() {
    if Save.is_damaged() {
        Ui.set_text(this.notice, "Your saved progress could not be read.");
    }
}
```

**A value that is not a number is refused**, because a NaN written to a save
comes back next run and poisons whatever reads it, long after the frame that
produced it has gone.

**Editor preferences are not game saves** and never mix. A save belongs to the
game — it is the player's, it ships with the build, and it round-trips
identically in a browser and on a desktop. Editor state belongs to whoever is
running the editor and never leaves their machine.

**There is no `set_text`.** The reference game's saved state is settings,
statistics, unlocks, currency and run history — numbers and truths. A string a
script can neither build nor compare would be nearly inert, so the store carries
text and the surface does not expose it until something needs it.

### Numbers a run can be replayed from

| Call | Returns |
| --- | --- |
| `Random.value()` | `f32` in `[0, 1)` |
| `Random.range(min, max)` | `f32` in `[min, max)` |
| `Random.int(min, max)` | a whole `f32`, both ends included |
| `Random.pick(group)` | `Entity` |
| `Random.seed(value)` | nothing |

```rust
script Spawner {
    fn update(dt: f32) {
        if this.due(dt) {
            let enemy = World.spawn(this.enemy);
            enemy.transform.position.x = Random.range(-8.0, 8.0);
            enemy.transform.position.y = Random.range(-4.5, 4.5);
        }
    }
}
```

**The stream is the host's, and a seed completely determines it.** The same seed
and the same sequence of calls give the same numbers in the editor, in a native
build, and in a browser — which is what makes a run replayable from a number a
player can share.

What that does *not* promise is that adding a call somewhere leaves the rest
alone. There is one stream and everything shares it, so a number drawn early
shifts every number after it. That is the honest cost of a single stream, and it
is why a run's *seed* is worth storing while a frame's numbers are not.

**`int` includes both ends**, because "a number from 1 to 6" means six outcomes
to everyone who is not writing the loop themselves. It is drawn without modulo
bias: the naive remainder makes the first few values slightly more likely, which
nobody notices on a die and which becomes a drop table that feels wrong over a
long run.

**`pick` exists because Decay has no indexing.** Without it a script cannot
choose from a group at all, and choosing from a group — a target, a module
offer, a spawn point — is most of what a game wants randomness for:

```rust
let enemies = World.with_tag(this.enemy);
if enemies.len > 0.0 {
    let target = Random.pick(enemies);
}
```

That guard is required: picking from nothing is refused rather than answered
with an entity that is not there, which would only move the same problem one
call further away.

**A range that runs backwards is refused**, because a spawner that never spawns
anywhere is worth hearing about. So is `int(1.2, 1.8)`, which has no whole number
to give.

**The engine never asks the platform for entropy.** It has no way to be
genuinely random and deliberately does not pretend otherwise: a host that seeds
nothing gets a fixed stream, so a run is repeatable rather than arbitrary. A game
that wants a different run each time calls `Random.seed` with something it knows
— a counter it saved, the moment the person pressed Start. `Random.seed` restarts
the one stream every script shares, which is what a run seed is for and why it
belongs at the start of a run rather than in the middle of one.

**This is not a source of secrets.** Anyone who can see a handful of outputs can
work out the state and predict the rest, which is fine for waves and drops and
disqualifying for anything else.

### Screen elements

| Call | Returns |
| --- | --- |
| `Ui.set_text(entity, "words")` | nothing |
| `Ui.set_number(entity, value)` | nothing |
| `Ui.set_numbers(entity, first, second)` | nothing |
| `Ui.set_fill(entity, amount)` | nothing |
| `Ui.is_hovered(entity)` | `bool` |
| `Ui.is_pressed(entity)` | `bool` |
| `Ui.is_held(entity)` | `bool` |

**The scene owns the words and the script owns the numbers.** Decay has no
string concatenation, no interpolation and no formatting library — `+` is
numeric addition and `decay/LANGUAGE.md` says why. A script therefore cannot
build `"Score: 1200"`, and a HUD that cannot show a number is not a HUD.

So a `sindri.ui.text` component authors a **template**, and a script fills its
slots:

```rust
// The scene:  text = "Score: {}"
Ui.set_number(this.score_label, 1200.0);     // Score: 1200

// The scene:  text = "{}/{}"
Ui.set_numbers(this.health_label, 45.0, 100.0);   // 45/100
```

This is the better half of the trade rather than a consolation. The words stay
in the scene file, where they can be read, reviewed, and one day translated —
not assembled inside a script where none of that is possible.

In a template, `{}` prints as few decimals as the value needs, up to three
(`1200`, `1.5`); `{.2}` prints exactly two (`1.50`), for any count up to six;
and a doubled brace is a literal one, either way round. A lone `}` is simply
itself: text is content, and refusing to draw a label over a stray brace helps
nobody.

A slot no script has filled yet reads as `0`, so a scoreboard that has not been
written to shows a score of nothing. A value that is not a number prints `NaN`,
because a HUD saying so is telling the truth about a gameplay bug. Anything that
is not a slot — `{.x}`, `{9}` — is drawn exactly as written, so a designer who
typed it wrong sees it and fixes it rather than watching it vanish.

`set_text` replaces the template itself, for swapping one authored string for
another: a warning appearing, a label changing with the mode. It takes a
literal, because a literal is the only string a script has.

**`set_fill` is what makes a bar a bar.** A `sindri.ui.image` keeps its authored
rect and draws a fraction of it, from the edge the scene names — so the empty
part of the bar is where the full one was, rather than the bar closing towards
its middle:

```rust
Ui.set_fill(this.health_bar, hp / max_hp);
```

It clips the texture with the quad, so a segmented or lettered bar stays
correct instead of being squashed. A bar filled to zero draws nothing at all.
Which edge it empties towards is authored rather than set here: that is a
decision about how the bar reads, not a per-frame gameplay value.

### Buttons, screens, and who gets the click

A `sindri.ui.button` makes an element pressable. Its rect is the entity's own
transform — the same one that already decides where it draws — so a button on an
entity that draws something is immediately pressable, and a button on a bare
entity is a hit area with no art.

```rust
script StartButton {
    fn update(dt: f32) {
        if Ui.is_pressed(this.entity) {
            World.set_property(this.menu, "showing", false);
            World.set_property(this.game, "running", true);
        }
    }
}
```

**`is_pressed` is a click, not a press:** the pointer went down on this element
and came up on it. Sliding off before letting go is how a person changes their
mind, and it keeps working here for the same reason it does everywhere else.
`is_held` is the in-between state, for a button that should look pushed while
it is. Both are answers about *this* frame, computed before any script ran.

**A disabled entity is not hit-tested**, and neither is anything under a
disabled parent — `World.is_active` already governs a subtree. That is also all
a *screen* is: a menu, a pause overlay and a HUD are entities with children, and
showing one is `World.set_active(this.menu, false)`. There is no screen stack,
because the engine already had the mechanism and a second one would be a second
answer to the same question.

The switch is written on the entity named and not down through its children,
which is what makes switching a screen back on restore the entries that were
off on their own account — a locked menu row stays locked. `is_active` answers
the same question the engine asks before drawing or stepping anything: false
for an entity switched off, for anything under one, and for a handle the world
no longer holds.

**When elements overlap, the top one takes the click** — highest layer first,
which is the order the overlay draws in. A modal is a modal because it is on a
higher layer, not because it declared itself one.

**Nothing is silently taken away from gameplay.** An engine that withheld input
from "gameplay scripts" while a menu was up would have to know which scripts
those are, and a rule that guesses will guess wrong. So a gameplay script asks:

```rust
fn update(dt: f32) {
    if Pointer.just_pressed("Left") && !Pointer.over_ui {
        this.fire();
    }
}
```

That one line is why a click on a pause button does not also fire the gun
behind it.

### Rows and columns

A `sindri.ui.layout` on a parent places its children along an axis — `row` or
`column`, with a spacing in overlay units. Three buttons could be authored as
three offsets; what cannot be authored is what happens when one is switched off.
A hand-placed row leaves a hole, and everything below a hidden entry is in the
wrong place. A layout counts only the children that are actually there, so a
menu that loses an entry **closes up around its own middle**.

There is no scroll region. Nothing being built against this engine has a list
longer than a screen, and a scroll invented before something needs one would
have to decide about clipping, momentum, and where a drag stops being a press —
none of which has an answer yet.

### Screens that fit any screen, and text that fits with them

A font size is in those units too — two is the whole height of the screen — so
text scales with everything around it. It used to be pixels, which made it the
one number on a screen element that did not follow the screen: a HUD authored
on a desktop was unreadable on a phone, and a heading authored in the units the
rest of the element uses drew nothing at all, because an eighth of a pixel is a
positive number.

The overlay is authored in normalized units: two tall, centred on the origin,
running out to the aspect ratio either side. A corner-anchored element is in
that corner on a portrait phone and a wide desktop window alike, which is
responsive layout without a single breakpoint.

**How wide "either side" is depends on the screen**, and that is the part worth
designing for. A portrait phone gives about nine tenths of a unit across; a
desktop window gives three and a half. An element that fits the narrow one fits
both, which is why a layout is worth authoring against a phone first — and why
three cards side by side is a landscape idea that does not survive being turned
upright.

A **world** camera has the same question, and `sindri.camera` answers it with
`fit`. An orthographic camera framing `vertical_size` by `height` shows a fixed
amount vertically and whatever the width happens to be, so turning a wide window
tall takes the sides off the world. `"fit": "shorter"` makes the size a promise
instead — *this much world is visible whichever way the screen is turned* — so
an arena fills the height of a landscape window and the width of a portrait one
and is never cut off by either.

The one thing a designer cannot author around is the **safe area** — a notch, a
rounded corner, a home indicator — because it is not in the scene, it is in the
hardware the scene happens to be running on. A host reports it in pixels and
anchored elements move in from their own edge; a centred element stays centred,
because the screen does not shrink, the edges come in.

**Accessible labels are authored but not yet surfaced.** A button carries a
`label`, stored beside the thing it names in the file a designer edits. Nothing
reads it yet: there is no DOM to expose it to until a project can be exported to
the web, and inventing a second accessibility path before then would be building
the wrong one.

**An entity that is not the kind of element the call needs is named**, rather
than the call quietly doing nothing — a HUD that stops updating because a script
points at the wrong element is the failure that survives a play-test. A host
laying out no screen UI refuses `Ui.is_pressed` for the same reason: a menu whose
buttons never respond should be heard about on the first frame, not mistaken for
a person who has not clicked yet.

### Bodies, and what they touched

| Call | Returns |
| --- | --- |
| `Physics.velocity_x(entity)` | `f32` |
| `Physics.velocity_y(entity)` | `f32` |
| `Physics.set_velocity(entity, x, y)` | nothing |
| `Physics.apply_impulse(entity, x, y)` | nothing |
| `Physics.collision_started()` | `Array<Entity>` |
| `Physics.collision_stopped()` | `Array<Entity>` |
| `Physics.sensor_entered()` | `Array<Entity>` |
| `Physics.sensor_exited()` | `Array<Entity>` |

This is **Sindri physics, never Rapier**. `docs/physics.md` makes the backend a
private implementation detail, and a namespace that leaked its vocabulary would
make the backend unreplaceable one script at a time.

A body is authored, not created here: an entity carries `sindri.physics2d.collider`
and optionally `sindri.physics2d.rigid_body`, and `ScenePhysics2d` keeps the
simulation in step with what the scene says. A prefab carrying those components
spawns with them, which is how a bullet gets a body.

```rust
script Bullet {
    @export let speed: f32 = 400.0;

    fn start() {
        Physics.set_velocity(this.entity, this.speed, 0.0);
    }

    fn update(dt: f32) {
        for hit in Physics.sensor_entered() {
            World.despawn(hit);
            World.despawn(this.entity);
        }
    }
}
```

**The event calls take nothing and answer about the entity the script is on.**
An event is about a *pair*, and the pair a script cares about is the one it is
half of — so the answer names the other half. A manager script that wanted every
collision in the world would be asking a different question, and the surface does
not offer it yet.

They are **queries rather than callbacks**, because Decay now has a value that
can hold several entities and a lifecycle function would be a second way for the
host to enter a script. The order is the order the step reported, which is the
same order twice for the same simulation.

**Started, not touching.** `collision_started` answers with what began touching
during the last step, so a projectile that should hit each target once hits each
target once without keeping a list. That is the shape the reference game's
piercing bullets need, and it falls out of the event rather than being a feature.

**Despawning from an event is safe.** Removing either half — the thing that was
hit, or the thing that hit it — is an ordinary `World.despawn`, and the body
leaves the simulation before the next step. Walking the answer while despawning
from it is the case `World.exists` covers, exactly as for any other collection of
references.

**A host with no physics refuses these**, rather than reporting a velocity of
zero for a body that does not exist. A game whose bullets never move because
nothing is stepping should hear about it on the first frame.

**There is no ray cast, no overlap query, and no contact detail yet.**
`docs/physics.md` lists them as first-slice-adjacent, and none has a demonstrated
consumer; a query surface invented before something needs it is a shape chosen
by guesswork.

### Grid position

| Call | Returns |
| --- | --- |
| `Grid.position_x(entity, grid)` | `f32` |
| `Grid.position_y(entity, grid)` | `f32` |
| `Grid.place(entity, grid, x, y)` | nothing |
| `Grid.can_reach(mover, grid, target)` | `bool` |
| `Grid.step_toward(mover, grid, target)` | `bool` |

The second entity must carry a world-space `sindri.tilemap`. Its projection,
cell size, and complete world-XY transform define the coordinate space; there is no implicit
"first grid" and no second set of projection settings for scripts to disagree
with. The coordinates are continuous logical positions, not rounded cell
indices, so a character can move smoothly between cells while gameplay still
speaks in grid axes.

`Grid.place` writes X and Y together and preserves the positioned entity's Z.
Moving, rotating, or scaling the tilemap changes the world position produced by
the same logical coordinate. `position_x` and `position_y` perform the inverse,
including that map transform, so placing and reading round-trip. A tilted map
or one with zero XY scale is refused: this surface describes a grid on Sindri's
world XY plane, not an arbitrary plane in 3D.

The split X/Y reads are a consequence of Decay not having a structured vector
or grid-coordinate value yet. They are kept behind one namespace so that value
can replace the pair later without exposing tilemap storage to gameplay code.

### Audio

| Call | Returns |
| --- | --- |
| `Audio.play(clip, volume)` | nothing |
| `Audio.loop(clip, volume)` | nothing |
| `Audio.stop_all()` | nothing |
| `Audio.pause_all()` | nothing |
| `Audio.resume_all()` | nothing |

`clip` is a logical audio asset ID and `volume` is a finite normalized number
from 0 through 1. `play` is one-shot and `loop` repeats until stopped. Decay
only emits typed playback intent; it never owns or talks to an audio device.
The host drains those requests through the platform audio boundary, which keeps
headless tests silent and lets browser playback obey its user-interaction unlock
without teaching the language about either platform.

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

### Where the person is pointing

| Path | Type |
| --- | --- |
| `Pointer.x` | `f32` |
| `Pointer.y` | `f32` |
| `Pointer.overlay_x` | `f32` |
| `Pointer.overlay_y` | `f32` |
| `Pointer.inside` | `bool` |
| `Pointer.over_ui` | `bool` |

| Call | Returns |
| --- | --- |
| `Pointer.is_down(button)` | `bool` |
| `Pointer.just_pressed(button)` | `bool` |
| `Pointer.just_released(button)` | `bool` |

| Path | Type |
| --- | --- |
| `Touch.count` | `f32` |

| Call | Returns |
| --- | --- |
| `Touch.x(index)` | `f32` |
| `Touch.y(index)` | `f32` |

`Pointer` is **one namespace for the mouse and the finger**, and that is the
whole point of it: a game that aims at a point should not have to ask which
device the person is using, and a game written for a mouse then works on a phone
without a second code path.

```rust
script Aim {
    fn update(dt: f32) {
        if Pointer.inside && Pointer.is_down("Left") {
            this.transform.position.x = Pointer.x;
            this.transform.position.y = Pointer.y;
        }
    }
}
```

What each unified answer means when both a mouse and a finger are present:

- **Position** is the mouse when there is one, and the first finger otherwise. A
  machine with both is a machine someone is using the mouse on.
- **`is_down("Left")`** is the left mouse button *or* any finger. A tap and a
  click are the same line of gameplay code, which is the convention the web
  settled on. A finger is `Left` and nothing else — it is not a right-click.
- **`just_released("Left")`** for a finger means the *last* one left. A second
  finger lifting while one is still down is not the pointer coming up, any more
  than releasing the right mouse button releases the left.

Buttons are named `"Left"`, `"Middle"`, `"Right"`, matching case-insensitively
because the name is typed by a person. **A name nothing answers to is refused**,
exactly as a key name is: a control that silently does nothing cannot be
reproduced.

`Pointer.inside` is false when the mouse has left the window and nothing is
touching the screen. A position read while it is false is zero rather than an
error — the mouse leaving mid-frame is an ordinary thing, not a mistake in a
script — so a script that cares must ask `inside` *before* it believes a
position.

**Coordinates are viewport pixels with the origin at the top left of the
viewport**, which is the same thing on every host: the window on native and in
the browser, and the Game view's own rectangle in the editor. A script reading
`Pointer.x` gets the same meaning in editor Play as in the real build, which is
what makes playtesting in the editor worth anything.

Viewport pixels are *physical* ones — the same pixels the surface is configured
with and the same ones a screen element's hit rect is worked out in. This page
used to say logical, and it was wrong in a way that only showed up off the
desktop: on a device reporting three physical pixels per logical one, a position
converted to logical is a third of the way to where the person actually pressed,
so everything below the top third of a phone screen was unreachable. A scale
factor of 1.0 makes the two spellings identical, which is why a desktop, and
every test written on one, could not tell them apart.

The practical consequence for a game: `x` and `y` are bigger numbers on a dense
display than on a coarse one, for the same physical spot. A game comparing them
against authored constants wants `overlay_x` and `overlay_y` instead.

**`overlay_x` and `overlay_y` are the same point in the overlay's units** — two
tall, centred on the origin, running out to the aspect ratio either side. They
exist because `x` and `y` are viewport *pixels*, and how many pixels tall a
window is is not something a scene knows: a script could say where the pointer
was on the screen and not what it was pointing at.

They stop at the overlay rather than going on to the world, because going on
means a camera, and a script that could ask a camera anything would be a script
the renderer answers to. A scene authored its own camera and knows how much
world it frames, so the conversion is a multiplication the game owns:

```rust
// The camera frames `view_height` world units from top to bottom.
let world_x = Pointer.overlay_x * this.view_height * 0.5;
let world_y = Pointer.overlay_y * this.view_height * 0.5;
```

A host laying out no UI has no overlay, and these read zero for the same reason
a position read while the pointer is outside does.

`Touch` is the raw fingers, for a game that wants more than "where is the person
pointing" — a second finger, or a pinch. `Touch.count` is how many are down, and
`Touch.x`/`Touch.y` take which one, counting from zero, in a stable order: a
finger keeps its place while it stays down, so a drag cannot jump from one
finger to another. Asking for a finger that is not down is refused rather than
answered with zero, which would read as a finger in the corner of the screen.

### Steering with a thumb

| Path | Type |
| --- | --- |
| `Stick.x` | `f32` |
| `Stick.y` | `f32` |
| `Stick.held` | `bool` |
| `Stick.anchor_x` | `f32` |
| `Stick.anchor_y` | `f32` |

`Stick` is a joystick made out of whichever finger is steering. It anchors
where that finger landed, and `x`/`y` are how far it has been pulled from
there — from -1 to 1, in screen axes, never longer than 1 no matter how far
past the radius the thumb goes.

This is a different question from `Pointer`, which is why it is a different
namespace. `Pointer` says *where on the screen* the person is pointing, which is
absolute. `Stick` says *which way and how hard* they are pushing, which is
relative to wherever they put their thumb down. A game that steers wants the
second, and steering towards `Pointer` instead gives the ship a lunge every time
a thumb lands, no way to ask for gently-left, and a thumb parked over the part
of the screen the player is trying to watch.

`held` is not the same as a non-zero reading: a thumb resting inside the dead
zone reads centred but is still holding the stick, which is what a game drawing
the control needs to know. `anchor_x`/`anchor_y` are where the thumb landed, so
a game that wants to draw the ring draws it from the same numbers the input came
from rather than from a second guess at where the stick is.

**This page used to say there was no drag abstraction, deliberately** — that a
game's deadzone and radius are tuning, and that baking one game's numbers into
an engine is how an engine acquires a genre. The principle stands and the
conclusion did not. What is genre-specific is the *numbers*; what is not is the
*shape* — anchor where it lands, clamp at the radius, rescale out of the dead
zone, centre on release, and keep the press that started it so a second thumb
cannot snatch the steering mid-turn. That shape is the same in every game that
has ever had a stick, and "four lines that belong to the game" is four lines
each game gets subtly wrong: this engine's own game shipped steering that
followed the finger. So the shape is here and the numbers are authorable, which
is what the original objection was actually asking for.

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

The smallest thing that lets two scripts cooperate, and it predates references:
when a script could not name another entity at all, leaving a number under a
name was the only way for one to tell another anything.

References do not retire it. A board is still the right shape for a fact that
belongs to the game rather than to an entity — a score, a countdown, whether the
game is won — and for those, `World.find` every frame would be a lookup to answer
a question no entity owns. What references replace is the *workaround*: a
collectible no longer publishes its position as two numbers for a player to
compare against, it asks the player where it is.

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

`abs`, `sqrt`, `sin`, `cos`, `atan2`, `min`, `max`.
That is the entire standard library.
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

**No `vec2`.** It is a value type, and Decay does not have one yet, so a
position is two numbers wherever one appears — `Pointer.x` and `Pointer.y` for
the same reason `Grid.position_x` and `Grid.position_y` are a pair.

**No query by component type.** A script asks for a group by authored tag, and
that is the only way to ask. Spelling `sindri.sprite` in a script would put
engine internals in gameplay code, and it would make every enemy that happens to
have a sprite an enemy. If a game wants a group, it says so by tagging it.

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

- Despawning and other script writes are not routed through `WorldCommand` and
  therefore do not produce undo entries. Editor play mode restores its snapshot
  on Stop instead.
- Decay's only numeric type is spelled `f32` and holds an `f64`; `WorldHost` is
  the one place the two meet and the one place that narrows.
- The script instance is created on first sight and lost when the world is
  reloaded. There is no state migration across a hot reload of the source: a
  changed file recompiles, and the running instance keeps its fields.
