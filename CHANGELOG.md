Warning: truncated output (original token count: 25825)
Total output lines: 1233

# Changelog

All notable changes to Sindri Next will be documented here.

## [Unreleased]

- Add editor wall/footprint authoring, typed Decay pathfinding, and a Gather Wisp that proves authored A* navigation end to end.

### Added

- **Orbital Last Stand's five weapon flags are real build-changing mechanics.**
  Guidance rounds seek, arcs jump across targets, nova kills hit an area, gravity
  anchors leave delayed mines, and prism impacts continue as piercing beams.
  Each is authored from ordinary prefabs, collision masks and Decay rather than
  an engine-owned weapon system. `World.property_number` lets a target read the
  immutable damage carried by the projectile that touched it, while
  `World.has_tag` classifies one collision handle without a full-world query.
  The existing `shots`, `crit`, bullet-speed and bullet-size stats are wired
  into firing as part of the same combat contract.

- **The Scene view draws UI as a canvas in the scene.** The overlay is pinned to
  the viewport, which is right for a game and wrong for a place you arrange one:
  panning and zooming moved the world and left the UI stuck to the glass.
  `UiCanvas::InScene` makes it a rectangle in the world at the game's shape, with
  its edge drawn, its clicks resolved through it, and its gizmos on it.

- **The Game view can be any screen.** A `Screen` picker draws it at a chosen
  shape — desktop, laptop, tablet, phone in either orientation — rather than at
  whatever shape the panel happens to be. The overlay is as wide as the aspect
  ratio, so the shape decides the layout: a menu arranged in a wide editor panel
  runs off the side of a phone, and there was no way to find that out without
  building for one. The chosen shape is what the engine is handed, pointer
  included, so a button previewed at phone size is clicked where it is drawn.

- **Scene format 9: a font size is a share of the screen.** Everything else
  about a screen element was already in the overlay's units — two tall, centred
  — including the safe area, which is converted into them before anything is
  placed. Text was the exception, and it was the one number that decides whether
  a word can be read. A migration converts every existing scene, so what read
  correctly at 720 pixels still does, and now keeps reading correctly at every
  other size.

- **`sindri.camera` gains `fit`.** An orthographic camera framed by height shows
  a fixed amount vertically and whatever width follows, so turning a wide window
  tall takes the sides off the world. `"fit": "shorter"` makes the size a promise
  — this much world is visible whichever way the screen is turned. Defaulted to
  the old behaviour, so no scene changes meaning.

### Fixed

- **Orbital Last Stand no longer loses projectile damage after showing an
  impact.** Player bullets and secondary projectiles remain readable until the
  full collision pass has consumed their hit, then retire on their next script
  tick. Enemy durability therefore no longer depends on entity update order.

- **Orbital Last Stand's opening enemies no longer become hidden bullet
  sponges.** Their base health now preserves the original game's two-, three-,
  and two-shot opening ratios, and the original's slight health scaling waits
  until three minutes instead of starting on the first frame.

- **Orbital Last Stand no longer dumps enemies in numbered waves.** Regular
  enemies now arrive one at a time on a gradually tightening timer, with later
  enemy families joining by elapsed time. The obsolete wave counter is hidden
  from the runtime HUD, while Warden encounters still clear the arena and pause
  regular spawns.

- **Text on a canvas in the scene did not scale with the view.** An element's
  position came from the projection and so followed the camera; its size was
  worked out from the viewport and so did not, which meant zooming moved the
  words around without making them any bigger. What one overlay unit is worth in
  pixels is measured through the projection now, and the editor's pick box is
  measured the same way so a click does not drift as the view zooms.

- **A browser canvas was a letterbox.** The host asked for the window size its
  project configured, which on a desktop is a window someone can drag and in a
  browser is a fixed rectangle in the middle of a page — 960 by 540 on a phone
  held upright, with the whole screen around it empty. A page *is* the window in
  a browser, so the canvas is the page now, and follows it when it changes,
  which is what rotating a phone is.

- **A game.** `games/orbital-last-stand` is the second vertical-slice
  acceptance project the audit asked for, and it passes all twelve of its
  points: a scene, eight prefabs, and fifteen Decay scripts, with no Rust in it
  but a harness that assembles the same public pieces a host does. Ten
  simulated minutes run 26,072 kills and 228 concurrent entities at 18% of the
  frame budget — `docs/orbital-last-stand-evidence.md`.

  Its upgrade catalog is entities rather than a table: each card carries its
  own words, numbers and tag, the chooser asks `World.with_tag("upgrade")` and
  switches three on, and each card applies its own effect. Adding an upgrade is
  adding an entity. No engine concept of an upgrade was needed, which was the
  last thing the capability matrix still called **Missing**.

- **Prefabs reach a build.** They were discovered, loaded and handed to scripts
  by the editor and by nothing else, so `World.spawn` in a shipped game said
  the prefab was missing while the same scene spawned correctly in the editor.
  The manifest has `AssetKind::Prefab`, the export walks prefabs as documents —
  including a prefab only another prefab's script names — and the browser host
  loads them.

- **`World.set_active` and `World.is_active`.** `docs/scripting.md` said a
  screen is an entity with children and that showing one is switching it on.
  That was true of the engine and not of Decay: a title screen could be
  authored and never dismissed.

- **`Pointer.overlay_x` and `Pointer.overlay_y`.** `Pointer.x` is viewport
  pixels, and how many pixels tall a window is is not something a scene knows,
  so a script could say where the pointer was and not what it was pointing at.
  These are the same point in the overlay's units — where the UI is already
  laid out and hit-tested. They stop at the overlay rather than going on to the
  world, because going on means a camera.

### Fixed

- **The site served a game that could not open.** Pages built its manifest from
  a directory scan that recorded every asset as `Other`. That was harmless
  while nothing read kinds, and became a trap the moment the browser host
  started asking for its assets by kind: a manifest naming no scene is a
  project that does not load. The scan now takes a file for whatever its name
  says it is, and the site is assembled by the export rather than by hand — one
  answer to what a project ships, instead of two that disagreed.

- **An export shipped a game with no menus.** Its walks were the runtime's, and
  the runtime's walks are active-only — correct for drawing and stepping, wrong
  for an export, whose question is not what is running but what a project could
  ever switch on.

- **Scripts were not gathered by the export at all**, because `sindri.script`
  is not a builtin component.

- **A bullet could not be aimed on the frame it was fired.** A spawned script
  starts in the pass that made it, and the documented example is a bullet
  setting its own velocity in `start` — but a body is built when the scene next
  synchronizes. A velocity set before the body exists is now remembered and
  applied when it arrives.

- **An entity despawned by another script still ran.** A director clearing the
  field at the end of a run produced one dead-handle failure per enemy, none of
  them a mistake in the game.

- **The browser smoke test asked for one project's file names.** It would have
  passed for a game with no prefabs, no scripts and no sound. It reads the
  shipped manifest and requires one asset of every kind it names.

- **`decay/LANGUAGE.md` said `while` was the only loop** and that there was
  nothing to iterate, a hundred and fifty lines after documenting `for` over a
  collection.

- **A project can be exported to a static web directory.** `sindri-export`
  walks a scene for what it references — textures, fonts, scripts, sheets,
  audio — and writes a directory a static host can serve. An asset that stopped
  being used stops being carried, and one that started being used cannot be
  forgotten.

  The layout is what makes caching safe: `assets/sindri.manifest.json` is small and
  must never be cached, and it names an `assets/<content hash>/` directory that
  can be cached for ever. A changed asset cannot land in a directory anyone has
  already cached, and an unchanged build keeps its name so a re-deploy
  re-downloads nothing. Exporting again removes the previous build.

  `--base` bakes the deployment path into `<base href>`, with the trailing slash
  the export adds — `<base href="/repo">` resolves `pkg/host.js` against the
  site root and 404s, which is the whole GitHub Pages subpath problem.

  `[assets] include` in `sindri.toml` carries what a scene cannot name: a script
  plays a clip by a string inside a program, and no walk of a scene can see one.

- **`AssetKind` in the manifest.** The browser host carried a list of asset IDs
  per kind, compiled in — so adding a texture meant editing Rust, and a project
  the host crate had never heard of could not be exported at all. The manifest
  says what a project is made of, and the host reads it.

