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

A **prefab** is an authored reusable entity definition: a single-root scene
fragment in the same document shape a scene uses, sharing its entity rules,
component payloads, versioning, and canonical serialization. `World::spawn_prefab`
creates the whole subtree or none of it, and answers with the root, every entity
made, and which authored identity each became. Instances carry no `source_id`,
because a prefab's identities name entities inside the prefab and two instances
would collide on every one. `docs/prefabs.md` is the contract.

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

Keyboard, mouse, and touch input arrive as platform-independent events and
accumulate into an `InputState` that answers held, pressed-this-frame, and
released-this-frame for keys and mouse buttons, plus pointer position, pointer
delta, scroll delta, and window focus. `axis(negative, positive)` returns -1, 0, or 1 and gives zero
when both are held, so opposed movement keys cannot resolve by event order.

Touch is held beside the mouse rather than folded into it, because they are
different facts: a mouse has one position and is always somewhere, while fingers
arrive and leave and there may be several. Fingers are bounded at ten, ordered
by the id their host gave them so one keeps its place while it stays down, and
let go of when a window loses focus — a finger cannot be reported as lifted once
the window has stopped hearing about it. What *unifies* the two is a separate
question and is answered where a game reads it: `pointer_position` is the mouse
if there is one and the first finger otherwise, and `pointer_down(Left)` is the
left button or any finger.

The editor routes all of it through the Game view during Play, in **that view's
own pixels**, so a script reads the same position there as in the real build.
A pointer outside the view is reported as gone rather than clamped to its edge,
because a game told the person is pointing at somewhere they are not is worse
than a game told they are not pointing at all.

### Audio

Audio is a platform service rather than simulation state. Encoded WAV, Ogg, and
MP3 assets are identified and validated by the asset layer, then registered with
an `AudioBackend`. Native hosts use Rodio/CPAL, browser hosts use media elements
with an explicit user-interaction unlock, and headless tests use a silent backend
that records every request without needing a sound device. The shared boundary
supports one-shot and looping playback plus per-voice stop and global pause,
resume, and stop.

Scenes can author `sindri.audio.source` with a logical clip ID, autoplay, looping, and a
normalized volume. The generic component inspector can add and edit it, and the
project browser recognises WAV/Ogg/MP3 files as audio assets. Decay exposes typed
`Audio.play`, `loop`, `stop_all`, `pause_all`, and `resume_all` calls while only
emitting intent, so the language itself retains its no-I/O boundary. Gather
exercises the path end to end with background, pickup, and victory sounds. The
looping background music has been observed playing in a real browser through
`scripts/browser/smoke.mjs`, which fails if a clip a page asked for did not play.

What audio does not do yet: nothing gathers the clips a scene names, the way
`referenced_textures` and `referenced_fonts` do, so a host loads audio from its
own list and the editor cannot load a scene's audio at all. The editor lists
audio files and edits `sindri.audio.source`; it cannot play one.

### Physics

`sindri-physics` is the masked physics boundary. Rapier2D and Rapier3D are
private implementation dependencies; public code speaks only in Sindri body,
collider, shape, layer, pose, error, and event types. Runtime entities remain the
identity exposed by the subsystem, so no Rapier handle can leak into a scene,
script, editor, or game contract.

The exercised runtime is currently 2D. `PhysicsWorld2d` owns fixed, dynamic,
position-kinematic, and velocity-kinematic bodies; box, circle, and capsule
colliders; independent membership/filter masks; sensors; friction and
restitution; checked velocity/impulse/kinematic operations; and normalized
collision/sensor start/stop events. Values are validated before they reach the
backend, and stepping takes an explicit engine fixed-step duration rather than a
render delta. Tests cover gravity, masks, sensor events, body operations, and
removal/reuse through generation-checked `EntityId`s, and the crate passes the
workspace's native and WASM checks.

A parallel Sindri-owned 3D body/collider data model already fixes the public
shape of the later 3D slice, but no 3D runtime behavior is claimed yet.

`sindri.physics2d.rigid_body` and `sindri.physics2d.collider` are registered
scene components with defaults the engine accepts, so a scene authors bodies and
colliders and the editor's generic component inspector adds and edits them.

