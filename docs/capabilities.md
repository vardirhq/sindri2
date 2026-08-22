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

### Grid geometry

`sindri-grid` provides renderer-independent signed cell coordinates, continuous
grid and plane points, finite rectangular bounds, deterministic cardinal and
eight-way neighbour queries, and validated orthogonal/isometric projection.
Cell centres and arbitrary fractional points round-trip through both projections
across negative and positive space. Upward/downward plane Y adapts world and
screen coordinate conventions without introducing a renderer dependency.

`sindri.tilemap` exposes the exact `GridSpace` and `GridBounds` its rendering and
editor picking use. A map's complete transform is composed around that local
grid, so moving, rotating, or scaling a map keeps drawn and picked cells in
agreement. Decay can read an entity's continuous logical X/Y relative to that
tilemap and place it back through the same projection and map transform. General
camera/viewport conversion remains missing.

`docs/feature-integration-matrix.md` tracks those counterparts explicitly rather
than allowing the runtime type to make the broader feature look complete.

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
**A sliced image says how it is cut, once.** A sheet document beside a texture —
`textures/tiles.png` is sliced by `textures/tiles.sheet.json`, at a derived ID
nothing has to declare — names the parts of it, either as a grid or as explicit
rects. A scene then writes `textures/tiles.png#floor`, and the `#` that had
always been rejected inside an asset ID is what carries the name, because it was
reserved so a fragment could not leak into a path. Three components used to carry
their own copy of a sheet's layout and could disagree; none carries one now.
Names rather than indices, so a re-slice that moves a cell does not silently
change what a scene draws.

Rects are checked once, where a sheet binds, and ride on the instance so every
part of one sheet stays in one draw call — a GPU test reads the pixels back to
prove the shader honours it. A name no loaded sheet places draws the whole image
and is reported by `unresolved_sprites`, the same rule an unbound texture
follows.

Animation reads those names: `sindri.sprite_animation` holds clips of sprite
names and which one plays, `SpriteAnimations` holds where each sprite has got to,
and the cursor holds a name rather than a rect so playback does not depend on
where anything sits in an image. Playback is runtime state, so watching an
animation run does not rewrite the scene it came from.

**A sheet is sliced in the editor, on the image.** Selecting a texture shows it
in the inspector with each cell outlined on the picture; columns, rows, margin
and spacing are drags, so a packed sheet with gutters can be cut and not only one
that divides edge to edge. A cell is named by clicking it, and the panel lists
the cells that have names rather than a field per cell, so a 16x16 atlas is as
workable as a four-frame strip. Saving writes the sidecar, and the browser then
lists the sprites underneath the image, collapsed until asked for. A sprite's
animation inspector creates, renames, and removes clips from those named cells,
orders frames, edits timing and looping, chooses the runtime clip, and previews
it without changing scene state.

A tilemap is a grid of tiles drawn from one entity: `sindri.tilemap` carries the
map's grid, a palette of sprite names, and a flat array of cells indexing that
palette, with `null` where the map is empty. Its cells become instances in the
same batches loose sprites use, so a prop sorts among the floor rather than
behind it. Columns and rows lay out orthogonally or isometrically, and placing a
tile and finding the tile under a point are inverses on every cell of both. What
it buys is authoring rather than speed — the same floor already batched into one
draw — and the companion game measures it: 68 entities to 20, 45KB of scene to
12KB.

**World-space tilemaps can be authored in the editor.** Selecting one shows the
slices from its texture's sheet as a visual palette, lets its grid be resized
while preserving the overlap, and turns primary drag in the Scene view into an
undoable paint or erase stroke. The hovered cell is found by inverting the exact
camera matrix used to render that viewport, so an orthographic or isometric map
does not need separate picking approximations. The ordinary component fields
still edit projection, tile size, tint, render layer, and space. Screen-space
tilemaps render and remain editable as data, but direct painting is deliberately
limited to world space until the Game view has an authoring-input contract.