### Fixed

- **Scripts were not being gathered by the export**, because `sindri.script` is
  not a builtin component — scripting is a layer above the scene, and a host
  registers it. The export would have shipped a game with no code in it and
  looked like it had worked. Caught by a test that asks for every kind.

- **Editor Play runs the loop a shipped game runs.** It stepped once per
  *rendered* frame, so a scene was simulated as fast as the machine happened to
  draw — a play-test was evidence about the editor, not about the game. It now
  owns a `FixedStepClock` and runs gameplay a whole number of times per frame,
  in the order the engine fixes: effects, physics, screen UI, scripts,
  animations. Animations moved into the fixed step with everything else, because
  a clip advancing per rendered frame played at a different speed in the editor
  than in the build.

- **A held scene can be stepped once.** What it is for is the bug that happens
  in one frame and is gone before anyone can look at it.

### Fixed

- **An input edge reached every fixed step in a frame instead of one.** A key
  going down is one event, and gameplay runs in the fixed step — so a 30 Hz
  display driving a 60 Hz simulation fired every button twice. The edge is now
  spent by the first step that sees it.

  It also used to be cleared at the end of every frame, which at 144 Hz — where
  most frames earn no fixed step at all — dropped most clicks before gameplay
  saw them. Edges now survive until a step consumes them. Accumulated pointer
  motion follows the same rule, so two frames of dragging between steps sum
  rather than losing the first.

  This was wrong in the shipped host as well as the editor. Three tests cover
  it, and all three fail against the old behaviour.

- **Flecks that are not entities.** The audit asked for a pooled effect path and
  said to measure both approaches before choosing one. `docs/effect-scaling.md`
  is that measurement, and `cargo run --release -p sindri-scene --example
  effect_scaling` reproduces it.

  An entity per fleck costs 5.25 ms a frame at 8,000 of them — a third of a
  60 Hz budget — against 0.018 ms for the same population as plain values.
  Extraction is 95% of that, and over half of *it* is `serde_json` turning each
  entity's stored payload back into a struct, once per entity, every frame.

  So `Effects2d` holds flecks as plain values. A fleck has no identity a script
  can hold, no components, no place in the hierarchy, and nothing can collide
  with it — everything an entity is for, given up, because the alternative costs
  a third of a frame. What a burst looks like is authored as
  `sindri.effect.burst`, since count, speed, spread, lifetime and colour are a
  designer's numbers and a call naming all of them would be unreadable.

  Flecks draw their directions from **their own random stream**, never the run's:
  a fleck drawn from the gameplay stream would shift every number after it, so
  turning an explosion up would change which enemies spawned.

  The pool is bounded; past capacity the oldest fleck makes way for the newest,
  because the newest action is the one someone is looking at. `Effects.burst`
  answers with how many flecks it actually made, so a game can see it should turn
  itself down. Bursts batch with ordinary sprites by layer and texture, so one
  burst is one draw call rather than a second rendering path.

  Not built, and recorded as such: an instanced primitive renderer and custom
  materials, neither of which has a consumer; and the per-frame payload re-parse,
  which every ordinary sprite pays too and which needs a change to the component
  model rather than an effect system.

- **`SceneRuntime` bundles what a host keeps beside the world.** Animations and
  the fleck pool both decide what a drawable looks like, and each would otherwise
  be another parameter on every extraction entry point.

- **A game can remember things between runs.** `SaveStore` and `SaveDocument` in
  `sindri-core`, a `SaveBackend` trait in `sindri-platform` with file, browser,
  memory and deliberately-damaged implementations, and a `Save` namespace in
  Decay.

  The document is **flat** rather than a tree. Decay holds numbers, truths and
  text and nothing else, so a structure a script could not build is a structure
  nothing could write; `settings.volume` and `progress.best_wave` are keys, and
  the file stays something a person can read and repair. Keys are ordered, so the
  same state is the same bytes and a save can be diffed.

  **Three absences, told apart.** A first run starts cheerfully; a save that was
  there and would not parse is worth telling someone about *before* their
  progress is written over; and one written by a newer build is reported without
  being read, because a reader that guessed at a format it does not know would
  corrupt it the moment it wrote back.

  `FileSaves` writes beside the target and renames over it — a save half written
  is a save destroyed, at the exact moment someone's machine lost power mid-run.
  `BrowserSaves` uses `localStorage`, chosen over every larger browser store
  because the alternatives are asynchronous and a game should not have to ask
  whether its progress has landed yet.

  **Nothing in Decay touches storage.** How often someone's disk is written is a
  decision about their machine, so the store is in memory and the host writes it
  out on a cadence and before it stops. Writing the same value again is not a
  change. A NaN is refused outright, because it comes back next run and poisons
  whatever reads it long after the frame that produced it has gone.

  Editor Play keeps its save in memory and never writes it to disk: persistence
  can be play-tested, and pressing Play does not put a file in someone's project.

- **The script host dispatches by table.** Each namespace was six near-identical
  lines in one function, which had reached a length nobody reads. A namespace is
  now one line in `host/dispatch.rs`.

- **Numbers a run can be replayed from.** `sindri_core::Rng` is a PCG-XSH-RR
  64/32 generator written out rather than depended on. Every general-purpose
  crate reaches the operating system for a seed, which on
  `wasm32-unknown-unknown` means `getrandom` and a target that refuses to
  compile — and more to the point, entropy is the opposite of what this is for.
  A run that cannot be replayed from its seed is not seeded at all.

  Integer arithmetic throughout, with the one division by a power of two, so a
  seed means the same thing in the editor, in a native build, and in a browser.
  Fractions come from the top 24 bits, so `[0, 1)` is never `1.0`. Bounded
  integers reject the draws that would make low values slightly more likely,
  because modulo bias on a drop table over a long run is the kind of wrongness
  that gets blamed on the game design.

  Decay gained `Random`: `value`, `range`, `int` (both ends included, because
  "a number from 1 to 6" means six outcomes), `pick`, and `seed`. `pick` exists
  because Decay has no indexing — without it a script cannot choose from a group
  at all, and choosing from a group is most of what a game wants randomness for.
  Picking from nothing is refused rather than answered with an entity that is
  not there.

  Editor Play puts the stream back to its seed on every fresh start, so pressing
  Play twice gives the same run twice; resuming from a pause deliberately does
  not, since that would replay numbers the scene has already acted on.

  One stream is shared by every script, so a number drawn early shifts every
  number after it. That is documented rather than hidden — it is why a run's seed
  is worth storing while a frame's numbers are not.

- **A HUD a script can change, and buttons a person can press.** Screen text and
  images rendered, and no script could touch either — the audit called the first
  half "Decay cannot change text content" and the second "missing as a runtime
  button, focus, layout and navigation system".

  Text is now a **template**. Decay has no string concatenation and
  `decay/LANGUAGE.md` says so deliberately, so the scene owns the words and the
  script owns the numbers: a designer authors `"Score: {}"` and a script calls
  `Ui.set_number`. The words stay in the file where they can be read, reviewed
  and one day translated. `sindri.ui.image` gained a fill fraction and the edge
  it empties from, which is what makes a bar a bar rather than a picture of one.

  `sindri.ui.button` makes an element pressable, its rect being the entity's own
  transform. `ScreenUi` lays every element out and hit-tests the pointer, and
  `Ui.is_hovered`/`is_pressed`/`is_held` answer about this frame. A click is a
  press and a release on the same element, so sliding off before letting go
  changes a person's mind. Overlapping elements resolve by layer, so a modal is
  a modal because it is on top.

  **Screens needed no new mechanism.** A menu is an entity with children, showing
  one is switching it on, and `World.is_active` already governed a subtree — so
  there is no screen stack. `sindri.ui.layout` places a parent's active children
  in a row or column, which matters for the one thing anchors cannot do: a menu
  that loses an entry closes up around its middle instead of leaving a hole.

  **Nothing is silently withheld from gameplay** while a menu is up: which
  scripts are gameplay is not something a host can know, and a rule that guesses
  will guess wrong. `Pointer.over_ui` is the one line a gameplay script writes,
  and it is why a click on a pause button does not also fire the gun.

  The overlay is normalized — two tall, centred, running out to the aspect ratio
  — so one authored scene is responsive from a portrait phone to a wide desktop
  window with no breakpoint. A **safe area** takes a notch off the edges, moving
  anchored elements in while leaving centred ones alone.

