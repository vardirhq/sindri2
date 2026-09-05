# What Sindri can do

An inventory of what exists, what is drawn but does nothing, and what is
missing. `ROADMAP.md` says what is planned and in what order; this says what is
true today.

It exists because the two are easy to confuse. A roadmap full of ticked boxes
reads like a capable engine, and an editor full of buttons looks like a working
tool. Twice now something has been described as working because it was written
down rather than because it ran.

## Keeping this current

**This file is updated in the same commit as the change it describes**, not
afterwards and not in a follow-up. A list that lags is worse than no list,
because it is trusted and wrong — which is the exact failure it exists to
prevent.

Update it when a change:

- adds a capability, to either the engine or the editor
- **wires up a control that was drawn but inert** — move it out of "Drawn, but
  does nothing" rather than leaving it in both places
- removes a capability, or deletes dead chrome
- turns something in "Not yet" into something that is
- discovers that an entry here is wrong

That last one matters as much as the rest. Every entry should be something
someone ran, not something they read in a roadmap or inferred from a type
signature. If you cannot demonstrate an entry, correct it or delete it.

---

## Engine

### Lifecycle and time

Legal state transitions are enforced rather than assumed: an engine can be
created, started, paused, resumed, and stopped, and an illegal transition is a
`LifecycleError` rather than a silently ignored call. Frame time is capped so a
long stall cannot produce one enormous step, and fixed-step accumulation runs
simulation at a steady rate with a bound on catch-up steps that prevents a
spiral of death. Time scale is rational rather than floating-point, so slow
motion does not accumulate drift. `ManualClock` drives the whole loop from a
test with no window and no sleeping.

### World and entities

Entities are generation-checked slot handles, so a handle to a despawned entity
is detected rather than silently addressing whatever took its place. Spawning,
access, recursive destruction, and slot reuse are all safe. Hierarchies support
reparenting with cycle prevention, and `World::check_set_parent` answers whether
a move is legal without making it. Each entity carries a name, a parent,
children, an optional `Transform3D`, arbitrary JSON components, and an
editor-only section the runtime never interprets. There is one transform, with
2D-shaped accessors that read and write X and Y and cannot express a change to
Z, and a Z lock a transform can declare that the command layer refuses to write
past: a 2D entity is one that keeps to a plane, not one with a different
transform type — see `docs/2d-model.md`.

Entity storage was measured at 1k, 10k, and 100k entities before considering an
archetype ECS; `docs/entity-scaling.md` records why one is not warranted.

A **prefab** is an authored reusable entity definition: a single-root scene
fragment in the same document shape a scene uses, sharing its entity rules,
component payloads, versioning, and canonical serialization. `World::spawn_prefab`
creates the whole subtree or none of it, and answers with the root, every entity
made, and which authored identity each became. Instances carry no `source_id`,
because a prefab's identities name entities inside the prefab and two instances
would collide on every one. `docs/prefabs.md` is the contract.

### Rendering

A frame goes through extraction, preparation, and rendering. Preparation
validates and orders passes deterministically by stage, then layer, then
insertion order; stage order is `Opaque3d`, `Transparent2d`, `Overlay`.

World and UI procedural shapes share one instanced renderer. Rectangles,
ellipses, grids, and regular polygons are still evaluated in the shader, but a
polygon may now also carry up to eight authored 2D vertices. Those vertices are
packed into the same instance payload and measured against the actual authored
edges in WGSL, so an irregular ship hull remains crisp at any zoom without
becoming a texture. Existing regular polygons keep their old path when no
vertices are authored.

The eight-point limit is deliberate rather than arbitrary. The renderer keeps
all per-instance geometry inside conservative WebGPU vertex-attribute limits so
the same project runs on native and browser targets. The scene component stores
the points with the rest of the shape geometry; the generic editor can still
inspect the shape, but there is not yet a dedicated point-handle editor. Decay
can author the current script entity's points through the bounded
`World.set_shape_point(index, x, y)` call. Orbital Last Stand uses that exact
path for the reference Strider's six-point hull, proving the capability in a
game rather than only in a renderer test.