An animated sprite whose reference names no part of its sheet draws the first
frame of the clip it is playing rather than the whole sheet, so a scene shows a
pose before anything has run it — in a game's opening frame, in an offscreen
capture, and in the editor outside play mode. Once a clip is running it decides
alone: a frame that resolves to nothing draws the whole image rather than the
resting pose, because a plausible picture of the wrong moment is the failure that
hides.
A sprite is either screen-anchored, which is the default and cannot be occluded
by the world, or in the world, drawn through the world camera by its full
transform and hidden by opaque geometry in front of it. Sprites batch per space,
layer, and texture, with a measured five-to-one draw-call reduction. Each batch
draws with its own camera and its own instances, from buffers of its own; a GPU
test draws two batches at two scales into one frame and reads back the pixels
that only come out right when they stayed separate. The frame
clears once before anything draws, so a scene of only sprites has a depth buffer
to test against. Perspective and orthographic cameras exist with tested projection
maths, and the zero-to-one depth range and Y-up convention are chosen in one
place rather than at each call site. Colour space is enforced by a shared
constant, and the offscreen capture verifies authored colours actually survive
to the pixels.

Offscreen rendering produces a deterministic PNG, which CI uploads — two of
them: the cube proof, and the companion game photographed part-way through a
scripted run.

**The engine runs in a browser**, which until recently it had never been asked to
do. The cube example draws the same picture in Chromium as it does natively, in
the same colours, over WebGPU, and **the companion game is playable there**: the
keyboard drives the player, an orb is collected, and a lamp lights, which is
Decay executing on the browser target for the first time along with entity
references, the blackboard and input. `scripts/browser/smoke.mjs` checks that a
page starts the engine and fails when it does not.

What still has *not* run in a browser is asset loading. Both callers embed their
assets, so the decoders run and `AssetLoader` and `UrlRoot` are still only
exercised by tests. `docs/browser.md` records what the first run found, which was
two bugs that had been true since the browser target was added.

### Scene to frame

Gameplay writes to the world and nothing tells the renderer. `SceneExtractor`
derives an ordered frame from whatever the world currently holds, using the
built-in camera, mesh, sprite, tilemap, animation, and text schemas. No scene
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
overwrite a replacement. Textures, fonts, sheets, scripts, and scene JSON decode
through typed decoders. Nothing pretends browser I/O is synchronous.

A `TextureId` is a generation-checked slot handle, so the renderer's texture
registry can release one texture and reuse its slot without a handle nobody
updated drawing whatever lands there next.

`AssetLoader` drives all of that in one place — request, enqueue, drain, decode,
apply — so a caller does not have to know the order. Requesting is idempotent, a
failure is reported once rather than retried forever, and `retain` releases what
is no longer wanted and says which IDs went, so a host can drop whatever it
built from them. GPU upload stays with the host, which is the only thing that
owns a device.

`FontAssetDecoder` validates OpenType bytes and records their declared family;
text binds that project-owned face under the scene's logical asset reference,
so native and browser builds never substitute different installed fonts.

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
- Shows the hierarchy from live runtime state as a Unity-style GameObject tree:
  every entity may own children, child-bearing rows fold with state remembered
  across launches, search retains and temporarily opens each match's ancestor
  path, selection works anywhere on a row, and empty space or Escape clears it
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
- **Creating empty root or child GameObjects and deleting entities**, from the
  hierarchy. Creation assigns a stable scene ID immediately, and creating a
  child opens its parent. Deleting takes the whole subtree, and **undo brings
  it back at the same handle** — so the
  selection and every earlier edit in the history keep pointing at what they
  named. That works because the history undoes in order: reaching a delete
  means everything after it is already undone, so the slot it freed is free
  again
- **Adding and removing components.** Add Component offers what the entity lacks
  and the registry can create, which excludes a type with no sensible blank
  rather than offering one the engine would reject. Text and sprite animation
  are completed at the editor boundary: a project font gives Text a valid
  visible default, while a Sprite component whose texture has named sheet
  sprites gives Sprite Animation its first one-frame clip. Both are undoable
- **Text authoring.** A `sindri.text` component gets a multiline content editor
  and a picker listing project-relative font assets. Existing missing font
  references remain visible and are called out instead of silently replaced
- **Sprite-sheet and animation authoring.** Selecting a texture opens its image
  slicer for grid dimensions and cell names. A `sindri.sprite_animation`
  component then creates, renames, and removes clips from those names, orders
  frames, edits frame time and looping, chooses what plays at runtime, and
  previews the selected clip against the real texture. Preview position is
  editor state and never dirties the scene
- **A script's `@export` properties in the inspector**, drawn from what the
  script declared: the field's name, its type, and its default, without running
  anything. A field the scene has not set shows its default and says so, and
  setting one is what puts it in the scene — so a scene records an author's
  choices rather than a copy of every default. This is the capability that
  justified a statically typed language