- **`FrameContext` carries the viewport.** A fixed update had no idea what shape
  the screen was, which is not something a game laying out a HUD can not know.
  Desktop and browser hosts report it on resize.

- **`HostServices` bundles what a script can reach.** The host constructor had
  reached eight arguments; a caller with no physics and no screen UI now leaves
  two fields out rather than passing two `None`s in the right positions.

- **Physics reaches a game.** Rapier2D ran, and nothing could get at it: masks,
  shapes, bodies and events all existed, with no way to author a scene that used
  them and no Decay access at all. `ScenePhysics2d` is the join — it builds the
  simulation from `sindri.physics2d.*` components, keeps it in step as entities
  are spawned, switched off and despawned, and writes what physics decided back
  into the transforms the renderer reads, leaving the authored Z alone.

  Decay gained `Physics`: `set_velocity`, `apply_impulse`, `velocity_x`/`_y`, and
  four event queries — `collision_started`, `collision_stopped`, `sensor_entered`,
  `sensor_exited` — each answering with the entities *this* one touched, as an
  `Array<Entity>`. Queries rather than callbacks, because the language now has a
  value that holds several entities and a lifecycle function would be a second
  way for the host into a script. A projectile that should hit each target once
  gets that from `collision_started` without keeping a list.

  Editor Play and the game session both step physics before running scripts, so a
  script observes the events of the step that just happened. A host running no
  physics refuses a `Physics.*` call rather than reporting a velocity of zero for
  a body that does not exist.

- **`Scripts::advance` takes a `ScriptFrame`.** The parameter list had reached
  six and every capability the scripting surface grows adds another. A caller
  that offers no prefabs or no physics now leaves a field out rather than passing
  something empty in the right position.

- **A script can read where the person is pointing.** Decay could read the
  keyboard and nothing else, which is enough for a game driven by arrow keys and
  no use to one that is mouse- and touch-first. `Pointer` is one namespace for
  both: the position is the mouse when there is one and the first finger
  otherwise, and `is_down("Left")` is the left button *or* any finger — so a tap
  and a click are the same line of gameplay, and a game written for a mouse
  works on a phone without a second code path. `Touch` is the raw fingers for a
  game that wants a second one.

  The platform gained touch behind the same boundary as the keyboard, bounded to
  ten fingers, ordered stably so a drag cannot jump between them, and let go of
  when a window loses focus. The editor routes it all through the Game view in
  **that view's own pixels**, so a script reads the same position in Play that it
  reads in the real build; a pointer over the inspector is reported as gone
  rather than clamped to the edge. A tap now also unlocks browser audio, which a
  list naming only keys and mouse buttons had left silent on a phone.

  There is no drag abstraction, deliberately: a deadzone and a radius are tuning,
  and baking one game's numbers into an engine is how an engine acquires a genre.

- **A script can ask about a group of entities.** Decay gained `Array<T>`: a
  fixed-length collection with `for … in`, indexing, and `.len`, which only a
  host can make — there is no literal, no `push`, and no way to write into an
  element, and that is what lets a host bound one. `World.with_tag` answers with
  the entities carrying an authored `sindri.tags` tag, active-only, in
  deterministic world order, bounded at 8192 and refused rather than truncated
  past it.

  By tag rather than by name, because `World.find` matches the name a scene gave
  one entity and a game whose enemies are made as it goes has no authored names
  for them. By tag rather than by component type, because spelling
  `sindri.sprite` in a script would put engine internals in gameplay code.

  Indices stay the language's one numeric type; a fractional, negative, or
  out-of-range index is three different runtime errors, because they are three
  different mistakes. `decay/LANGUAGE.md` and `docs/scripting.md` are the
  contracts.

- **A script can make an entity.** Decay could find another entity, reach
  through it, check whether it still existed, and remove it — and could not
  create one, because creating one means saying what to create and the engine
  had nothing to say it with. A **prefab** is that: an authored reusable
  definition, stored as a single-root scene fragment in the same document shape
  a scene uses, so it carries every component a scene can carry and is
  validated, versioned, and canonically written by the same code.
  `World.spawn(prefab)` answers with a generation-checked reference, and
  overrides are the ordinary writes through it rather than JSON escaping into
  the language. `World.set_parent` moves one, and `World.set_property` authors
  a per-instance starting value before the spawned script's first callback.

  A prefab reference is a typed `Prefab`, authored into an `@export` field, not
  a string in a script's source — which is what lets the editor resolve it, load
  the document before the frame that needs it, and refuse a reference naming
  nothing. A spawned script starts within the same pass, so a bullet fired
  during an update moves during that update; the rounds that makes possible are
  bounded, and so is the number of entities one pass may create.
  `docs/prefabs.md` and `docs/scripting.md` are the contracts.

- **A project's main scene can be chosen.** `sindri.toml` nominates the scene a
  project opens on, and until now that field was written once when the project
  was created and then only editable by hand — a project whose first scene
  turned out to be a sketch opened on the sketch for ever. **Set as main scene**
  on any scene row in the project browser nominates it, the row that already is
  it says so on hover rather than offering an entry that would change nothing,
  and a scene made in a project that nominates none claims the empty place. A
  project that already opens on something is never silently re-pointed.

- **The editor opens a project, and asks which one.** A project is a directory
  holding `sindri.toml` — a format version, a name, and the scene opening it
  opens — and the welcome window is its own window, with the editor's hidden
  behind it until a project is chosen. It lists the projects you have opened,
  marks one that has moved or been deleted as missing rather than dropping it
  silently, and offers the two ways to get another: **New project**, which
  writes a manifest, a scene with a camera in it, and the folders assets resolve
  from; and **Open project**, which takes a folder that already is one. Gather
  ships with a manifest and is offered as a sample when the editor is run from a
  checkout. `docs/project-format.md` is the contract.

  A scene carries its project with it: one opened from the command line, a file
  dialog, or a browser row walks up to the nearest `sindri.toml`, so the project
  browser roots at the whole project and is headed with the project's own name —
  Gather rather than `assets`. A scene in no project still opens, which is what
  the editor did before projects existed.

  A launch honours the command line first, then the last project when the
  welcome window's footer was ticked to skip it, then the window. Reopening a
  project reopens the scene you were last in inside it, rather than its front
  door. `sindri-editor` with no argument now opens the welcome window rather
  than the demo scene compiled into the repository, which is also why
  `scripts/capture-editor.sh` names that scene explicitly.

- Decay has loops: `while`, with `break` and `continue`, bounded by a new
  operation budget so that a script which does not stop cannot take the editor
  with it. One call may run 1,000,000 instructions by default before
  `OperationBudgetExceeded`, and the host may change that.
- Decay has `%` and `%=`, a remainder whose sign follows its left operand.
- Decay accepts a chained `else if`, which previously had to be written
  `else { if ... }`.
- A Decay field initializer that reads a field declared below it, or reads
  itself, is now a compile error naming both fields. It used to compile and fail
  at runtime with a path name.

### Fixed

- **Every asset picker offered a path that would not load.** The project browser
  is rooted at the project, and asset references resolve against the directory
  the open scene sits in. Those are the same folder for a project the editor
  creates and two folders apart for one that keeps its scene under `assets/` —
  which is the layout the companion game uses. So the inspector read Gather's
  working `textures/orb.png` as a reference the project does not contain, marked
  it in the warning colour, and offered `assets/textures/orb.png` in its place:
  the correct path from the project root, and the one that makes the sprite
  disappear. **Copy asset path** on a browser row copied the same unusable
  string. The tree now knows both — where a file sits below the root, and how a
  scene names it — and every picker, every "this reference is not in the
  project" warning, and the copied path use the second. A file the loader cannot
  reach at all, such as the game's own `src/main.rs`, is offered nowhere rather
  than offered under a path that will not resolve.

  The tile and sprite palettes had the same fault from the other end: they read
  the image behind a reference by joining it onto the project root, so selecting
  Gather's floor showed a missing-file message where its tiles should be, and a
  sprite animation's preview showed one where its sheet should be. Both now join
  onto the directory the reference actually resolves against.

### Changed