`ScenePhysics2d` is what joins the two halves. It builds the simulation from
those components, keeps it in step as entities are spawned, switched off, and
despawned — a body outliving its entity would collide on behalf of nothing — and
writes what physics decided back into the transforms the renderer reads, leaving
the authored Z and the 3D scale alone. A body whose authored values have not
changed is left as it is rather than rebuilt, because rebuilding one discards the
velocity and contacts the simulation owns. Editor Play and the game session both
step it once per fixed update, before scripts run, so a script observes the
events of the step that just happened.

What the editor does not have is physics-specific authoring: no collider outline
in the viewport, no handles for a shape, and nothing that shows a body's kind
without reading the payload. No game yet plays a scene whose bodies move —
`games/orbital-last-stand` is where that proof will come from. Both remain
separate feature-track slices in `docs/physics.md` rather than things this
foundation pretends to have completed.

### Randomness

`sindri_core::Rng` is a **PCG-XSH-RR 64/32** generator written out rather than
depended on. Every general-purpose crate reaches the operating system for a
seed, which on `wasm32-unknown-unknown` means `getrandom` and a target that
refuses to compile without an opt-in — and more to the point, entropy is the
opposite of what this is for. A run that cannot be replayed from its seed is not
seeded at all.

Everything in it is integer arithmetic and the one division is by a power of
two, which is what makes a seed mean the same thing on every host. Fractions are
built from the top 24 bits, so `[0, 1)` is never `1.0`; bounded integers reject
the draws that would make the low values slightly more likely, because modulo
bias on a drop table over a long run is exactly the kind of wrongness that gets
blamed on the game design. Seeding takes two steps around the state, so
neighbouring seeds do not produce neighbouring first outputs.

The engine never asks the platform for entropy and does not pretend to: a host
that seeds nothing gets a fixed stream. The editor puts the stream back to its
seed on every fresh Play, so pressing Play twice gives the same run twice and a
bug found once can be found again; resuming from a pause deliberately does not,
since that would replay numbers the scene has already acted on. A game that
wants variety calls `Random.seed` with something it knows.

One stream is shared by every script, so the seed determines a run but a number
drawn early shifts every number after it. That is stated rather than hidden, and
it is why a run's seed is worth storing while a frame's numbers are not. It is
not a source of secrets: a handful of outputs reveals the state.

### Screen UI

`sindri.ui.image` and `sindri.ui.text` draw against the viewport. Text is a
**template**: the words carry `{}` slots and a script supplies the numbers, so a
HUD updates without Decay needing string concatenation it deliberately does not
have. An image carries a **fill** fraction and the edge it empties from, which is
what makes a bar a bar rather than a picture of one.

`sindri.ui.button` makes an element pressable, its rect being the entity's own
transform. `ScreenUi` is the runtime beside the world: it lays every element out,
hit-tests the pointer, and answers hover, click and hold. A click is a press and
a release on the same element, so sliding off before letting go changes a
person's mind. Overlapping elements are resolved by layer, so a modal is a modal
because it is on top. A disabled entity — or one under a disabled parent — is not
hit-tested, which is also what a *screen* is: a menu is an entity with children,
and showing it is switching it on. No screen stack was added, because the engine
already had the mechanism.

Nothing is silently withheld from gameplay while a menu is up: which scripts are
gameplay is not something a host can know. `Pointer.over_ui` is the one line a
gameplay script writes instead.

`sindri.ui.layout` places a parent's active children in a row or column. Three
buttons could be authored as three offsets; what cannot be authored is a menu
closing up around an entry that was switched off.

The overlay is authored in normalized units — two tall, centred, running out to
the aspect ratio — so one authored scene is responsive across a portrait phone
and a wide desktop window without a breakpoint. A **safe area** takes a notch or
a home indicator off the edges, moving anchored elements in while leaving centred
ones where they are; the editor reports none, because a desktop window has no
notch.

