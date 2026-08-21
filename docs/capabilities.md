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
transform type —
see `docs/2d-model.md`.

Entity storage was measured at 1k, 10k, and 100k entities before considering an
archetype ECS; `docs/entity-scaling.md` records why one is not warranted.

### Scenes

Scenes are versioned JSON documents with stable authored IDs kept separate from
runtime handles, so saving a loaded scene reproduces the authored identities
rather than inventing new ones. Loading validates duplicate IDs, missing
parents, and hierarchy cycles. Unknown component payloads are preserved by
default rather than dropped, which is what makes a scene written by a newer
editor survive an older runtime.

Serialization is canonical and a fixed point: entities and keys are sorted,
empty sections omitted, short scalar arrays kept on one line. Saving an unedited
scene reproduces the file byte for byte, which is what makes saving safe to
offer at all. Golden fixtures enforce it. A migration API exists before a second
format version does.

### Commands and undo

Every world edit can go through a `WorldCommand` that produces its own inverse:
set name, set the transform, set parent, set or remove a component.
`Transaction` is all-or-nothing, and `CommandHistory` gives bounded undo and
redo with labelled steps and merge runs, so a continuous drag collapses into one
undoable step rather than several hundred. The history also numbers the state
the world is in, so a tool that remembers the number it last saved can tell
whether the world and the file still agree — including after undoing back to it.

### Input

Keyboard and pointer input arrive as platform-independent events and accumulate
into an `InputState` that answers held, pressed-this-frame, and
released-this-frame for keys and mouse buttons, plus pointer position, pointer
delta, scroll delta, and window focus. `axis(negative, positive)` returns -1, 0, or 1 and gives zero
when both are held, so opposed movement keys cannot resolve by event order.

### Hosts and platforms

`Game` receives `start`, `fixed_update`, `update`, and `stop`, each with a
`FrameContext` carrying the world, the input state, and frame time, and each
able to fail in a way that reaches the host rather than being swallowed. The
same loop runs windowed on a desktop, in a browser through the same `winit`
event loop and canvas attachment, and headless in a test.

`DesktopApp` supplies the window, event loop, and input translation once, so an
application does not rewrite them. Target-specific code is confined to the
crates that must have it.

### GPU

Adapter, device, and queue negotiation is shared, with conservative
cross-target limits, resource labels, and errors that name what failed.
Swapchain acquisition has seven outcomes and one policy applied to all of them,
so hosts do not re-derive when to skip, reconfigure, or recreate — see
`docs/rendering-surface.md`. Surfaces must negotiate an sRGB format or fail
loudly. Headless adapter initialisation is proven in CI on software Vulkan.

### Rendering

A frame goes through extraction, preparation, and rendering. Preparation
validates and orders passes deterministically by stage, then layer, then
insertion order; stage order is `Opaque3d`, `Transparent2d`, `Overlay`.

What can actually be drawn: a triangle, a coloured cube, a textured cube with
depth testing, and textured sprites with tint, anchor, and layer, sorted back to
front by how far from the camera they are.
A sprite addresses part of a texture through a checked `UvRect`, so a sprite
sheet is expressible: `UvRect::cell` slices a grid, the rect rides on the
instance so every frame of one sheet stays in one draw call, and a GPU test
reads the pixels back to prove the shader honours it. That rect animates from
clips authored in the scene: `sindri.sprite_animation` names the
sheet's grid, the clips cut from it, and which one plays, and
`SpriteAnimations` holds where each sprite has got to. Playback is runtime
state, so watching an animation run does not rewrite the scene it came from.
What is missing is authoring: clips are typed into the scene file by hand,
because the editor has no sheet slicer or clip list yet.
A sprite is either screen-anchored, which is the default and cannot be occluded
by the world, or in the world, drawn through the world camera by its full
transform and hidden by opaque geometry in front of it. Sprites batch per space,
layer, and texture, with a measured five-to-one draw-call reduction. The frame
clears once before anything draws, so a scene of only sprites has a depth buffer
to test against. Perspective and orthographic cameras exist with tested projection
maths, and the zero-to-one depth range and Y-up convention are chosen in one
place rather than at each call site. Colour space is enforced by a shared
constant, and the offscreen capture verifies authored colours actually survive
to the pixels.