- **The project browser lists the assets, not the whole checkout.** Gather's
  project holds a Cargo manifest, a `src/`, a `tests/`, and a web page beside
  the `assets/` directory that has its scene, art, scripts, fonts, and audio in
  it, and the browser listed all of it — so most of the rows in the panel named
  files no component can reference. The listing now starts at the directory
  asset references resolve against, and a switch in the browser's toolbar shows
  the rest of the project when you want it, remembered between launches. The
  switch appears only where the two listings differ: a project whose scene sits
  beside its `textures/` has nothing hidden and is offered no control.

- **A UI element could not be given the script that drives it.** `space::accepts`
  read the two families as symmetric, so a UI entity accepted `sindri.ui.*` and
  nothing else: selecting Gather's banner — a UI image driven by a script — and
  pressing Add Component offered one entry, UI Text. The banner could be opened
  and not rebuilt. Only the four components that *place* something are exclusive
  to a space, which `declared_space` already knew and `accepts` was not asking;
  the test that existed only checked a UI entity refuses a sprite, so a rule
  stated in two directions was tested in one.

### Added

- **Add Component is grouped.** Thirteen entries is a list you read rather than
  a menu you use, and it only grows. They sit under Rendering, UI, Physics, Grid
  and Behaviour now, from an authored table rather than from splitting the type
  name on its dots — the namespace is a naming scheme, not a taxonomy, and
  splitting it gives two one-entry submenus and five components with no family
  at all. A family holding a single offer is listed at the top level instead,
  because hiding a lone entry behind a heading is a click that buys nothing.
  That table also says which glyph each component draws with, so the two facts
  cannot drift: they already had, and audio sources, rigid bodies and colliders
  were all drawing with the generic entity box. A test asserts every registered
  component has a row, so the next one added to the engine fails the build
  rather than quietly arriving unfamilied — and audio stopped sharing the play
  glyph with sprite animation while the two were side by side in it.
- **An entity can be switched off without being deleted.** It was the last of
  the audit's smaller findings and it was an engine gap, not an editor one:
  nothing in the scene format said an entity could be inactive, so the editor
  had nothing to offer. `EntityData` and `SceneEntity` carry a `disabled` flag
  now, and `World::is_active` is the question anything drawing, stepping,
  scripting or picking asks — it walks ancestors, so switching off a HUD
  switches off its pips. The filter is applied once, in
  `ComponentSchemaRegistry::query`, rather than at each of the six places that
  would otherwise have to remember it. The flag is per entity and is never
  written down through a subtree, so re-enabling a parent brings back exactly
  the children that were on. Omitted from a saved scene when false, so there is
  no format change and every existing file is byte for byte what it was. In the
  editor: an Active switch on the inspector, greyed out and saying so on a child
  that is off because its parent is; Disable and Enable on a hierarchy row's
  menu, taking the whole selection; and a struck-through row for anything
  switched off, because dim already means "nothing here to act on" and this is
  the row you would switch back on.
- **A History dock, showing what Ctrl+Z will do and everything past it.** The
  history was answerable one step at a time, from a label on a menu entry nobody
  opens mid-edit, so "how far back can I go" had no answer and an edit made
  twenty steps ago that turned out to be wrong was undone by pressing a key
  twenty times and watching the viewport to see where you were. The dock is the
  stack drawn: "Scene opened", then every step in the order it happened, the one
  the world is at marked, and the steps already undone still listed under it
  dimmed, because they are still reachable. Clicking one travels there — by
  calling the same undo and redo the keys call, once per step, rather than by a
  jump of its own. `CommandHistory` gained `undo_steps` and `redo_steps`, which
  hand out labels and keep the transactions private: a caller holding one could
  apply it out of order.
- **A string can be clicked in the Scene view.** It was the last drawn thing
  that could not be, and the reason was real: a string is the one drawn thing
  with no size in the scene. What it covers is glyph layout — kerning, fallback,
  the wrap the viewport imposes — decided inside the text renderer, and a box
  guessed from the font size and the character count picks the wrong entity
  along its edges, which is worse than not picking at all. So the box is not
  guessed. `TextRenderer::measure` answers it from the same shaping the frame is
  drawn with, now one function shared by drawing and measuring, and
  `OverlayPlacement::text_origin` answers where the string starts from the same
  place the frame's text pass positions it. The editor measures at the
  resolution the view renders at and hands the boxes to picking, which stays
  free of the GPU and settles a string against an image by the layer rule two
  images already settle it by.
- **Siblings can be reordered by moving them rather than by renaming them.**
  Order was the stable ID sorted, so authoring order was alphabetical by a
  string most authors never look at: five pips made from one arrived as
  `pip-1`, `pip-1-copy`, `pip-1-copy-2`, and putting them in the order the HUD
  reads them meant renaming their IDs. Move up and Move down now sit on a row's
  right-click menu and on Alt+Up and Alt+Down, greyed out at the ends of a list
  rather than offered and refused. Where the order is recorded is the whole of
  the problem: a scene's document order is canonical and deliberately
  meaningless, so that a save stays stable while entities are added and
  reparented, and draw order is expressed by render layers and depths. So
  sibling order goes in the entity's editor-only section of the file, which a
  runtime carries but never interprets — no format change, and a scene nobody
  has reordered still lists exactly as it did. That needed one new command,
  `WorldCommand::SetEditorEntry`: the editor map had no write path, so nothing
  in it could be undone or mark a document unsaved.
- **More than one entity can be selected at a time.** Every bulk verb was
  impossible to express while a selection was one entity: deleting five pips
  meant five deletes and five undo steps, and moving a row of them meant
  dragging each one to the same place by eye. Ctrl-click adds and removes,
  Shift-click takes the range between two rows as the hierarchy is drawing them,
  and Ctrl-click does the same in the Scene view. Delete, Duplicate and a drag
  to a new parent then take the whole selection in one undo step, and so does a
  gizmo drag: each selected entity is moved, turned or scaled by what the one
  under the pointer was, from its own start, so a row stays a row. A selection
  has a primary as well as a set — the last entity pointed at — because a panel
  of fields and one set of handles can only be about one subject; the inspector
  stays on it and says how many the verbs outside it would take, and the rest of
  the selection wears a ring in the Scene view where its own handles would have
  been. Every bulk verb folds the set to its topmost entities first, because all
  of them already take the subtree: a parent and its child both selected would
  otherwise despawn the child's handle twice, land two copies of it, or move it
  by the parent's delta and then again by its own.
- **A clip can be heard and a font can be seen before either is named in a
  component.** They were the last two kinds the project browser could list and
  do nothing with, and both are decisions a filename cannot answer: which of
  four `.wav` files is the pickup, and which of four typefaces suits a score.
  Selecting an audio file offers Play and Stop, played by the editor's own
  audio device rather than by the scene — opened on the first clip rather than
  at startup, so it argues with nothing until asked, and a preview cannot leave
  a voice running in the world someone then presses Play on. Selecting a `.ttf`
  or `.otf` draws a sample in the face itself, at two sizes, over the letters,
  digits and punctuation a HUD actually uses rather than a pangram. A container
  nothing decodes is offered no play button and a `.ttf` that is not a font
  says so, because a preview whose whole job is to reveal what a filename hides
  should not hide it too.
- **A text file the browser lists can be read in the editor, and a script can be
  made there.** The project browser listed the language's own source files and
  could do nothing with any of them, in an engine whose headline capability is
  scripting. Selecting one shows it in the inspector now, the same way selecting
  an image opens the slicer: the file, its line count, and its source in a
  monospace column that scrolls both ways. Read-only, and it says so — an editor
  that opens a script in a text box is promising to be a code editor, and half
  of that is worse than none. It covers every text file the browser lists, not
  only `.decay`. **New script here** writes one that compiles and does nothing,
  because a file that reports an error before anyone has typed a line of it is a
  worse start than an empty one. Audio and fonts still have no preview: one
  needs playback on demand and the other needs the project's font in the
  editor's own text stack, and reading either as text says nothing.
- **The project browser can make, rename, copy, import and delete files.** Every
  asset used to have to arrive from outside the editor — there was no create, no
  folder, no rename, no delete, no duplicate and no import — so building a
  project meant a file manager beside the window and the Refresh button
  afterwards. A row's menu does all five now, and the directory is re-read
  afterwards, so Refresh is for changes made outside the editor rather than for
  it…5825 tokens truncated…y validation.