What is not built: no scroll region, and no accessibility surface. A button
carries a `label`, authored beside the thing it names, but nothing reads it —
there is no DOM to expose it to until a project can be exported to the web, and
a second accessibility path invented before then would be the wrong one.

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
agreement. `sindri.grid.navigation` adds authored internal wall edges without
duplicating those bounds or projection settings, while `sindri.grid.occupant`
names its grid by stable scene ID and carries a relative multi-cell footprint.
`WorldGridNavigation` resolves those references, derives anchors from current
world transforms, rejects conflicting or invalid placements as one incomplete
snapshot, and exposes validated placement and wall-aware path queries.

Decay can read an entity's continuous logical X/Y relative to a tilemap and
place it back through the same projection and map transform. General
camera/viewport conversion and typed Decay access to occupancy and paths remain
missing.

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

Animation reads those names: `sindri.animation.sprite` holds clips of sprite
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

- **Opens a project, not only a scene.** A project is a directory holding
  `sindri.toml`, which carries a format version, the project's name, and the
  scene opening it opens. The welcome window is its own window and the editor's
  is hidden behind it until a project is open: it lists the twelve most recently
  opened projects, marks one that has moved or been deleted as missing rather
  than dropping it, makes a project — a manifest, a scene, and the folders
  assets resolve from — and opens a folder that already is one. Gather ships
  with a manifest and is listed as a shipped sample when the editor is run from
  the repository. A scene opened from anywhere walks up to the nearest
  `sindri.toml`, so the browser is rooted at the whole project and headed with
  the project's own name — Gather rather than `assets`. What it *lists* is the
  directory asset references resolve against, which is the open scene's own:
  a project's Cargo manifest and `src/` are part of the project and are not
  files a component can name, and the rest of the project is a switch in the
  browser's toolbar away, remembered between launches and drawn only where the
  two listings differ. A launch honours the
  command line first, the last project when the user asked for that, and the
  welcome window otherwise. **Set as main scene** on a scene row nominates what
  the project opens on, and a scene made in a project that nominates nothing
  claims the empty place rather than leaving the project opening on nothing.
  `docs/project-format.md` is the contract
- Opens a scene from a command-line argument or **File → Open scene**, saves it
  back canonically, reloads from disk, and discards changes — including a scene
  carrying components it has never heard of, which it keeps through a save and
  lists in the inspector
- **Makes a scene and forks one**, so a project can be started rather than only
  continued. New scene (Ctrl+N) asks where the file goes, writes it, and opens
  it through the ordinary path; it holds one world camera, because a scene with
  none renders a black Game view, and it is named from its file. Save scene as…
  (Ctrl+Shift+S) is offered whether or not the scene has a file behind it —
  giving a detached scene one is the case that used to have no answer — and the
  project beside the scene, the remembered scene, the textures and the scripts
  all move with it. A save box takes a name rather than an extension, so
  `level` is written as `level.scene.json`, which is what the browser lists as a
  scene and what reopening it finds
- Shows the hierarchy from live runtime state as a Unity-style GameObject tree:
  every entity may own children, child-bearing rows fold with state remembered
  across launches, search retains and temporarily opens each match's ancestor
  path, selection works anywhere on a row, and empty space or Escape clears it
- **Lists world objects and UI objects apart**, under a World group and a UI
  group, with icons of their own. Which group a top-level entity is in is read
  from what it carries — a `sindri.ui.*` component means the viewport — so
  nothing has to be kept in step by hand and no entity can claim a space it is
  not drawn in. A group holding only UI elements is listed with the UI.
  Create GameObject makes an empty object or a UI Image directly, and the
  inspector says which space the selected entity is in
- **Snapping increments that can be set**, from a right-click on the snap
  button, and remembered across launches. They were constants the tooltip named
  and nothing could change. A step of zero means that one does not round
- **A console that can be read.** Filtered by level — everything, problems, or
  only what did not happen — and remembered across launches, because someone
  watching for a failure wants it filtered for as long as they are watching.
  Clear empties it, so a transient failure stops being counted once it has
  stopped being true. An entry about an entity ends in that entity's name and selecting it goes there:
  a script failure used to print the runtime's own handle, which is not
  something anyone can look for in a hierarchy