Offscreen rendering produces a deterministic PNG, which CI uploads.

### Scene to frame

Gameplay writes to the world and nothing tells the renderer. `SceneExtractor`
derives an ordered frame from whatever the world currently holds, using the
built-in `sindri.camera`, `sindri.mesh`, and `sindri.sprite` schemas. No scene
needs hand-written extraction code.

Textures bind by reference: a scene names `textures/badge.png`, the renderer
knows only a `TextureId`, and one place knows both. An unbound reference draws a
magenta checker and is reported by name rather than failing the frame or
silently reusing the last texture.

### Assets

`AssetId` is a validated relative logical ID, never a path or URL. Sources
resolve it against a root: the filesystem source canonicalises to catch symlink
escapes, and `UrlRoot` percent-encodes and normalises bases with rules exercised
on every target rather than only in a browser. The load queue is bounded,
rejects duplicates, and carries a generation token so a late completion cannot
overwrite a replacement. Textures and scene JSON decode through typed decoders.
Nothing pretends browser I/O is synchronous.

A `TextureId` is a generation-checked slot handle, so the renderer's texture
registry can release one texture and reuse its slot without a handle nobody
updated drawing whatever lands there next.

`AssetLoader` drives all of that in one place — request, enqueue, drain, decode,
apply — so a caller does not have to know the order. Requesting is idempotent, a
failure is reported once rather than retried forever, and `retain` releases what
is no longer wanted and says which IDs went, so a host can drop whatever it
built from them. GPU upload stays with the host, which is the only thing that
owns a device.

On native, `AssetWatch` notices when the file behind a loaded asset changes and
`AssetLoader::reload` loads it again, by polling modification time and length
rather than subscribing to filesystem events.

`AssetManifest` records what a project ships — each asset's length and the
SHA-256 of its stored bytes — as a versioned, ID-ordered file. A loader given one
holds arriving bytes to it, so a truncated response or a stale cache entry is an
error naming the asset rather than a picture from last week; an asset the
manifest does not list still loads. The editor picks one up from the directory a
scene lives in.

A corpus of deliberately awkward images — every PNG colour type, sixteen bits per
channel, an interlaced encoding, and a JPEG — is decoded and checked pixel by
pixel on both native and `wasm32-unknown-unknown`, so a texture cannot decode one
way in the editor and another in the browser.

---

## The editor

Reflects `main`. The editor renders the real runtime frame through eframe's
shared WGPU device — it is not a mock, and it does not create a second device.


An action that fails says so: a sticky notice from the last thing you did is
kept apart from the per-frame render result, which used to overwrite it within a
frame.

### Works

- Opens a scene from a command-line argument or **File → Open scene**, saves it
  back canonically, reloads from disk, and discards changes — including a scene
  carrying components it has never heard of, which it keeps through a save and
  lists in the inspector
- Shows the hierarchy from live runtime state, nested, searchable, with
  selection anywhere on a row — and with clearing the selection by clicking
  empty space or Escape
- Inspector edits of name and the transform, including the Z lock, which takes
  away the Z drag while it is on
- **Editing any component's fields in the inspector**, driven by the stored
  payload rather than by hand-written rows: numbers get drags, booleans get
  checkboxes, text gets a field, and a short numeric array gets a labelled row.
  A component the engine has never heard of is editable too, which is what the
  preserve policy promises. Every edit goes through `SetComponent`, so it
  undoes; every edit is checked against the component's own schema first, so one
  that would stop it decoding is refused and said aloud rather than written into
  a scene that then will not open. A field that decides nothing is not offered —
  a world-space sprite has no anchor row
- **Adding and removing components.** Add Component offers what the entity lacks
  and the registry can create, which excludes a type with no sensible blank
  rather than offering one the engine would reject. Both are undoable
- **A script's `@export` properties in the inspector**, drawn from what the
  script declared: the field's name, its type, and its default, without running
  anything. A field the scene has not set shows its default and says so, and
  setting one is what puts it in the scene — so a scene records an author's
  choices rather than a copy of every default. This is the capability that
  justified a statically typed language