- Extensive checkable roadmap and architecture feasibility review.
- CI checks for formatting, Clippy, tests, and the declared MSRV.
- Shared `sindri-gpu` adapter/device negotiation and surface configuration policy.
- Target-independent `sindri-render` triangle pipeline.
- A single triangle example that targets native desktops and WebGPU browsers.
- Browser-target compilation in CI.
- Perspective camera matrices with projection math tests.
- Resizable depth targets and reusable indexed colored-mesh buffers.
- Depth-tested colored cube renderer with uniform-buffer transforms.
- A shared native/WebGPU cube example with frame-time-based keyboard rotation.
- Validated RGBA texture uploads with reusable texture views and filtering samplers.
- UV vertex layouts and a depth-tested textured cube pipeline.
- Procedural checkerboard texture proof shared by native and WebGPU targets.
- Offscreen color targets with aligned GPU readback and row-padding removal.
- Deterministic 512×512 headless cube capture, rendered through Mesa software Vulkan and uploaded as a PNG CI artifact.
- Aspect-correct orthographic camera with projection tests.
- Alpha-blended textured sprite renderer and a native/WebGPU cube-plus-overlay proof.
- Configurable opaque, straight-alpha, premultiplied-alpha, and additive sprite blending.
- Validated transparent draw keys with deterministic layer, back-to-front depth, and stable tie ordering.
- Dynamically growing instanced sprite batches with per-instance transforms/tints and draw-call statistics.
- Explicit frame extraction and preparation with validated viewports, clear operations, layers, and deterministic pass ordering.
- A versioned JSON scene that drives the shared cube and sprite-batch example through the same native, browser, and offscreen frame pipeline.
- Typed scene-component registration with metadata, schema validation, configurable unknown-component handling, and typed world queries.
- Native Rust editor shell with a styled hierarchy, interactive viewport composition, transform inspector, toolbar, and runtime status surfaces driven by the real demo scene document.
- Portable logical asset IDs, typed strong and weak handles, explicit load states and failures, duplicate-request coalescing, and reference-counted collection semantics.
- A cross-platform `sindri-assets` source contract with deterministic in-memory storage, root-confined native filesystem reads, and asynchronous browser Fetch API loading.
- A bounded asynchronous asset-load queue with native I/O workers, non-blocking browser future polling, generation-safe completions, duplicate rejection, and backpressure.
- Cross-platform PNG/JPEG texture decoding into upload-ready RGBA8 data, validated scene JSON decoding, and generation-checked completion application to typed asset stores.
- Shared-device editor viewport that renders the real prepared cube-and-sprite runtime frame into an egui texture, with drag orbit, zoom, resize handling, and a full-window CI screenshot artifact.
- Viewport-first editor workspace with a compact hierarchy, scene tool rail, real asset browser, inspector sections, Inter typography, and Material Symbols icons.
- Switchable perspective and orbit-matched orthographic projection for the editor's real WGPU scene viewport.
- Lossless world-to-scene saving that preserves authored stable IDs and reports runtime entities that have none.
- Deterministic ID assignment for runtime-spawned entities that skips identities already in use.
- Canonical scene serialization with sorted entities and keys, omitted empty sections, single-line scalar arrays, and fixed-point output.
- Golden scene fixtures with load, save, re-serialize, and idempotence coverage, regenerable through `SINDRI_UPDATE_SCENE_FIXTURES`.
- Namespaced editor-only metadata on scene documents and entities, carried through the runtime untouched and strippable for export.
- A scene migration API with forward-only, non-overlapping steps defined before format version 2 exists.
- Scene validation for non-finite transform values that JSON cannot represent.
- A deferred world command buffer whose commands each produce their own inverse, giving the core reversible edits rather than leaving undo to tools.
- All-or-nothing transactions that roll back applied commands when a later one is rejected.
- Bounded undo and redo history with labelled transaction grouping and an unrecorded zero-limit mode.
- Merge runs that collapse a continuous drag into a single undo step.
- Editor hierarchy, selection, and inspector driven by the live runtime world instead of a second copy of the scene document.
- Editor name and transform edits applied through world commands, with undo/redo on the toolbar and Ctrl+Z/Ctrl+Shift+Z.
- Editor play, pause, stop, and reset-to-authored-state driven by the real engine lifecycle state machine.

