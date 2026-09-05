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