- Reparenting through a **Parent** menu or by dragging onto another GameObject
  or the World root. Both ask the world's cycle check before offering or
  accepting a move; drag targets show whether they are legal, successful drops
  select the moved entity and open its new parent, and the move is one undo step
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
- Scene-view click selection for world sprites, filled tilemap cells, and
  meshes. It inverts the exact camera used to draw the frame, uses the
  renderer's unit quad and cube dimensions, resolves transparent overlaps by
  layer and depth, honours opaque occlusion, and clears selection on empty
  space. Camera drags and tile painting retain the pointer when active
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
old inert add-entity button and the inspector's old inert Add Component button,
both since replaced by working command-backed controls; the inspector's Tag and
Layer, which are not things a Sindri entity has; the inert section chevrons and
overflow menus, superseded in the hierarchy by real child-row folding; and the
settings gear.

---

## Not yet

### Engine

- **No reusable tileset asset model.** A tilemap has a component, renderer, and
  palette of named sprites from a sheet, but tile semantics such as terrain,
  collision, and reusable tile metadata do not exist
- Text is currently screen-space only; world labels, wrapping/alignment
  controls, and rich spans remain
- **One mesh primitive: `Cube`.** No quad, sphere, or glTF import
- **No audio.** Now scheduled in `ROADMAP.md`; it previously had no item at all
- No physics or collision. This one is a deliberate gap rather than a missing
  foundation: collision against transforms is gameplay code, and a Rapier
  adapter is planned as optional rather than built in
- No particles, authored parallax system, or pathfinding. Renderer-free
  footprints, bounded occupancy, and placement validation now exist, but have
  no engine component, editor, Decay, or Gather adapter yet
- No optional TypeScript embedding SDK; browser games currently expose narrow
  application entry points and run their gameplay in Decay
- No spawning from a script. Cross-entity lookup, access, existence checks, and
  despawning work through generation-checked entity references; see below
- Hot reload covers assets, not the scene file: editing a scene on disk while it
  is open is not noticed
- No deterministic system ordering

### Editor

- Cannot create or delete an asset
- No first-class project model or multi-scene workspace. The Project dock does
  read and filter the directory containing the open scene
- No gizmos or multi-select; viewport selection currently covers world-space
  sprites, filled tilemap cells, and meshes rather than screen-space overlays
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
time, logical grid position through an explicit tilemap entity, six maths
functions, and `print`. The whole table is in
`docs/scripting.md`. Verified in the editor: holding an arrow key moves the
fixture's cube, releasing stops it, Space recentres it and puts a line in the
console naming the entity that said it.
**Those paths are typed**, so `this.transfrom.position.x` is a compile error
with a line number rather than a first-frame failure, and reaching for a method
on a container says what to write instead. The analyzer's view and the host's
accessors are derived from one description, and a test walks every path the
analyzer accepts to assert the host answers it.

A failing script reports itself and does not stop the others.

**A whole game's rules are written in it.** The companion game's moving,
gathering, counting and winning are four Decay scripts and no Rust — see "The
companion game" below.

**A script can name another entity.** `Value::Reference` is a value Decay can
hold, pass, compare and store but cannot construct or look inside; the engine
packs a runtime handle into it. `World.find` looks one up by the name a scene
gave it, `World.exists` asks whether it still names anything, `World.despawn`
removes it, and reaching through one gets the same transform and sprite paths a
script reaches on itself — checked at compile time, so `other.transfrom` is an
error with a line number. Reaching through a stale or null reference is reported
rather than silently ignored. Verified in the game: the orbs used to compare
against a position the player published to the shared board, and now ask the
player directly, with the picture unchanged.

The board is still there and still earns its place, for facts that belong to the
game rather than to an entity — the score, whether the game is won.

**A script can speak in the tilemap's coordinates.** `Grid.position_x` and
`Grid.position_y` invert a tilemap's projection and full world-XY transform;
`Grid.place` projects a continuous logical position back while preserving the
actor's Z. Both arguments are typed entity references, and the grid must
explicitly be the entity carrying `sindri.tilemap`, so a world with two maps is
not governed by an accidental first match. Gather uses this surface for player
movement, floor bounds, and orb distance checks.

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
three-method `Host` trait — load a path, store a path, call a path. Each takes a
subject as well: `None` for a path a script rooted at something the host owns,
and the reference for one rooted at a value the script is holding. Three methods
and not six, because a subject is an argument rather than a mode.