- A `sindri-platform` boundary crate defining what a host supplies: a clock trait, platform-independent input, and the loop that turns them into gameplay calls.
- Physical-key and mouse input with both held and per-frame edge state, ignoring operating-system key repeat and releasing everything on focus loss.
- A `Game` trait with fallible start, fixed-update, update, and stop hooks, and an `EngineHost` that reports which phase a failure came from.
- A manual clock and frame timer that make the whole loop testable with no window, GPU, or sleeping, including a frame-rate independence proof.
- Rational time scale that carries its division remainder between frames, so scaled time never drifts from the exact ratio of real time.
- Separate scaled and real frame deltas, so interface animation keeps running while the simulation is slowed or frozen.
- A `sindri-desktop` adapter translating `winit` keyboard, mouse, pointer, wheel, and focus events into platform input.
- A `sindri-scene` crate owning the built-in `sindri.camera`, `sindri.mesh`, and `sindri.sprite` schemas and deriving frames from a world, so no scene needs hand-written extraction code.
- Sprite batching per render layer instead of requiring every sprite in a scene to share one.
- Sprite anchors resolved against the overlay camera's extent, covering all nine corner, edge, and centre positions.
- Camera views that orbit, scale distance, and switch projection without touching the scene.
- A single shared colour target format, so offscreen and in-editor targets cannot disagree about colour space.
- A required sRGB swapchain format, reported as an error rather than silently accepted as a fallback.
- Verification that the headless capture actually contains the colours the scene authored, reporting the frame's dominant colours when it does not.
- Renderer texture handles and a registry, so one renderer draws every mesh and sprite in a scene instead of owning a single baked-in texture.
- Scene texture references resolved through bindings, making the `texture` field on mesh and sprite components load-bearing for the first time.
- Sprite batching split per texture as well as per layer, so a batch remains one draw call.
- A missing-texture fallback drawn in place of an unbound reference, and a report naming every unresolved reference in a world.
- The demo's badge as a real PNG decoded through the asset pipeline, connecting `sindri-assets` to `sindri-render` for the first time.
- Platform-independent asset URL resolution supporting relative, root-relative, and absolute bases, including static hosting under a non-root path.
- Percent-encoded URL path segments, so asset IDs containing spaces or non-ASCII resolve to the file they name.
- Rejection of URL roots carrying a query string or fragment, which would otherwise land in the middle of every asset URL.
- A single presentation surface policy deciding what a ready, suboptimal, outdated, timed-out, occluded, lost, or validation-failed acquisition each means, checked case by case without a GPU.
- A `WindowSurface` owning a surface, its configuration, and that recovery, so hosts acquire a frame or skip one and never write the decision themselves.
- An error for a surface that was lost and could not be built again, which is the one acquisition failure retrying does not fix.
- Apache 2.0 and MIT licence files, which every crate manifest has claimed since the workspace was created.
- Contributing guide and code of conduct, recording the conventions the codebase already follows.
- Dependency policy enforced by `cargo deny`, covering licences, sources, wildcard requirements, and security advisories across all four supported targets.
- A weekly scheduled advisory check and Dependabot updates, so a dependency problem surfaces without waiting for someone to open a pull request.
- A versioning policy for crate versions and the scene format, naming the editor protocol and npm SDK as deliberately undecided until they exist.
- A windowed host in `sindri-desktop` owning the window, event loop, device request, frame timing, and input, so an application supplies only how to build itself, what to do with a frame of time, and how to draw.
- A clock the host reads on both native and browser targets, replacing hand-measured frame deltas.
- Fallible application hooks, so a failure during a frame stops the host and is reported rather than logged and drawn over.
- A `verify` binary that reads back a captured PNG and holds it to the colours the demo scene authors, decoded through the engine's own texture decoder.
- Colour verification of the editor screenshot in CI, which was captured and uploaded but never examined.
- Gameplay running through the engine's fixed-step loop, so the demo turns at the same rate at any frame rate, proved at 15, 60, and 144 frames per second.
- An editor that opens a scene file from disk, saves the world back to it in canonical form, and reloads it, with the open file and its unsaved state shown in the status bar.
- A working File menu and Ctrl+S, replacing menu labels that did nothing.
- An entity-scaling benchmark covering spawn, iteration, typed queries, teardown, and both directions of save and load at 1k, 10k, and 100k entities.
- A `ViewportTarget` owning a colour target, the two views a Sindri target is drawn into and sampled through, and the depth buffer rebuilt with it.
- Camera panning in the editor viewport, measured in fractions of the framed height so a drag moves the picture the same distance at any zoom and under either projection.
- A reset control returning the editor viewport to the camera the scene authored.
- Editor settings that survive a launch — the project browser's presentation, the viewport projection, and the open bottom dock — alongside the window geometry and panel sizes egui persists itself.
- A project browser list view showing each asset's name and kind, now the default.
- A game view rendering the real scene through the authored camera, so what the player would see sits beside what is being edited.
- Decay, an experimental gameplay language in the isolated `decay/` workspace, with a lexer, parser, semantic analyzer, portable symbolic IR, and interpreter that depends on no engine crate.
- Decay is the decided scripting direction for Sindri Next; no embedded language is adopted.
- Decay `let` and `var` bindings execute, and a binding declares its value rather than storing it.
- Decay blocks carry their scope into the IR and the runtime, so a shadowing declaration no longer replaces the name it shadows.
- A Decay call-depth limit, so runaway recursion returns an error instead of aborting the process.
- The Decay example the documentation leads with is executed by a test, against a recording host.
- Decay's CI checks that the language compiles for `wasm32-unknown-unknown`.
- `sindri-decay`, binding the Decay language to a world: a `sindri.script` component, a host that gives Decay's symbolic paths a meaning on one entity's transform, and a driver that runs every script once a frame.
- Authored `@export` properties, carried in the scene and applied to a script instance before it starts, with a property naming an unexported or undeclared field refused rather than ignored.
- Decay scripts loaded through `sindri-assets` and hot-reloaded from the same file watch textures use.
- The editor snapshots the world when Play is pressed and restores it on Stop, so a script writing to the world cannot cost unsaved work.
- `TextAssetDecoder`, for assets that are text.
- `decay/LANGUAGE.md`, a language reference stating the grammar, what does not exist, and which behaviours will surprise you, with its claims enforced by a test.
- `docs/scripting.md`, the contract for how a script reaches a world.
- `.decay` files listed as scripts in the project browser rather than as plain files.
- Typed host members in Decay: a host describes a named type's members, and a path through a described type is checked when the script compiles rather than failing on its first frame.
- Decay host types may have methods, checked for arity and argument types like any other call.
- Reaching for `this.helper()` on a script's own function is refused with the bare-name form to write instead, rather than failing at runtime as an unknown host path.
- Sindri's script surface is described once and read by both the analyzer and the host, with a test asserting the host answers every path the analyzer accepts.
- Decay scripts can read the keyboard: `Input.axis`, `is_down`, `just_pressed`, and `just_released`, with physical key names that ignore case and a name nothing answers to refused rather than read as never-held.
- Decay scripts can ask the frame its `Time.delta` and their own `Time.elapsed`.
- Decay scripts can read and write their entity's sprite tint and layer, reaching the stored payload so a scene still round-trips byte for byte.
- `print` from a Decay script, reaching the editor console tagged with the entity that said it.
- Physical key names on `sindri_platform::Key`, with `ALL`, `name`, and a case-insensitive `from_name`.
- The editor translates its own keyboard into engine input for running scripts, and lets go of every key when play stops or a text field takes focus.
- Editing any component's fields in the inspector, driven by the stored payload rather than hand-written rows, so a component the engine has never heard of is editable too.
- Component edits checked against the component's own schema before they become commands, so an edit that would stop it decoding is refused rather than written into a scene that then will not open.
- Adding and removing components from the inspector, both undoable, with Add Component offering only types the registry can create.
- Component schema registrations may carry the payload a fresh component starts as, validated when it is registered rather than when someone clicks Add.
- A script's `@export` properties in the inspector, drawn from what the script declared — name, type, and default — with an unset field showing its default and saying so.
- Decay sources compile when a scene names them rather than when it is played, so a broken script reports at the scene it was opened with.
- Creating and deleting entities from the editor's hierarchy, both undoable, with a delete taking the whole subtree.
- `World::spawn_at`, which creates an entity at an exact handle, so undoing a delete gives back the same `EntityId` rather than a new one that leaves the selection and the rest of the undo history pointing at nothing.
- `World::next_handle`, so a caller can know an entity's handle before creating it and a spawn command can be redone onto the same one.
- `WorldCommand::Spawn`, `Despawn`, and `Restore`, each producing its own inverse, with a restored subtree returning to its place among its siblings.
- `Game.get` and `Game.set`, a board of named numbers every script on a world shares, because Decay has no value that can hold an entity and a script that needs a fact from another one has nowhere else to leave it.
- Gather, the companion game, in `game/`: five orbs on a floor, a thing you drive with the arrow keys, and a row of lamps that fills as you collect them, with all four of its rules written as Decay scripts and none of them in Rust.
- A second deterministic capture, of the companion game part-way through a scripted run rather than at rest, uploaded by CI beside the cube proof.
- `sindri.tilemap`, a grid of tiles cut from one sheet and drawn from one entity, laid out orthogonally or isometrically, whose cells join the same sprite batches loose sprites use so a prop sorts among the floor rather than behind it.
- A check that every built-in component naming a texture is one hosts actually load, since a component missing from that list draws the magenta checker while everything else about it works.
- A Decay value that can hold an entity: opaque to the language, holdable and comparable, with `World.find`, `World.exists` and `World.despawn` beside it, so one script can read and write another entity's transform and sprite instead of leaving numbers on a shared board for it.
- `EntityId::to_bits` and `from_bits`, for handing a runtime handle across a boundary that carries numbers and nothing else — never for writing to a file.

- `SpriteSheetDocument`, a sidecar naming the parts of one texture at an ID derived from the texture's own, so a sliced image says how it is cut in one place rather than in each component that draws it.
- `SpriteRef`, which splits `textures/tiles.png#floor` into a path and a sprite name, keeping `AssetId` a pure path — `#` was already reserved so a fragment could not leak into a URL.
- A check that an animated sprite asks for the sheet its clips read, whose own reference names no part of one and so is invisible to anything looking for fragments.

- A sheet slicer in the editor: selecting a texture outlines every cell on the picture, columns, rows, margin and spacing are drags, a cell is named by clicking it, and saving writes the sidecar beside the image.
- Margin and spacing on a sheet grid, so a sheet packed with gutters — which is how sheets are exported, to stop filtering bleeding one frame into the next — can be cut as it actually is.
- Sprite rows under a sliced image in the project browser, collapsed until asked for, so a sixty-four frame sheet does not flood the listing.

- `scripts/browser/smoke.mjs`, which loads a wasm-pack build in a real browser and fails when the page does not start the engine.
- `docs/browser.md`, recording what the engine's first run in a browser found.
- A browser build of the companion game, which is playable there: the keyboard drives the player, an orb is collected and a lamp lights, so Decay, entity references, the blackboard and input all run on the browser target for the first time.

### Changed

- Renamed the components a subsystem owns to hierarchical, subsystem-owned IDs
  as scene format 5: `sindri.grid.navigation`, `sindri.grid.occupant`,
  `sindri.animation.sprite`, and `sindri.audio.source`, matching the
  `sindri.physics2d.*` names 2D physics arrived with. Format-4 scenes migrate
  their keys on load with their payloads untouched, and a scene carrying both
  spellings of one component stops the migration by name rather than having one
  of them silently overwritten. Root-level singletons — camera, mesh, sprite,
  text, tilemap, and script — keep their flat names.

- Reconciled the README, roadmap, capability inventory, scripting contract, and contributor guidance with the implemented Decay entity references, tilemaps, editor authoring, companion-game browser build, and Editor + Decay product direction.