- Reparenting through a **Parent** menu that offers only the moves the world
  would accept
- Undo and redo of every edit, with drag-merging so a slider drag is one step;
  Ctrl+Z, Ctrl+Shift+Z, and Ctrl+Y
- An unsaved marker that means the world and the file differ, so undoing back to
  what was saved reads as saved again
- A confirmation before anything that would throw unsaved work away — opening
  another scene, reloading from disk, discarding changes, and closing the window
  — each naming what it is about to do and offering to save instead
- Hot reload: saving a texture the open scene uses shows the edit within about a
  second, without restarting, and without blinking through the missing checker
- Loading the textures a scene names from the directory the scene lives in,
  through the real asset pipeline, so opening a project's scene shows that
  project's art rather than the two textures a demo crate supplied; each load or
  failure is named in the console, and the engine's procedural textures are
  generated rather than loaded
- A live viewport with orbit, pan, zoom, reset-to-authored-camera, and **Focus
  selection** (F), which centres the view on the selected entity; the zoom spans
  a factor of four hundred and moves proportionally, and the orbit cannot be
  driven onto the pole
- An axis indicator in the scene view's corner drawn from the same camera view
  the frame under it was drawn through, foreshortening and reordering its arms
  as the camera turns
- Perspective and orthographic toggle
- Scene and Game views, the latter rendering through the authored camera with no
  editor chrome painted over it — both live at once in the `2 by 3` layout
- Two workspace layouts chosen from **View → Layout** and remembered between
  launches: `2 by 3` puts Scene above Game with Hierarchy, Project, and
  Inspector beside them, and `Wide` shows one view at a time over a Project
  dock
- Play, pause, and stop, driving the real engine lifecycle rather than a display
  flag, and a separate **Discard changes** that returns the world to the file.
  Play advances sprite animations and Decay scripts, and hands scripts the
  keyboard while it does; pause
  holds the frame, and stop puts every clip back to its start. A scene at rest
  shows its clips' first frames, so a broken clip is reported without anyone
  pressing anything
- A Project dock listing the real contents of the directory the open scene
  lives in, with a list/grid toggle, a search that filters it, a refresh, and a
  double click on a scene row to open it
- A Console dock holding what the editor has actually said — every failure, what
  each scene turned out to be when it opened, and every texture it names that
  nothing has bound — bounded, with a repeated message collapsed into a count so
  a per-frame render failure cannot bury what explains it, and feeding the error
  and warning counts in the status bar
- Reopening the scene the editor was last left in, overridden by a path on the
  command line, with the file and its unsaved state named in the window title
- Preferences that survive a restart
- A deterministic full-window screenshot captured in CI

### Drawn, but does nothing

Listed because a control that looks like a feature is worse than an absent one.
`docs/editor-audit.md` is the full sweep — control by control, with what breaks
under use and what the editor cannot express at all. This is the summary, and
it is deliberately short now: everything the audit found is either working or
gone, and what is left here is waiting on a build rather than on a handler.

- **Play and Pause** — they run sprite animation and Decay scripts. No other gameplay
  is stepped, so the demo's own turning cube does not turn
- **Rotation** in the inspector — the word "Quaternion". The format stores a
  rotation and the renderer applies it; nothing edits it

Removed rather than left drawn, because each was a promise about a feature that
does not exist: the Select, Move, Rotate, and Scale tool modes, which set a mode
nothing read and had no gizmos to drive; "Scene", "Build", "Tools", and "Help",
which were labels shaped like menus; the top bar's project name; the hierarchy's
add-entity button and the inspector's Add Component, which need spawn and
component commands behind them; the inspector's Tag and Layer, which are not
things a Sindri entity has; the collapse chevrons and overflow menus that
collapsed and overflowed nothing; and the settings gear.

---

## Not yet

### Engine

- **No tilesets.** Sprites address part of a texture and animate through a
  sheet, but a tilemap has no data model, no component, and no renderer