The whole workspace compiles for `wasm32-unknown-unknown`, which its CI checks.
`decay/examples/player.decay` is executed by a test rather than only shown in
the README.

### Not yet

- **No loops**, and therefore no operation budget — call depth is bounded, which
  is the only unbounded path today
- No arrays, maps, closures, or first-class functions
- No standard library; not even `math`
- **No spawning.** Creating an entity means saying what to create and the engine
  has no prefab to say it with, so this is blocked on the engine rather than on
  the language. Finding, reaching, and despawning exist
- Despawning is not undoable — no script write is, and play mode restores from a
  snapshot, so routing it through `WorldCommand` stays open
- No mouse, and no general component surface beyond `sindri.sprite`; tilemaps
  are available only through the deliberately narrow `Grid` coordinate API
- No LSP, no formatter, no debugger, no syntax highlighting anywhere
- No script state migration across a reload: a changed file recompiles, and the
  running instance keeps whatever fields it had
- Nothing has *fetched* an asset on the browser target: both callers embed
  theirs, so `AssetLoader` and `UrlRoot` remain exercised only by tests
- The only numeric type is spelled `f32` and every value it holds is an `f64`

---

## The companion game

`game/`, crate `sindri-gather` — "Gather". Five orbs on a diamond floor, a
thing you drive with the arrow keys, a row of lamps that fills as you collect
them, and a banner that fades in when you have them all. `ROADMAP.md` says why
it exists and why it is not an example; this says what of it is real.

### Works

**Its floor is a tilemap on a sliced sheet.** One entity holding a 7x7 grid of
cells indexing a two-name palette, where it was 49 sprite entities; the picture is
the same to within one 8-bit step, which is the cost of baking the darker checker
square into the sheet rather than tinting
it at draw time.

**It is a game you can play.** `cargo run -p sindri-gather` opens a window,
arrow keys move the player, walking into an orb takes it, taking all five wins.
Escape quits. It runs its gameplay on the fixed step, so gathering happens at
the same rate whatever the frame rate is.

**None of its rules are in Rust.** Moving, gathering, counting and winning are
four Decay scripts in `game/assets/scripts/`. The Rust is a window, a device, a
loop, and the embedded bytes of the scene, the scripts, and the art. That split
is what the game exists to test, and it held. The game first earned one engine
feature, `Game.get`/`Game.set`, as a shared blackboard. Entity references later
let each orb find the player and read its transform directly; the board remains
for game-wide facts such as score and victory. Neither change added gameplay to
Rust.

**Its gameplay now uses the diamond's logical grid.** The player and orbs read
continuous coordinates through the floor tilemap, movement clamps to the 7x7
logical bounds, and placement projects back through the same isometric mapping
that draws and picks the floor. Arrow keys follow the two diagonal grid axes;
holding two walks along a screen axis. Orb bobbing remains a presentation offset
and is reset from the logical resting point every frame.

**It is checked, not just run.** `game/tests/the_game_holds_together.rs` asserts
every texture and script the scene names is shipped, that the scripts compile,
that every authored property names a field its script `@export`s, that the scene
holds no component the game cannot run — and it plays the game through the same
scripts and the same `InputState` the window feeds, steering to each orb in turn
and checking the banner comes up.

**It is a CI artifact.** `gather-capture` plays a fixed run — a fixed key held
for a fixed number of fixed steps — and photographs where that leaves the game,
so the picture proves the scripts ran rather than that the scene loads.

**It opens in the editor, and Play runs it there.** All 21 entities load, both
viewports draw it, and the editor advances the same Decay sources the standalone
game does. `docs/editor-meets-the-game.md` records the first editor session
against the older 68-entity scene; the tilemap removed its 49 floor rows and
world renderables can now be selected in the Scene view. The general hierarchy
case is covered too: every GameObject may contain children, folded rows retain
their state, and search shows the path to each match.

**It found a bug the proofs could not.** It is the first thing in the workspace
that draws a world and a screen overlay in one frame, and doing so revealed that
every sprite batch after the first drew with the last batch's camera. See
`docs/rendering-frame-pipeline.md`.

### Not yet

- Text draws the title from the shipped Inter asset, but the score remains a
  row of lamps and winning remains a banner sprite until dynamic script-to-text
  binding exists
- No sound, because there is no audio
- Browser asset fetching is not exercised: the playable web build embeds its
  scene, scripts, sheets, textures, and font
- No restart without relaunching, no menu, no pause