- Made a surface that offers no sRGB format draw through an sRGB view of one it can hold, instead of refusing to start; a browser canvas offers no sRGB format at all, so the engine had never once started in one.
- Made a host log the failure it records, since in a browser `run` has already handed the event loop to the page and there is nobody to return an error to — the engine was stopping at the device request in silence.

- Moved `sindri.sprite`, `sindri.sprite_animation` and `sindri.tilemap` onto named sprites as scene format 4: `uv_rect` and both sheet grids are gone, clips list names, and a tilemap's cells index a palette of names. The migration recovers a rect's cell without being told the grid, and stops rather than guessing when a rect is not a whole cell of one.
- Made a playing clip decide which part of a sheet is drawn on its own, so a frame that resolves to nothing draws the whole image rather than falling back to the clip's first frame, which was a plausible picture of the wrong moment.
- Gave the companion game one function that binds its textures and sheets, instead of one copy per binary; the window had sheets and the capture did not, which drew every sprite as its whole sheet.

- Rebuilt the companion game's orbs to ask the player where it is rather than compare against two numbers it published, which is what the blackboard was standing in for; the board keeps the score, which is a fact about the game rather than about an entity.
- Rebuilt the companion game's floor as one tilemap instead of 49 sprite entities, taking the scene from 68 entities to 20 and from 45KB to 12KB, and its hierarchy from a list you scroll past 49 rows of floor to one that fits on a screen.

- Gave every sprite batch its own uniform and instance buffers, so each draws with its own camera and its own instances; they shared one set, and because `queue.write_buffer` stages a write until the queue is submitted, every batch in a frame drew with the last one's camera — which no proof noticed, since none of them had both a world and an overlay.
- Made an animated sprite that authored no rect of its own draw the first frame of the clip it is playing, rather than the whole sheet squeezed into one quad, so a scene shows a pose before anything has run it.
- Stopped the editor reporting a script that is still loading as a compile error, which put one permanent error per scripted entity in the console on every cold open; a script that will never arrive is still reported.
- Made `scripts/capture-editor.sh` take a scene path, so the editor can be photographed against a scene other than its fixture.