- **No text rendering.** No score, menu, or dialogue
- **One mesh primitive: `Cube`.** No quad, sphere, or glTF import
- **No audio.** Now scheduled in `ROADMAP.md`; it previously had no item at all
- No physics or collision. This one is a deliberate gap rather than a missing
  foundation: collision against transforms is gameplay code, and a Rapier
  adapter is planned as optional rather than built in
- No tilemaps, particles, parallax, or pathfinding
- No TypeScript SDK; the WASM binding crate does not exist
- No spawning, input, timing, or cross-entity access from a script — a script
  reaches its own transform and nothing else; see below
- Hot reload covers assets, not the scene file: editing a scene on disk while it
  is open is not noticed
- No deterministic system ordering

### Editor

- Cannot create or delete an entity, or an asset. Components can be added and
  removed
- Does not read a project directory
- No gizmos, no viewport selection, no multi-select
- No prefabs, no play-mode-against-a-copy, no build or export controls
- No versioned editor protocol; the editor and runtime are one process
- Cannot open or edit a Decay script's *source* — the project browser lists
  `.decay` files as scripts, and opening one does nothing

---

## Decay

The gameplay language, in `decay/` — a **separate Cargo workspace**, not a
member of this one. Nothing under `decay/` depends on a `sindri-*` crate and no
engine crate depends on Decay, so everything below is true of the language in
isolation and none of it is true of the engine.

### Works

**A script drives a real entity.** `sindri-decay` binds the language to a world:
a `sindri.script` component names a source and a container, `WorldHost` gives
Decay's symbolic paths a meaning in terms of one entity's transform, and the
editor runs every script once a frame with whatever the transport says a frame
is worth. Authored `@export` properties reach the script before its first line.
Sources load through `sindri-assets` and hot-reload from the same `AssetWatch`
the textures use. Verified in the editor: the fixture's cube turns because
`editor/assets/scripts/spin.decay` says so, and Stop restores the world to the
pixel.

A script reaches its own transform's position, scale and Z rotation, its
sprite's tint and layer, the keyboard, the frame's delta and its own elapsed
time, six maths functions, and `print`. The whole table is in
`docs/scripting.md`. Verified in the editor: holding an arrow key moves the
fixture's cube, releasing stops it, Space recentres it and puts a line in the
console naming the entity that said it.
**Those paths are typed**, so `this.transfrom.position.x` is a compile error
with a line number rather than a first-frame failure, and reaching for a method
on a container says what to write instead. The analyzer's view and the host's
accessors are derived from one description, and a test walks every path the
analyzer accepts to assert the host answers it.

A failing script reports itself and does not stop the others.

A script is also text on disk that a test can run. `decay-syntax` lexes and parses
it, reporting diagnostics with a span, line, and column; the parser recovers
rather than stopping at the first error, and survives two hundred thousand
random token sequences without panicking or hanging. `decay-semantic` resolves
names through block scopes, checks a small type model (`f32`, `bool`, `String`,
`unit`, named host types), enforces `let` against `var`, and rejects duplicate
members and locals. Host globals such as `Input` enter through an
`Environment` rather than being builtins.

`decay-ir` lowers a checked program to a symbolic instruction list, with member
chains becoming paths such as `this.transform.position.x` rather than anything
the IR interprets. `decay-runtime` executes it: bindings, arithmetic,
comparisons, `if`/`else`, returns, calls between Decay functions, and script
instances whose fields persist across calls. Everything external crosses a
three-method `Host` trait — load a path, store a path, call a path.

The whole workspace compiles for `wasm32-unknown-unknown`, which its CI checks.
`decay/examples/player.decay` is executed by a test rather than only shown in
the README.

### Not yet

- **No loops**, and therefore no operation budget — call depth is bounded, which
  is the only unbounded path today
- No arrays, maps, closures, or first-class functions
- No standard library; not even `math`
- No spawning, despawning, or reaching another entity — blocked on Decay having
  a value that can hold one
- No mouse, and no components beyond `sindri.sprite`
- No script state migration across a source reload
- No LSP, no formatter, no debugger, no syntax highlighting anywhere
- No script state migration across a reload: a changed file recompiles, and the
  running instance keeps whatever fields it had
- Nothing has been *executed* on the browser target, only compiled
- The only numeric type is spelled `f32` and every value it holds is an `f64`
