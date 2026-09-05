# Changelog

All notable changes to Sindri Next will be documented here.

## [Unreleased]

- Add editor wall/footprint authoring, typed Decay pathfinding, and a Gather Wisp that proves authored A* navigation end to end.

### Added

- **Procedural 2D polygons can now use authored vertices.** `sindri.shape`
  keeps the existing regular-polygon path, but can also carry up to eight explicit
  2D points in the same instanced WebGPU renderer. Decay exposes the bounded
  `World.set_shape_point(index, x, y)` call, and its math host gains `exp()` for
  frame-rate-independent interpolation. Orbital Last Stand uses both to recreate
  the Strider's exact six-point hull and reference movement-facing response.

- **Orbital Last Stand now has reference elites and combat drops.** Regular
  enemies become eligible after 105 seconds using the original capped chance
  curve and one of five authored health, damage, speed and value traits. Normal
  difficulty's regular, elite and boss drop chances, missing-hull bonus and
  120-kill repair pity now produce repair, arena pulse and eight-second
  overdrive pickups. The repair amount preserves the reference proportion on
  the game's normalized five-hull scale.

- **Decay scripts can read the viewport aspect ratio.** `Viewport.aspect`
  exposes the screen's width divided by its height without exposing pixels or
  guessing which authored camera owns gameplay. Orbital Last Stand combines it
  with its `fit: shorter` camera framing to restore the original visibility
  contract: the ship only targets enemies at least partly on-screen, while
  off-screen Wardens and Chargers may approach but cannot begin attacks.

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

- **Orbital Last Stand's ship is no longer hit through a visible gap.** The
  player and hostile-shot colliders now author world-space radii matching their
  rendered scale, with a small player-favouring margin, instead of both using
  the unscaled half-unit default.

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
  change. A NaN is not a number JSON can represent and is refused rather than
  silently written as null.