- Increased the MSRV from Rust 1.85 to 1.87 to use the current `wgpu` 30 release.
- Replaced the planned Tauri/React editor architecture with native `egui`, `egui-winit`, and `egui-wgpu` integration.
- Increased the MSRV from Rust 1.87 to 1.95 for the first `egui` release aligned with `wgpu` 30.
- Replaced the editor's painted viewport composition with the actual Sindri render pipeline while retaining the native UI overlays and controls.
- Added a bounded X11 window-capture lifecycle for reliable full-editor WGPU screenshots in Xvfb CI runs.
- Rewrote the shared demo scene asset in canonical form; the extracted frame and draw order are unchanged.
- Replaced the cube example's hand-rolled key bitflags with the shared input state and the `winit` adapter.
- Replaced the demo's procedural badge with `assets/textures/badge.png`, decoded at runtime; the image is byte-identical, so the rendered frame is unchanged.
- Moved browser asset URL building out of the `wasm32`-only fetch source, where no test could reach it, into a tested module shared by every target.
- Removed the editor's duplicate left tool rail; the scene view toolbar already drove the same select, move, rotate, and scale modes.
- Replaced the cube example's bespoke extraction with the shared extractor, and moved its cube spin into the world so gameplay drives rendering through scene state.
- Fixed the editor viewport rendering into a non-sRGB target, which stored linear colour as if it were sRGB and made the scene far darker and more saturated than the offscreen capture of the same content.
- Replaced the editor's decorative transport cluster with working undo, redo, stop, pause, and play controls.
- Replaced the editor's hardcoded console and status text with the live entity count, engine state, and renderer error state.
- Fixed the editor hierarchy being clipped to the height of the adjacent tool rail, which hid most of the scene.
- Replaced editor status bullets and em dashes, which the bundled Inter subset cannot render, with a painted dot and in-subset punctuation.
- Replaced both proof examples' identical copies of the swapchain acquisition and recovery decision with the shared surface policy.
- Replaced the examples' panic on a validation error during acquisition with a skipped frame, leaving the device's error scope to report it.
- Gave the path dependencies of `sindri-platform`, `sindri-desktop`, and `sindri-scene` explicit versions, without which crates.io rejects a publish.
- Moved window creation, the browser canvas lookup, the four-state startup, and the async device request out of both examples and into the windowed host; the triangle example is now a quarter of its previous size.
- Replaced the cube example's hand-measured, hand-capped frame delta with the host's clock and frame timer.
- Fixed the editor viewport sampling its sRGB colour target through an sRGB view, so egui decoded a second time and every authored colour arrived far too dark; the scene render was correct and only the display of it was wrong.
- Moved the authored-colour expectation and its tolerance out of the capture binary so the offscreen capture and the editor screenshot are held to one definition rather than two.
- Moved the demo's cube rotation into a `Game` implementation driven by `EngineHost`, replacing a frame delta integrated by hand in the middle of rendering.
- Gave the world a single owner: a scene now carries its component schemas only, and extraction reads whichever world the engine or the editor holds.
- Moved input accumulation out of the windowed host into the engine host, so one `InputState` answers whether a key is down.
- Made the editor open the demo scene from disk rather than a copy compiled into the binary, falling back to the embedded copy and saying so when the file cannot be read.
- Made the editor screenshot wait for a frame with content, so a window grabbed before it had drawn no longer produces a blank capture.
- Fixed scene validation being quadratic in the number of entities, which every load, save, and canonical serialization paid: a ten thousand entity scene took about 1.4 seconds to validate and now takes ten milliseconds, and a hundred thousand entity scene completes at all.
- Moved the editor's viewport colour and depth targets onto the shared `ViewportTarget`, so the rule that kept them in the right colour space lives in the renderer rather than in one caller.
- Made the project browser's list and grid buttons switch the view; they were drawn but did nothing.
- Replaced the editor's game preview placeholder with a real rendered view, sharing the scene view's renderers and drawing only whichever view is visible.
- Added an editor fixture scene holding one cube, one sprite, and the two cameras they need, opened with `cargo run -p sindri-editor -- editor/assets/fixture.scene.json`.
- Added end-to-end editor tests that open the fixture, edit it through the command history, save, undo, and reload, proving an untouched scene saves byte for byte unchanged.
- Added a `Parent` menu to the editor's inspector, so an entity can be moved under another or out to the root as one undoable step.
- Added `World::check_set_parent`, which answers whether a reparent would be accepted without making it, so an interface can offer only the moves the command layer allows.
- Updated `glam` to 0.33, moving the camera matrices onto its new projection API.
- Added `perspective_projection`, `orthographic_projection`, and `look_at` to `sindri-render`, so the zero-to-one depth range and Y-up convention are chosen once rather than at each call site.
- Updated `pollster` to 1.0.
- Unpinned `png` to share `image`'s version, which stops every build compiling two copies of it.
- Added File → Open scene to the editor, so it can open a scene other than the one it was started on.
- Made the editor start with nothing selected, and let a click on the hierarchy's empty space or Escape clear the selection.
- Collapsed `Transform2D` into `Transform3D` so sprites and meshes share one world, as scene format version 2.
- Added the scene format version 1 to 2 migration, so scenes written by an earlier Sindri still open; the editor now migrates rather than parsing strictly.
- Added workspace layouts to the editor, chosen from the View menu and remembered between launches: `2 by 3` shows the scene above the game view with Hierarchy, Project, and Inspector beside them, and `Wide` keeps the previous single-view arrangement.
- Made the editor render the scene and game views at the same time in the `2 by 3` layout, rather than only whichever tab was showing.
- Fixed action failures being invisible: a failed save, open, or undo was overwritten by the next frame's render result within a frame of happening.
- Moved the frame clear out of the mesh pass and into `encode_clear`, so a scene with several meshes no longer erases all but the last, and a scene with none still starts from a cleared colour and depth buffer.
- Gave sprites a `space`: the default `screen` is the anchored overlay every sprite already was, and `world` places a sprite by its whole transform, draws it through the world camera, and lets opaque geometry in front of it hide it.
- Added the first GPU-backed tests, run in CI on software Vulkan, proving a world-space sprite is occluded by a mesh in front of it and drawn when it is the only thing in the frame.
- Made the editor's inspector show each built-in component's real fields instead of fixed text describing the demo scene.
- Sorted transparent sprites by how far they are from the camera rather than by a `depth` field authored beside them, with the render layer as the explicit override; the field is gone as scene format version 3, and the migration turns a screen sprite's depth into the Z that now orders it.
- Added `PerspectiveCamera::view` and `OrthographicCamera::view`, so where a camera is and what it looks at can be asked for without its projection.
- Added 2D-shaped accessors to `Transform3D` — `position_2d`, `set_position_2d`, `translate_2d`, `scale_2d`, `set_scale_2d`, and the turn about Z — so code thinking in two dimensions has a call that cannot flatten the Z a layered scene depends on.
- Added a Z lock a transform can declare, so an author can say that what layer a thing is on is deliberate: the command layer refuses a write that would move or drop a locked Z, and the inspector shows the lock and takes the Z drag away while it is on.
- Fixed the editor being unable to select an entity: a hierarchy row reported the surrounding layout's response rather than the row's, so no click ever reached it and every edit in the editor — names, transforms, parents, undo — was unreachable. A row now answers across its whole width.
- Fixed the editor refusing, and from the command line panicking on, a scene carrying a component its built-in schemas do not know: such a scene now opens, keeps the payload through an edit and a save, and shows its fields in the inspector.
- Made the editor ask before throwing unsaved work away: opening another scene, reloading from disk, discarding changes, and closing the window each name what they are about to lose and offer to save first, rather than doing it silently.
- Fixed the editor's Stop button discarding every unsaved edit; it now stops the engine lifecycle and nothing else, and is enabled only while something is running.
- Added `CommandHistory::revision`, which numbers the state the world is in, so a tool can tell whether the world still matches what it saved rather than tracking that a write happened.
- Fixed the editor claiming unsaved work after undoing back to the saved state, and claiming none after redoing away from it.
- Fixed Ctrl+Shift+Z performing an undo in the editor: egui ignores an extra modifier when matching, so the redo shortcut was consumed by the undo binding tested before it.
- Added `SceneExtractor::world_camera_view`, which answers where the world camera looks under a given view adjustment, so a viewport can draw its own chrome without extracting a frame or keeping a second copy of the orbit maths.
- Fixed the editor's axis indicator being painted at three fixed offsets: it now turns, foreshortens, and reorders its arms with the camera, which is the first thing in the editor that visibly answers where the viewport is looking from.
- Made the editor's project browser read the directory the open scene lives in, replacing eight hardcoded entries that named files no project contained, with the walk bounded in depth and count so opening a scene in a source tree does not stall the editor.
- Fixed the editor's asset search box accepting typing and filtering nothing: it now filters the browser to matching files, listed flat by their path below the root so a match is never indented under a parent the search removed.
- Added opening a scene by double-clicking its row in the project browser, which asks about unsaved work the same way every other way of leaving a scene does.
- Replaced the project browser's inert filter icon with a refresh that re-reads the directory, which is what a cached listing needs when a file appears beside it.
- Added an Edit menu holding undo and redo, each labelled with what it would undo; "Edit" was a label shaped like a menu that opened nothing.
- Removed the editor controls that were drawn and did nothing: the Select, Move, Rotate, and Scale tool modes and the `EditorMode` they wrote to, the "Scene", "Build", "Tools", and "Help" labels shaped like menus, the top bar's project name, the hierarchy's add-entity button, the inspector's Tag, Layer, and Add Component, the collapse chevrons and overflow menus that collapsed and overflowed nothing, and the settings gear.
- Replaced the editor's three fixed console lines with a real log: every failure, what each scene turned out to be when it opened, and every texture it names that nothing has bound, bounded and with a message repeated back to back collapsed into a count so a per-frame render failure cannot bury what explains it.
- Made the editor's error and warning counts count what the console holds, rather than saying "1 Error" for anything at all and never mentioning a warning.
- Made the editor reopen the scene it was last left in, overridden by a path on the command line, falling back to the demo scene and saying so when the remembered file has moved or been deleted.
- Named the open scene and its unsaved state in the editor's window title, so a task switcher can tell two editors apart.
- Widened the editor viewport's zoom from a factor of under three to a factor of four hundred and made the wheel move it proportionally, so a scene much larger or smaller than the demo can be framed at all.
- Added **Focus selection** to the editor viewport, on the toolbar and on F, which centres the view on the selected entity.
- Stopped an orbit being driven onto the camera's pole, where nothing says which way round the picture goes and dragging through straight down whipped the whole scene round to face the other way.
- Renamed `SceneExtractor::world_camera_view` to `world_camera`, returning a `ViewCamera` carrying the framed half-height as well as the view, which is the unit a pan is measured in and so what turns a distance on screen back into one.
- Added `AssetLoader`, which drives a store, a bounded queue, and a decoder in the one order that works, so loading an asset is a request and a poll rather than six steps that each fail quietly when skipped; requesting is idempotent, a failure is reported once rather than retried forever, and releasing says which assets went.
- Made the editor load the textures a scene names from the directory the scene lives in, through the real asset pipeline, so opening a project's scene shows that project's art; until now the editor bound two textures a demo crate handed it and drew the magenta checker for everything else.
- Added `referenced_textures`, which lists every texture a world draws with — the statement of what a scene needs loading, as against `unresolved_textures`, which is that list narrowed to what nothing has bound.
- Added `PROCEDURAL_TEXTURES`, one table of the textures the engine generates rather than loads, shared by the demo and the editor so two hosts cannot choose different colours for the same reference.
- Moved `encode_prepared_frame` and its target and renderer types out of the cube example and into `sindri-render`, which is where a stage that knows nothing about worlds or scenes belongs; the editor no longer depends on an example in order to draw.
- Made the editor's console wrap long lines, so an asset failure naming a path and an operating system error can be read rather than clipped at the edge of the dock.
- Gave the editor's fixture scene its own copy of the badge texture, so it resolves from its own directory like any other project's, with a test holding it to naming only textures that actually resolve.
- Added hot reload for native development: saving a texture the open scene uses shows the edit in the editor within about a second, without restarting and without blinking through the missing checker.
- Added `AssetWatch`, which notices that the file behind an asset changed by polling its modification time and length, and `AssetLoader::reload`, which loads an asset again because what is held is stale rather than because it failed.
- Made `TextureId` a generation-checked slot handle, so the texture registry can release a texture and reuse its slot while a handle nobody updated still resolves to the missing checker rather than to whatever landed there next.
- Made the editor release the GPU texture a reload or an edit replaced, which hot reload turned from a slow leak across a session into one per keystroke.
- Added image decoding compatibility tests that run on both native and `wasm32-unknown-unknown`, holding a corpus of every PNG colour type, sixteen bits per channel, an interlaced encoding, and a JPEG to the same pixels on both, so a texture cannot decode one way in the editor and another in the browser.
- Added `AssetManifest`, a versioned file recording each asset's length and the SHA-256 of its stored bytes, so a build knows what to publish and a load can check what arrived against what was promised.
- Made `AssetLoader` optionally verify arriving bytes against a manifest, turning a truncated response or a stale cache entry into an error naming the asset rather than a picture from last week; an asset the manifest does not list still loads.
- Made the editor pick up `sindri.manifest.json` from the directory a scene lives in, treating a malformed one as absent rather than refusing to open the scene.
- Added `docs/2d-inventory.md`, recording what each legacy 2D subsystem should become — port, refactor, replace, or defer — read from the legacy engine rather than from memory of it.
- Added a committed manifest for the demo's assets, with a test that regenerates it and compares, so editing an asset without updating the manifest fails there rather than in somebody's browser.
- Gave a sprite a checked `UvRect`, so it draws part of a texture rather than all of one and a sprite sheet becomes expressible; the rect rides on the instance, so every frame of one sheet stays in a single draw call, and a GPU test reads the pixels back to prove the shader honours it.
- Added `sindri.sprite_animation`, which cuts a sprite's texture into a grid, names clips of cells with their timing, and plays one; playback lives beside the world rather than in it, so watching an animation run does not rewrite the scene it came from.
- Made the editor's Play button actually run something: sprite animations advance while the engine is running, hold while it is paused, and go back to their first frame on stop, with the editor's fixture scene gaining a four-cell sheet that visibly spins.