What can actually be drawn also includes textured sprites with tint, anchor,
and layer, tilemaps, text, particles, a coloured cube, and textured 3D cubes
with depth testing. Procedural shape strokes, sweeps, dashes, fill/stroke alpha,
blend mode, and runtime side-count changes remain composable with authored
polygon points.

A sliced image says how it is cut once: sheet sidecars name cells, fragments
select them, and animation refers to those names rather than copying UV layout
into every component. World sprites batch by texture/layer/space and UI sprites
use the same asset binding rules. Offscreen rendering produces deterministic
PNGs used by CI, and the browser host runs the same WebGPU shape shader as native.

### Scripting

`sindri-decay` binds the Decay language to engine state without teaching the
language about entities. Scripts can read and write transforms, sprite and
shape presentation, query tagged entities, spawn prefabs, react to physics,
play audio, use seeded randomness, write saves, and drive UI. The host surface is
checked by the semantic environment before runtime.

The numeric host includes `exp()` alongside `sqrt`, `sin`, `cos`, `atan2`,
`min`, `max`, and `abs`. `exp()` exists because frame-rate-independent smoothing
is a gameplay concern, not a rendering trick: Orbital Last Stand uses
`1 - exp(-12 * dt)` for the reference Strider's heading response.

`World.set_shape_point(index, x, y)` is the narrow authored-polygon bridge. It
writes one of eight possible points on the current script entity's
`sindri.shape`, refuses non-integer or out-of-range indices, and errors if the
entity has no shape. It is intentionally not a general JSON-array mutation API.
That keeps the script surface typed and bounded while still letting gameplay
define exact silhouettes when a regular polygon is not enough.

### Input

Keyboard, mouse, and touch input arrive as platform-independent events and
accumulate into an `InputState` that answers held, pressed-this-frame, and
released-this-frame for keys and mouse buttons, plus pointer position, pointer
delta, scroll delta, and window focus. `axis(negative, positive)` returns -1, 0,
or 1 and gives zero when both are held, so opposed movement keys cannot resolve
by event order.

Touch is held beside the mouse rather than folded into it, because they are
different facts: a mouse has one position and is always somewhere, while fingers
arrive and leave and there may be several. Fingers are bounded at ten, ordered
by the id their host gave them so one keeps its place while it stays down, and
let go of when a window loses focus.

### Audio

Audio is a platform service rather than simulation state. Encoded WAV, Ogg, and
MP3 assets are identified and validated by the asset layer, then registered with
an `AudioBackend`. Native hosts use Rodio/CPAL, browser hosts use media elements
with an explicit user-interaction unlock, and headless tests use a silent backend
that records every request without needing a sound device. Decay emits typed
playback intent and Gather exercises the path end to end.

### Physics

`sindri-physics` is the masked physics boundary. Rapier2D and Rapier3D are
private implementation dependencies; public code speaks only in Sindri body,
collider, shape, layer, pose, error, and event types. The exercised runtime is
currently 2D, with authored bodies/colliders, sensor and collision events,
velocity/impulse control, and fixed-step synchronization back into scene
transforms.

### Effects, saves, randomness, UI, grids, assets, hosts

Those existing capabilities remain unchanged by the authored-polygon slice:
pooled fleck effects, flat versioned saves, deterministic PCG randomness,
responsive screen UI, orthogonal/isometric grid navigation and pathfinding,
manifest-backed project assets, editor Play, native desktop, browser/WASM, and
headless test hosts all continue through the same public boundaries documented
in their subsystem files and in `docs/feature-integration-matrix.md`.

---

## The editor

The editor renders the real runtime frame through eframe's shared WGPU device.
It can create and inspect the existing `sindri.shape` component and edit its
ordinary scalar fields through the generic component inspector. Authored
irregular polygon points are now valid scene data and render correctly in Scene
and Game views, but there is no dedicated point list or viewport vertex-handle
workflow yet. The feature is therefore runtime-ready and script-proven while
editor authoring remains partial, explicitly tracked in the integration matrix.

---

## What is not yet

This file is intentionally not a promise list. The relevant gaps exposed by the
current work are: dedicated editor authoring for irregular polygon vertices,
more general vector/path tooling only if a real game proves eight points too
small, and the remaining Orbital Last Stand visual-parity layers such as player
ship details, backdrop motion, combat presentation, HUD reconstruction, and
capture-based comparison.