- Inspector edits of name and the complete transform: position, Euler-degree
  rotation backed by the stored quaternion, scale, and the Z lock, which takes
  away movement off the current layer
- **An entity's stable ID, shown and editable.** It is what the file keys an
  entity by, what a parent link names, what sibling order is derived from and
  what `sindri.grid.occupant` points at, and it used to be invisible — so a
  scene made here was `game-object-1`, `game-object-2`, and a shipped scene's
  `player` and `orb-1` were unreachable. Renaming one carries every occupant
  that names it along in the same undo step; one that is blank or already taken
  is refused at the field rather than written
- **The scene itself, where an entity's inspector would be.** With nothing
  selected the panel shows the scene's own name — a real field that round-trips
  through a save — along with its file and how many entities it holds. The
  rename is an ordinary undoable edit, so the editor still knows the document
  is unsaved
- **Editing any component's fields in the inspector**, driven by the stored
  payload rather than by hand-written rows: numbers get drags, booleans get
  checkboxes, text gets a field, and a short numeric array gets a labelled row.
  A component the engine has never heard of is editable too, which is what the
  preserve policy promises. Every edit goes through `SetComponent`, so it
  undoes; every edit is checked against the component's own schema first, so one
  that would stop it decoding is refused and said aloud rather than written into
  a scene that then will not open
- **Controls that know what a field means.** A component draws every field it
  has, filled out from the registry's own blank, so two of one component show
  the same rows and a field nobody wrote down is still visible at what it means;
  only a field actually changed is written back. A value that is one of a few
  names is a menu — a camera's projection, a UI anchor, a tilemap's projection,
  a rigid body's kind — taken from the engine's own list. A camera's projection
  decides which other fields it has, so choosing one writes them: switching to
  orthographic drops the vertical field of view, keeps the near and far planes,
  and gains a vertical size, which typing the word into a text box could never
  do. A field naming a project asset offers what the project holds while
  staying typeable — spelled the way the open scene resolves it, which is not
  the path from the project root whenever a project keeps its scene under
  `assets/`, and a reference the loader could never reach is offered nowhere
  rather than under a path that will not load — a tint opens a colour picker,
  and a row that is only a readout says on hover why it is one
- **Switching an entity off without deleting it.** Off means it takes no part
  in the scene — not drawn, not stepped, not scripted, not picked — and neither
  does anything under it, while it stays in the world and in the file. An Active
  switch on the inspector, Disable and Enable on a hierarchy row's menu taking
  the whole selection, and a struck-through row for anything switched off. The
  flag is per entity and never written down through a subtree, so re-enabling a
  parent brings back exactly the children that were on
- **A History dock showing what undo will do, and everything past it.** The
  undo stack drawn: "Scene opened", every step in the order it happened, the
  step the world is at marked, and the undone steps still listed under it
  because they are still reachable. Clicking one travels there, by calling the
  same undo and redo the keys call, one step at a time
- **Everything the Scene view draws can be clicked in it**, including strings.
  Meshes, world sprites, filled tilemap cells, authored cameras and UI images
  are hit-tested from their own geometry; a string has none in the scene, so its
  box is measured by the text renderer that draws it — the same shaping, at the
  resolution the view renders at — rather than guessed from the font size, which
  would pick the wrong entity along its edges. A fully transparent element is
  skipped in both passes: a thing drawn as nothing is not a thing to click
- **Sibling order, moved rather than renamed.** Move up and Move down on a
  row's menu and on Alt+Up and Alt+Down, greyed out at the ends of a list. The
  order lives in the entity's editor-only section of the file rather than in
  the scene proper, because document order is canonical and meaningless by
  design and draw order is render layers and depths — so where a row sits in a
  panel is a fact about the panel. A scene nobody has reordered still lists
  alphabetically by stable ID
- **More than one entity at a time.** Ctrl-click adds and removes, Shift-click
  takes the range between two rows as the hierarchy is drawing them, and
  Ctrl-click does the same in the Scene view. Delete, Duplicate and a drag to a
  new parent then take the whole selection in one undo step, and dragging the
  handles moves, turns or scales every selected entity by what the one under
  the pointer was moved, turned or scaled by — from each one's own start, so a
  row stays a row. One panel and one set of handles can only be about one
  subject, so the inspector stays on the last entity pointed at and says how
  many the verbs outside it would take; the rest of the selection wears a ring
  in the Scene view where its own handles would have been
