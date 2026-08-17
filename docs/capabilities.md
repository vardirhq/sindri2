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
children, an optional `Transform2D` and `Transform3D`, arbitrary JSON
components, and an editor-only section the runtime never interprets.

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
set name, set either transform, set parent, set or remove a component.
`Transaction` is all-or-nothing, and `CommandHistory` gives bounded undo and
redo with labelled steps and merge runs, so a continuous drag collapses into one
undoable step rather than several hundred.

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
depth testing, and textured sprites with tint, anchor, layer, and sorting depth.
Sprites batch per layer and texture, with a measured five-to-one draw-call
reduction. Perspective and orthographic cameras exist with tested projection
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

---

## The editor

Reflects `main`. The editor renders the real runtime frame through eframe's
shared WGPU device — it is not a mock, and it does not create a second device.

An action that fails says so: a sticky notice from the last thing you did is
kept apart from the per-frame render result, which used to overwrite it within a
frame.

### Works

- Opens a scene from a command-line argument or **File → Open scene**, saves it
  back canonically, reloads from disk, and discards changes
- Shows the hierarchy from live runtime state, nested, searchable, with
  selection — and with clearing the selection by clicking empty space or Escape
- Inspector edits of name, `Transform2D`, and `Transform3D`
- Reparenting through a **Parent** menu that offers only the moves the world
  would accept
- Undo and redo of every edit, with drag-merging so a slider drag is one step
- A live viewport with orbit, pan, zoom, and reset-to-authored-camera
- Perspective and orthographic toggle
- Scene and Game views, the latter rendering through the authored camera with no
  editor chrome painted over it — both live at once in the `2 by 3` layout
- Two workspace layouts chosen from **View → Layout** and remembered between
  launches: `2 by 3` puts Scene above Game with Hierarchy, Project, and
  Inspector beside them, and `Wide` shows one view at a time over a Project
  dock
- Play, pause, stop, and reset-to-authored-state, driving the real engine
  lifecycle rather than a display flag
- A Project dock with a list/grid toggle, and a Console dock
- Preferences that survive a restart
- A deterministic full-window screenshot captured in CI

### Drawn, but does nothing

Listed because a control that looks like a feature is worse than an absent one.

- **Select, Move, Rotate, Scale** — they set a mode nothing reads. There are no
  gizmos
- **`+` Add entity** and **Add Component** — not handled. Entities and components
  can only come from a file
- **Filter assets** — decoration
- **The project browser's contents** — eight hardcoded entries. It does not read
  a directory, so it shows the same list whatever scene is open
- **Edit, Scene, Build, Tools, Help** — plain labels, not menus. File and View
  open
- **Tag** and **Layer** in the inspector, and the per-component property rows —
  fixed text, not the component's actual values
- **The project name in the top bar** — a label
- **The Console** — three fixed lines. Two interpolate real values (entity count,
  lifecycle state), so it is a status readout rather than a log; nothing the
  engine reports reaches it

---

## Not yet

### Engine

- **Sprites are pinned to the screen.** A sprite's `Transform2D` is an offset
  within the overlay camera's extent, and the camera's view matrix cancels its
  own centre, so moving a 2D camera moves nothing. There is no 2D world to move
  a character through
- **One texture is one sprite.** No UV rects, so no sprite sheets, no animation
  frames, no tilesets
- **No text rendering.** No score, menu, or dialogue
- **One mesh primitive: `Cube`.** No quad, sphere, or glTF import
- **No audio.** Now scheduled in `ROADMAP.md`; it previously had no item at all
- No physics or collision. This one is a deliberate gap rather than a missing
  foundation: collision against transforms is gameplay code, and a Rapier
  adapter is planned as optional rather than built in
- No tilemaps, particles, parallax, or pathfinding
- No TypeScript SDK; the WASM binding crate does not exist
- No hot reload, asset manifest, or GPU asset release
- No deterministic system ordering

### Editor

- Cannot create or delete anything — no entity, no component, no asset
- Does not read a project directory
- No gizmos, no viewport selection, no multi-select
- No component editing beyond names and transforms
- No prefabs, no play-mode-against-a-copy, no build or export controls
- No versioned editor protocol; the editor and runtime are one process