- **Every verb that acts on one entity, from that entity's own right-click
  menu.** Rename, Duplicate, Create child, Frame in the Scene view, and Delete,
  each also on a key — F2, Ctrl+D, F, and Delete or Backspace. Rename happens in
  the row itself, focused as it appears, committed with Enter and abandoned with
  Escape, so fixing one name among forty does not move your eyes to another
  panel; a double click starts it. Duplicate copies the whole subtree beside the
  original as a sibling, gives each copy a stable ID nothing else is using, and
  undoes in one step. A project row has a menu of its own for what the browser
  can already do — open a scene, look inside a folder, slice an image — plus the
  asset path a component field wants, the one the open scene resolves against,
  which until now had to be read off the row and typed back in
- **Creating empty root or child GameObjects and deleting entities**, from the
  hierarchy. Creation assigns a stable scene ID immediately, and creating a
  child opens its parent. Deleting takes the whole subtree, and **undo brings
  it back at the same handle** — so the
  selection and every earlier edit in the history keep pointing at what they
  named. That works because the history undoes in order: reaching a delete
  means everything after it is already undone, so the slot it freed is free
  again
- **Adding and removing components.** Add Component groups every type the
  entity's space accepts under Rendering, UI, Physics, Grid or Behaviour — by an
  authored table rather than by the type name's namespace, which is a naming
  scheme and not a taxonomy — and a family holding one offer is listed at the
  top level rather than hidden behind a heading. Only the components that
  *place* something are exclusive to a space, so a UI element can be given the
  script that drives it. It disables the ones that cannot be added yet, each
  saying why: no font in the project, no sliced sprite to build a clip from, no
  tilemap on this entity to navigate. Camera is among them on a scene that
  already has a world camera — a second authored one is a hard extract error, so
  offering it was a button that broke the scene in one click. Text and sprite animation
  are completed at the editor boundary: a project font gives UI Text a valid
  visible default, while a Sprite component whose texture has named sheet
  sprites gives Sprite Animation its first one-frame clip. Both are undoable.
  The menu offers one space or the other: an entity carrying `sindri.ui.*` is on
  the viewport and is not also offered a world sprite, and the reverse
- **A transport that says what it does.** One button enters and leaves play
  mode, labelled Play or Stop by which of the two pressing it will do; one icon
  pauses and resumes a scene already in play mode, and is disabled outside it;
  and a word beside them reads Editing, Playing, or Paused. Ctrl+P and
  Ctrl+Shift+P are the same two actions. Stop puts back everything playing
  changed, from the world as it was when Play was pressed rather than from the
  authored file, so pressing Play never costs an unsaved edit
- **Text authoring.** A `sindri.ui.text` component gets a multiline content editor
  and a picker listing the project's font assets, spelled as the scene resolves
  them. Existing missing font
  references remain visible and are called out instead of silently replaced
- **Sprite-sheet and animation authoring.** Selecting a texture opens its image
  slicer for grid dimensions and cell names. A `sindri.animation.sprite`
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
- Scene-view Select, Move, Rotate, and Scale tools on Q/W/E/R. Handles use the
  exact rendered camera, support local/world orientation and optional
  translation/angle/scale snapping, respect Z lock, and merge a whole drag into
  one undoable command-history step
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
- **A preview for every kind of asset the browser lists.** An image opens the
  slicer, a text file is read in a monospace column, a clip plays on demand, and
  a font draws a sample in the face itself. The last two are what a filename
  cannot answer: which of four `.wav` files is the pickup, and which of four
  typefaces suits a score. The clip plays through the editor's own audio device
  rather than the scene's, so auditioning one needs no running world and cannot
  leave a voice behind in it
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
under use and what the editor cannot express at all.
`docs/editor-authoring-audit.md` is the second sweep, which asks the harder
question: whether the controls that do work add up to a tool the companion game
could be built in. They now do — every finding it made is fixed except the six
right-click surfaces it tabulates, which are places to put actions that already
exist rather than gaps in what the editor can express. This is the summary, and
it is deliberately short: everything the audits found is either working or gone,
and what is left here is waiting on a build rather than on a handler.

- **Play and Pause** — they run sprite animation and Decay scripts. No other gameplay
  is stepped, so the demo's own turning cube does not turn
Removed rather than left drawn, because each was a promise about a feature that
does not exist: "Scene", "Build", "Tools", and "Help",
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
- Text is currently a UI component only; world labels, wrapping/alignment
  controls, and rich spans remain
- **One mesh primitive: `Cube`.** No quad, sphere, or glTF import
- **No audio.** Now scheduled in `ROADMAP.md`; it previously had no item at all
- Physics exists only as the masked 2D runtime foundation. No scene components,
  editor authoring, Decay access, Gather integration, or exercised 3D runtime
  exists yet
- No particles or authored parallax system. Renderer-free footprints,
  bounded occupancy, placement validation, symmetric wall edges, and
  deterministic A* now have authored engine components and a world adapter,
  but no editor, Decay, or Gather pathfinding integration yet
- No optional TypeScript embedding SDK; browser games currently expose narrow
  application entry points and run their gameplay in Decay
- No editor authoring of prefabs. The format, the spawn path, and the Decay
  surface all work, but making a prefab means writing the file: nothing turns a
  selected entity into one, and a `Prefab` field draws as the string it is
  stored as rather than as an asset picker
- No prefab instance link. A spawned entity does not remember what it came
  from, so editing a prefab does not update instances of it in an open scene
- Hot reload covers assets, not the scene file: editing a scene on disk while it
  is open is not noticed
- No deterministic system ordering

### Editor

- **File operations on the project.** A row's menu makes a folder, renames in
  place, copies beside itself, imports files from anywhere into the project, and
  deletes — with the directory re-read afterwards, so Refresh is for changes made
  outside the editor rather than for its own. None of them undo: the history
  describes a world and these describe a directory. What stands in for it is
  that each is checked before it runs and refuses rather than overwrites, that
  nothing can name a path outside the project, and that deleting asks first.
  Renaming the open scene follows it, so the next save does not write it back
  under its old name
- A component's fields come from the registry, so one added in the editor is the
  same component the shipped scenes use. A type with no honest blank —
  `sindri.ui.text`, `sindri.animation.sprite`, `sindri.grid.occupant`,
  `sindri.audio.source` — is completed from the project beside the scene, and is
  offered only when the project holds what it needs
- Play mode is read-only. Nothing that writes to the world or the file is
  available while a scene is running, because Stop restores the world as it was
  when Play was pressed. Editing a running scene and keeping the changes is not
  supported
- No first-class project model or multi-scene workspace. The Project dock reads
  the directory containing the open scene, folds its folders, scopes the listing
  to one of them, marks both its own selection and the open scene, copies an
  asset's path, and makes, renames, copies, imports and deletes files. Every
  text file it lists opens in the inspector, read-only — a `.decay` script, a
  scene, a sheet, a README — and a script can be made from a row's menu. The two
  kinds that are not text have a preview of their own: a `.wav`, `.ogg` or
  `.mp3` plays on demand through the editor's own audio device, and a `.ttf` or
  `.otf` draws a sample in the face itself
- Context menus exist on the two panels that list things — a hierarchy row and
  a project row — and nowhere else. Empty space in either panel, a component
  heading, a property row, the Scene view, and a console line all still ignore a
  right-click, so the actions that belong there (paste, reset a field, frame
  all, copy a message) do not exist
- No copy/paste of entities or of components
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

**And a script can draw numbers a run can be replayed from.** `Random.value`,
`range`, `int`, `pick` and `seed` read the host's stream, which a seed
completely determines: the same seed and the same sequence of calls give the
same numbers in the editor, in a native build, and in a browser. Exercised in
`crates/sindri-decay/tests/a_script_draws_a_number.rs`.

**And a script can change what the screen says, and read what was clicked.**
`Ui.set_text`, `set_number`, `set_numbers` and `set_fill` write into the element
the entity already carries; `is_hovered`, `is_pressed` and `is_held` answer about
the pointer. The scene owns the words and the script owns the numbers, because
Decay has no way to build a string — a designer authors `"Score: {}"` and a
script fills the slot. Exercised in
`crates/sindri-decay/tests/a_script_drives_a_hud.rs` and
`a_script_reads_a_button.rs`.

**And a script can drive a body and be told what it touched.**
`Physics.set_velocity`, `apply_impulse` and `velocity_x`/`_y` act on an entity's
body; `collision_started`, `collision_stopped`, `sensor_entered` and
`sensor_exited` answer with the entities this one touched during the last step,
as an `Array<Entity>`. An event is about a pair, so the answer names the other
half — whichever side of the event this entity was on. Despawning either half
from inside the answer is safe. A host running no physics refuses the call
rather than reporting a velocity of zero for a body that does not exist.
Exercised in `crates/sindri-decay/tests/a_script_drives_a_body.rs`.

**And a script can tell where the person is pointing.** `Pointer.x`,
`Pointer.y` and `Pointer.inside` read the position and whether there is one;
`Pointer.is_down`, `just_pressed` and `just_released` take a button name.
`Touch.count`, `Touch.x` and `Touch.y` reach the individual fingers. A button
name nothing answers to is refused exactly as a key name is, and asking for a
finger that is not down is refused rather than answered with zero, which would
read as a finger in the corner of the screen. Exercised in
`crates/sindri-decay/tests/a_script_reads_the_pointer.rs`.

**And a script can ask about several.** `World.with_tag` answers with an
`Array<Entity>` — every active entity carrying an authored `sindri.tags` tag,
in deterministic world order, bounded at 8192 and refused rather than truncated
past it. A tag says what an entity *is*, which is the question a game that makes
its enemies as it goes actually has: they have no authored names for `find` to
match, and asking by component type would put `sindri.sprite` in gameplay code.
The answer is a snapshot of handles, so an entity despawned mid-walk leaves one
that `World.exists` answers false for. Exercised in
`crates/sindri-decay/tests/a_script_asks_for_a_group.rs`.

**And a script can make one.** `World.spawn` takes a typed `Prefab` — an asset
reference the scene authored into an `@export` field, not a string in the
source, which is what lets the editor resolve it and load the document before
the frame that needs it — and answers with a generation-checked reference to the
new root. Overrides are the ordinary writes through that reference;
`World.set_parent` moves it, and `World.set_property` authors a per-instance
starting value that reaches the spawned script before its first callback. A
spawned script starts within the same pass, so a bullet fired during an update
moves during that update. Both the cascade that allows and the number of
entities one pass may create are bounded and reported rather than run.
Exercised in `crates/sindri-decay/tests/a_script_makes_an_entity.rs`. Not yet
exercised as gameplay: `games/orbital-last-stand` is the project that will,
and until it does this is a working surface rather than a proven capability.

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

- No ranges, so `for` walks a collection and nothing else. `while` and `for` are
  both bounded by the operation budget alongside the call-depth limit
- No array literals, no `push`, and no way to write into an element:
  `Array<T>` exists, and only a host makes one. No maps, closures, or
  first-class functions
- No query by more than one tag at a time, and no measured cost for a query at
  combat density
- No standard library; not even `math`
- Despawning is not undoable — no script write is, and play mode restores from a
  snapshot, so routing it through `WorldCommand` stays open
- No general component surface beyond `sindri.sprite`; tilemaps are available
  only through the deliberately narrow `Grid` coordinate API
- No scroll wheel, and no gamepad. The platform tracks scroll; nothing has
  needed it from a script yet
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

### Scripted pathfinding

Decay can query and advance authored grid occupants through deterministic A* with `Grid.can_reach` and `Grid.step_toward`. The host delegates to the same `WorldGridNavigation` adapter used by engine tests, so walls, occupancy, and whole footprints retain one meaning. Gather's Wisp exercises that path at runtime.