# Changelog

All notable changes to Sindri Engine will be documented here.

## [Unreleased]

- **Tile System 2 begins with correct global painter ordering and a stackable
  volume contract.** Transparent sprites are ordered across texture boundaries
  before adjacent equal textures are batched, so an object from one sheet can
  correctly appear between two objects from another. The accepted replacement
  for `sindri.tilemap` stores integer `(x, y, z)` cells, derives visible baked
  faces from their neighbours, and gives Gather a new identity as a
  Minecraft-style isometric world rendered entirely through 2D sprites.

- **Gather is now a farm rather than a nine-cell-wide plaza.** Its authored
  island is 25 by 25 cells, with three working plots, dry and watered soil, an
  irregular coast and pond, a connected path network, a flagstone farmhouse
  yard, boundary groves, field walls, standing stones, and four waystones. The
  old composition remains at the centre in world space, so this is an expanded
  place rather than a different one pasted over it.

  A Decay camera script eases after the active player so the larger map is
  actually walkable at the original readable scale. The shed now carries its
  own camera too: switching the opening scene off used to switch off its only
  camera, leaving the interior's scripts alive but its world impossible to
  render. An integration test now holds every playable place to exactly one
  active world camera. Both cameras use parallel projection: the ground's
  hidden depth offset can still keep transparent draws ordered, but can no
  longer turn into false parallax that makes props slide over their tiles.

  The oldest repeated landmarks have been redrawn at the same time. Trees now
  have a tapered trunk, roots, branching and a layered canopy; waystones have a
  weathered crown, luminous carved rune, moss and loose rubble; standing-stone
  clusters use irregular caps, varied faces and ground fragments. Their
  editable isometric-baker recipes remain the source of truth.

- **Gather has a second place in it, and a door between them.** The shed: a
  small room off the island's plaza, reached by walking onto a door and left the
  same way. The integration proof for everything above — a scene switched off
  rather than unloaded, so the island is exactly as it was on the way back,
  asserted with a mark written into the played world that no scene file
  contains.

  Each place has its own player. That is how the genre works, and it is what
  keeps a transition from having to carry an entity between scenes: continuity
  lives in the blackboard, and the door hands over the cell to arrive in. Which
  is also why arrival is taken in `update` and not `start` — walking back into
  somewhere already visited runs no `start` at all.

  `player.decay` now asks the map how big it is with `Grid.columns` / `Grid.rows`
  instead of clamping to the island's nine cells. The hardcoded bounds let the
  player walk straight off the shed the first time it opened, which is the kind
  of thing a second place finds immediately.

- **`World.find` prefers an entity that is in play.** Once a world can hold
  several scenes, a game with two places has a `Player` in each and only one of
  them is anywhere the player is; a lookup reaching the switched-off one would
  drive a player nobody can see, in a place nobody is.

  It prefers rather than filters, which is the load-bearing part. A switched-off
  screen is looked up by name all through Orbital's title and pause flow
  precisely so something can switch it back on, so a lookup that skipped what is
  out of play would make every one of those unreachable. `World.with_tag` still
  filters: a query answering with things out of play is a different question
  from a lookup of one thing somebody already knows the name of.

- **A scene can be loaded without renaming what is in it.**
  `LoadedScenes::load_keeping_identities` and `enter_keeping_identities`, backed
  by an empty namespace in `World::add_scene`.

  Found by wiring Gather's host for scenes. Routing its opening scene through
  the loader renamed every entity in the game — `shrine` became
  `gather.scene.json/shrine` — and two tests that identify authored entities by
  stable ID failed. The tests were right. Everything that resolves an entity by
  stable ID was written against the identities in the *file*: a test, an editor
  showing a running world, an authoring proposal naming a dozen entities. A
  runtime that silently renamed them would make the two disagree about what
  anything is called.

  So a project's own opening scene keeps its authored identities, and guest
  scenes are namespaced. The cost, stated rather than discovered: an
  unnamespaced scene has no room for a second one using the same IDs, and that
  is refused rather than resolved.

- **Gather's host plays scenes rather than a scene.** The session holds every
  scene the project can reach, which of them are in the world, and which one is
  being played; a request from `Scene.go` is performed after the frame, never
  inside it.

  The opening scene is loaded through the same loader as any other, under a root
  that can be switched off. A world whose first scene had been poured in flat
  would be the one place a game could never leave.

- **Gather's ground is rebuilt.** One `ground` sheet replaces `tiles`: three
  turf variants, a flowering one, tilled and watered soil, a path, flagstone
  and water. Groundwork for the farming game, and the first art authored
  against one shared style module rather than by copying numbers between
  recipes.

  Two things learned doing it, both about the fact that a tile's top is a flat
  upward face. The light therefore gives every tile the same value, so **a field
  of grass is one plane of one colour** — texture has to be geometry, which is
  why these tiles carry sub-grid detail rather than a second tint.

  And that detail has to *tile*. The first attempt scattered small domes, which
  the baker will not let cross a footprint, so they had to be inset — leaving a
  bare margin on every tile that laid a lattice of absence across the field.
  Worse than no texture, because it draws the grid. Boxes on a sub-grid that
  exactly divides the tile reach the edge without ever crossing it.

  Ground also carries **no outline**, where props still do. An outline
  distinguishes an object from its background; drawn on every cell of a
  continuous field it draws the grid instead. That is now a rule in the style
  rather than an accident.

- **A project can declare more than one scene, and every one of them ships.**
  `[project] scenes` in `sindri.toml` lists the scenes a game can reach besides
  the one it opens on, and `sindri-export` walks each of them exactly as it
  walks the main scene.

  Declared rather than discovered, which is the only list the exporter takes.
  A scene reached by `Scene.go("house")` is a string inside a program, and the
  exporter deliberately does not hunt for strings that resemble paths — a field
  declared `String` is text however much it looks like one, which is why
  prefabs are found through a script's declared types instead. Nor would
  `[assets] include` do: it ships raw bytes, so a scene listed there would
  arrive without its own textures, scripts or prefabs. That is an export that
  looks complete and opens a door onto nothing, and it is what the new tests
  are pointed at.

  `scene_id()` now returns the scene the project *names* rather than the first
  scene asset gathered, which stops being the same thing once more than one
  ships.

  The editor's manifest carries `scenes` without using it yet. It serializes by
  named field, so a field it did not model would have been dropped the first
  time anything rewrote `sindri.toml` — someone would have found their
  interiors missing from the next build with nothing to point at. Asserted
  through the real save path, because it is the write that loses a field.

- **A script can ask to be somewhere else.** `Scene.go(name)` and
  `Scene.current()`, with `LoadedScenes` keeping which scene a world is playing
  and switching between them. Together with `World::add_scene` that is a game
  with more than one place in it.

  The change cannot happen inside the call. The script asking is running *in*
  the scene being left, from a world the change would rearrange underneath it,
  so `Scene.go` records an intention and the host performs it between frames —
  the same shape `Audio.play` already has for recording a sound for whoever
  owns a speaker. `Scene.current()` therefore answers the scene being played
  and never the one asked for; a script that read back its own request would
  see the move happen a frame before it did.

  The first request in a frame wins. Two scripts asking is a conflict with no
  right answer, and taking the last would make a door beside a door depend on
  which script the pass reached first.

  Leaving a scene disables it rather than unloading it, so everything it holds
  is as the player left it on return. Whether that is what a game wants is the
  game's call, not the engine's: `load` and `enter` are separate verbs so a
  scene can be brought in without being entered, and `unload` is the one verb
  that discards played state.

  A host that plays exactly one scene passes no channel, and `Scene.go` says so
  rather than accepting a request nothing will perform — a game whose doors
  silently never open should be heard about on the first frame.

  `Scene.current()` answers a plain name rather than an optional one. Decay
  cannot compare a `String` with `null`, so a null would have been a value no
  script could test; a host that has a scene channel is playing something, and
  "playing nothing" is a state only its own startup can be in.

- **A world can hold more than one scene.** `World::add_scene` loads a scene
  beside whatever a world already holds, optionally under a parent entity —
  and because `World::is_active` walks ancestors, disabling that one entity
  takes the whole scene out of drawing, stepping, scripting and picking without
  touching anything inside it.

  This is the first part of `docs/parity.md`'s largest gap: one scene per
  project. Gather needs a farm and a farmhouse both *live*, because walking
  indoors has to leave the crops growing, and reloading the farm from its file
  on the way out would reset them — a scene file holds the authored state, not
  the played one. So a scene is added and switched off rather than loaded and
  dropped.

  The awkward part is identity. A scene's entity IDs are unique inside that file
  and nowhere else, so two interiors may each author a `door`, while
  `source_id_map` answers for the whole world. Every ID is therefore namespaced
  on the way in: `door` from `house` becomes `house/door`. A collision after
  that is a caller giving two scenes one namespace, and is refused rather than
  resolved — with every identity resolved before anything is spawned, so a
  refusal leaves the world exactly as it was.

  This is a runtime capability, deliberately. `World::to_scene` writes one
  document, so a world holding several scenes does not round-trip back into the
  files it came from; the editor still edits one scene at a time.

  Not yet: a project-level scene list, an editor surface, or anything in Decay
  that can ask for a change.

- **A Decay script can change the ground.** `Grid.tile` and `Grid.set_tile`
  read and write a tilemap's cells by column and row, and `Grid.columns` /
  `Grid.rows` give the map's size. This closes the last 🟡 on the tilemap row of
  `docs/parity.md`.

  Until now a script could only put entities *on top of* a floor, so anything
  where the ground itself changes — tilled, watered, grown, burnt, flooded — had
  to keep the picture and the game's idea of the picture in agreement by hand.

  A cell is named by whole column and row rather than by a world position,
  because the map is the authority on which cell a position falls in and a
  script doing that arithmetic itself would be a second answer free to disagree.
  Reading off the map answers `-1` rather than failing, since anything that
  moves will ask about the edge; writing off the map is an error, because it has
  no sensible meaning and dropping it silently would hide the mistake. So is a
  palette index the map cannot answer — a map holding one fails validation on
  the next load, long after and nowhere near the script that wrote it.

  The write edits the stored payload in place rather than going through the
  typed view, for the reason the sprite and shape paths do: the view is
  `Deserialize`-only, so rebuilding it would drop a field it does not model.

- **Gather's role is widened.** `AGENTS.md` said the showcase may only
  demonstrate capabilities the engine already has. It is becoming an isometric
  farming game held to the standard of a game somebody would choose to play,
  and it may now pull new engine capability — generally, not shaped around it.
  What still separates it from Orbital Last Stand is why a gap is found: Gather
  finds them by trying to be good, Orbital by trying to be faithful.

- **Every boss in Orbital Baked is its own boss now.** Ten of the twelve shared
  one prefab, one script and one sprite sheet, told apart by a `kind` number and
  a tint — so they were the same orange gunship in ten colours, and eight of the
  ten opened by orbiting the player and firing a ring. The vector original gave
  each of them a different polygon count; baking had flattened that into one
  hull.

  Each now has its own recipe, sheet, prefab and script, and the side count is
  back: five for the Harrower through sixteen for the Last Light. More
  importantly each has its own *verb*, because a boss that differs only in how
  many bullets it fires is not a different fight:

  | boss | how you fight it |
  | --- | --- |
  | Harrower | bait the charge; it is soft only just after it misses |
  | Prism | shoot on the beat — soft only while a facet faces you |
  | Singularity | it drags you in for the whole fight |
  | Crown | it heals unless it is being hurt now |
  | Brood | cannot be hurt at all while a child lives |
  | Mirror | every hit comes back; stop shooting |
  | Architect | it fills the arena with slow walls |
  | Spine | ten plates, each one you break is damage it stops soaking |
  | Leviathan | one turning beam; the safe ground is behind it |
  | Last Light | borrows the others' verbs in turn |

  The Warden and the Aegis kept the fights they had. The roster is asserted
  rather than trusted — its own script, its own sheet, clips that name frames
  the sheet holds, and a director that can reach all twelve — because a game
  runs perfectly well with every boss wearing the same hull, which is exactly
  how this happened.

- **Every baked rotation in both Orbital games was doing nothing.** A model
  part's `rotation` is three Euler angles in degrees, and all 822 of them across
  the nine `games/orbital-*` recipes were authored in radians. Read as degrees,
  a quarter turn became a nudge of one and a half degrees, so every part kept
  its position and lost its orientation.

  It failed quietly, which is why it survived: the models still baked, still had
  the right silhouette and the right size, and simply pointed nowhere. A ring of
  blades meant to face outward came out a ring of parallel planks. The drifter's
  spokes, the warden's cannons, the aegis's plates and the swept wings on the
  player, core and charger are all in the recipes and none of them reached the
  screen.

  The recipes are converted and re-baked, whole degrees snapped to whole degrees
  so the baker's exact quarter turns stay exact. The unit is now stated in
  `docs/isometric-baker.md` and asserted in `tools/isometric-baker/test`,
  including the specific case of radians-as-degrees, because a recipe with the
  wrong unit bakes successfully and looks merely mediocre.

- **Sindri will manage its own model runner.** A prebuilt llama.cpp server is
  about 31 MB and needs no installation at all — it is a binary unpacked into
  your own folder — where Ollama is a system service with a gigabyte installer
  and a permission prompt. Adopting it makes setup passwordless and removes the
  one step Sindri could not perform on your behalf. Ollama stays supported for
  anyone already running it; what changes is which runtime the guided path
  installs.

  Getting a file onto the machine now goes through a pinned manifest and a
  verification Sindri does itself. Transport is the platform's own downloader —
  `curl`, or `wget` where there is no curl — because a TLS client would have
  meant a crypto subtree in a crate graph that is meant to gain no HTTP stack.
  The only dependency this needs is `sha2`, which was already in the editor's
  tree, so the capability costs no new subtree at all.

  That is not a shortcut. A downloader reporting success has said nothing about
  *what* it fetched: a captive portal, a truncated transfer and a tampered
  mirror all look like a completed download to the tool that performed it. So
  downloads go to a `.part` file and the real name comes into existence only by
  a rename, only after the hash matched; a mismatch deletes the partial rather
  than leaving it for a resume to build on; a file already on disk that already
  verifies is reused, because re-running setup has to be free or nobody re-runs
  it; and `curl` is given `--fail`, or it exits zero on an HTTP error page and
  hands a 404 body to the hash check.

- **The assistant grades a model against your machine instead of answering yes
  or no.** Recommended, Supported, Best effort, or Will not fit — because "runs,
  but will drop a tool call halfway through a multi-step edit" is a real and
  common answer that a boolean has nowhere to put, and omitting such a model
  from the list reads as Sindri not supporting something you can plainly see
  running.

  How much memory a model needs is now **estimated** from its parameter count,
  quantisation and context length rather than hardcoded per model. Context is
  the part of the memory bill a person changes, so a fixed figure per model is
  wrong the moment they change it.

  The models themselves moved out of the code and into
  `editor/assets/ai-models.json`: a committed manifest, validated on load
  against a schema version, carrying the licence and source of everything it
  names. Adding a model needs no code.

  One entry is marked the standard, and that is what Sindri leads with wherever
  it is comfortable. Bigger is not better past the point where a model drives
  the tool loop reliably — beyond it the extra parameters cost context headroom
  and speed, which is a trade to make deliberately rather than a default to be
  handed.

  Hardware detection now asks `rocm-smi` as well as `nvidia-smi`. Asking only
  NVIDIA meant every Radeon machine fell silently through to system memory and
  was told it could run less than it can.

  The grading, the residency estimate and the manifest shape follow
  `vardirhq/local-code`, which solved the same problem against the same
  reference card.

- **Setting up the local assistant is a panel with one button in it.** An
  Assistant panel reads the machine and shows the one next thing to do: install
  the model runner, start it, download a model chosen for the hardware, check
  what it can actually do. Each state is a sentence and a button.

  **Nobody is asked to type anything.** Not a command, not a model name, not a
  path. The one exception is a password, and only where the operating system
  itself demands one — that prompt belongs to the OS and Sindri never sees what
  goes into it. A test asserts this against the action set rather than against
  the drawing, so a step added later that wants a name fails the build.

  The states are named rather than collapsed into "could not connect", and the
  order they are asked in is the diagnosis: not installed before not running
  before no model, because each later question is meaningless when an earlier
  one is the answer. While the next move is happening outside the editor it
  keeps watching, so finishing an install advances the screen by itself — there
  is no refresh button to find.

  A model is only suggested when the machine can hold it, weights plus room for
  context and overhead. Recommending one that will not load is worse than
  recommending none, because the person who takes the suggestion concludes that
  local AI does not work. A machine too small for anything measured is told so
  plainly and still offered the whole list to choose from.

  Finishing reports what was *verified* — structured answers, tool calling,
  scene inspection, Decay editing — rather than a green connection light, and a
  model that fails something every proposal depends on is called unusable
  instead of ready.

  Detection talks to the runner over the loopback socket using the standard
  library alone, so the editor gains no HTTP stack and an AI-disabled build
  stays a normal configuration. Downloading a model goes the same way: the
  runner does the fetching and Sindri asks it to. Installing the runner itself
  needs TLS, which the editor deliberately has no client for, so that step opens
  the pinned download page through the operating system and the probe watches
  for the result.

- **A world can resolve a scene's stable identities, and a transaction can be
  rehearsed.** Two small additions to `sindri-core`, both of which a host for
  the AI authoring protocol cannot be written without, and both useful on their
  own account.

  `World::entity_for_source_id` and `World::source_id_map` answer the direction
  that was missing. A world could mint stable IDs and write them out, but
  nothing could read one back, so every caller holding a serialized identity
  walked the entities itself and decided what to do about a tie. Anything that
  names an entity the way the *file* does — a saved selection, a prefab
  reference, an authoring proposal — needs this, because a runtime handle means
  nothing once the scene has been reloaded. The answer is unambiguous by
  construction: two entities cannot share a stable ID, so a reference resolves
  to one entity or to none.

  `Transaction::rehearse` runs a whole group against a copy and returns the
  world it would produce, or the error that would stop it, leaving the live
  world and the undo stack untouched. `apply` already rolls back a refusal, so
  this is not about safety — it is about being able to ask *what would this do*
  without the answer being visible in the editor for a frame, which is what a
  host showing a proposal for acceptance needs.

- **Ctrl+K finds anything.** One field that searches panels, the scene's
  entities, the project's files and scenes, the arrangements, and the editor's
  verbs — and does the thing when you press Enter. Arrow keys move, Escape
  closes, and a pill in the title bar says the shortcut exists, because a
  shortcut nobody is told about is a shortcut nobody uses.

  It came before any further canvas work on purpose: the canvas arrangement
  hides more than the docked one did, and until now the only way back to a
  closed panel was the View menu. A palette is what makes hiding a panel
  something other than losing it.

  Matching is by subsequence and scored, not substring — `oscn` finds
  `orbital.scene.json` — and the query is split into terms matched in any order,
  so `prefabs drifter` and `drifter prefabs` both work. What decides whether a
  palette is useful is the order results come back in, so the ranking lives in
  `palette/score.rs` with tests written as the orderings a person would expect:
  initials beat incidental letters, a prefix beats a match further in, a
  contiguous run beats a scattered one, and the shorter of two answers wins.

- **The scene view is the document.** The editor now opens in a `Canvas`
  arrangement: the scene fills the window, the hierarchy and the project browser
  overlay its corners, and the inspector stays docked at the edge. A panel can
  now be placed one of two ways — a **dock** takes room from the scene, an
  **overlay** floats over it, anchored to one of the scene's four corners — and
  the rule for reaching an empty one is that **edges dock and corners float**.

  Overlays anchor and stack rather than floating freely, so two sharing a corner
  sit one above the other and cannot be piled on each other by accident.
  Clicking the tab already showing rolls one up to its strip and back down,
  which is the answer to the honest objection to overlays: they cover the world.
  They are drawn opaque on purpose — a translucent panel is legible over a
  mockup's calm sky and unreadable over a dense tileset.

  The inspector is deliberately *not* floating. It would move on every selection
  so no muscle memory could form, it would cover the neighbours a value is being
  judged against, and a schema-driven entity is thirty fields deep. Verbs travel
  well; properties do not.

  The previous arrangement is kept as the `Docked` preset rather than archived,
  so changing your mind costs a menu click. `docs/editor-direction.md` records
  what this direction takes from the canvas-first proposal, what it declines and
  why, and the two things — liveness, and the change-to-consequence loop — that
  no mockup can show and that this must not be allowed to reorder.

- **The console reports the size of a problem rather than the size of its own
  log.** It collapsed a repeated message into a count only when the repeat was
  the entry *immediately* before it, and nothing fails on its own: a frame
  reports a rotation of failures — one per entity, one per script — so an
  identical pair is almost never adjacent and the collapsing almost never
  fired. Every frame added the whole rotation again. Opening Orbital Last Stand
  filled the console with the same handful of errors and put seventy-two on the
  status bar.

  A repeat is now counted against any matching entry in the window, and counted
  where that entry already sits rather than moved to the end: recurring failures
  settle with their counts climbing while anything new still arrives at the
  bottom, instead of the whole list churning once a frame. The status bar's
  count is of distinct failures, which is the number worth reading.

- **The editor's panels are arranged by the person using it.** Every panel —
  Scene, Game, Hierarchy, Inspector, Project, Console, History — is now a tab,
  and every tab is dragged into any of seven slots: two columns down each side,
  one along the bottom, and the centre split in two. Dropping onto a tab strip
  inserts between the tabs it lands between; dropping on a window edge opens a
  slot nothing was in. An empty slot is not drawn. The arrangement and each
  slot's size survive a restart, `View → Panels` reopens anything closed, and
  `View → Arrangement` offers the presets as starting points rather than as the
  only shapes the editor has.

  The Console opens beside the Scene view in the default arrangement. It used to
  share one bottom slot with the project browser and the history, so reading the
  log meant giving up the browser — and what the console says is usually about
  the thing in the viewport next to it.

  **This also fixes panels that could not be resized.** An `egui::Panel`
  persists the size of the rectangle its *contents* occupied rather than its
  own, so a panel whose contents were narrower than their slot shrank to fit
  them on the next frame — and, because the persisted size is where the
  following frame starts, stayed shrunk however far its edge was dragged. That
  is why the project column sat at its minimum width and would not move, and why
  every panel opened narrower than the default it declared. Each slot now claims
  its full width before anything draws, and the arbitrary maximum widths are
  gone: a slot may take up to three quarters of the window. `panel::fill_slot`
  carries a headless regression test for both halves of it.

  The tab strip has also replaced each panel's separate header. A docked panel
  used to spend a second row saying its own name directly under a tab that had
  just said it; a panel's own controls now sit at the far end of the strip that
  names it.

- **CI runs the same checks about three times faster.** The workspace's tests
  were the whole of CI's critical path, and most of that was one number: the
  repository set no Cargo profile, so a game engine whose tests are mostly
  simulation ran that simulation unoptimised. One Orbital test spent a hundred
  and ten seconds stepping six thousand frames. Optimising *dependencies* takes
  the suite from about 288 seconds to 97 -- most of that arithmetic is in the
  physics solver, which is a dependency -- with debug assertions and overflow
  checks untouched. Running it under `cargo-nextest`, which schedules every test
  in one pool instead of one binary at a time, and splitting the slowest test
  into the five screen shapes it was looping over take it further still.
  Doctests are run in their own step, because nextest cannot run them and
  dropping them silently would have been a real loss of coverage.

  Dependencies rather than the whole workspace is the part worth knowing.
  Optimising workspace crates too runs faster -- 97 seconds down to 30 -- and is
  still the wrong trade for CI, because a dependency is compiled once and
  restored from the cache while a workspace crate is recompiled on every run.
  Measured, that cost about 123 seconds of compilation to save 67 seconds of
  test time, so the first version of this change was a net loss in CI and the
  smaller speedup is the one kept. A developer running the suite repeatedly
  against an unchanged tree is in the opposite position and can add
  `[profile.test] opt-level = 2` locally.

  Two things this turned up. Adopting nextest exposed a test that was flaky by
  construction: it waited for a background loader by spinning a fixed ten
  thousand times, which measures nothing -- how many yields a worker needs to be
  scheduled depends on what else the machine is doing. It now waits on a
  deadline, which says the thing meant. And path-filtering CI by "docs only"
  was considered and rejected: sixteen markdown files in this repository are
  read by Rust tests, so a documentation change here genuinely can fail one.

- Add `tools/isometric-baker`, an offline asset baker that turns a 3D model into
  an ordinary Sindri sprite sheet and the `.sheet.json` beside it. The pipeline
  is adapted from IsoGame's Sprite Factory (MIT); the renderer is a dependency-free
  CPU rasteriser, so bakes are byte-stable and checkable in CI. It changes nothing
  about the runtime — Sindri still has one mesh primitive, no glTF import, no
  material authoring and no lighting system — and no game uses its output yet.
  It also generates a `.prefab.json` beside the sheet: a one-entity prefab with
  the transform scale that makes one baked pixel one intended pixel, so a baked
  asset can be used without custom game code. Generated documents are written in
  Sindri's canonical form, and `sindri-core` has a test that proves it.
  Gather's whole world is baked with it: floor tiles, the shrine, two waystones,
  a ridge of wall segments, trees and stone outcrops. The island is re-composed
  around them — a shore rim, a grass field, a path from the north landing to the
  shrine's flagstone plaza, and landmarks placed on exact grid cells with depth
  that follows how far south they stand. Gather's two floor tones used to differ
  by six values out of 255, so its authored regions were invisible; there are
  now four baked tiles and a floor plan drawn in them. The Wisp halo and shrine
  heart stay `sindri.shape`, because a script moves them every frame and no
  baked frame can do that. See `docs/isometric-baker.md`.

- **`docs/parity.md` gains a "beyond parity" section.** Everything above it
  answers "what is an engine expected to do" and is checkable against Unity or
  Godot; the new section answers "what could an engine provide that none of them
  do", and is kept separate so a reader can still tell a gap from an ambition.
  Candidates earn a row by naming what a game in this repository does by hand —
  `player.decay` has a literal `fn nearest()` looping tagged entities with a
  `best_distance`, and hand-decremented `cooldown`, `mine_cooldown` and
  `spawn_timer`; seven scripts despawn themselves on a countdown. Where no game
  wants a candidate the row says so, which is why named time domains are
  recorded and not queued. The section also corrects a framing error above it:
  raycast and overlap are listed as physics queries, but the query the games
  actually hand-roll is a gameplay one over tagged entities, of which physics
  casts are a subset.

- **The baker bakes three views, not one.** Its camera derived everything from a
  floor diamond: pitch was the tile's own ratio, and a square tile was refused
  because it has no pitch to derive — so a straight-down view was unreachable by
  construction, and the yaw was fixed at the 45° diagonal besides, so an
  axis-aligned view was unreachable at all. `view` now picks `isometric` (the
  default, unchanged), `top-down` or `side`. The flat views state
  `pixels_per_unit` rather than deriving a scale from a diamond they do not
  draw, each view refuses the other's fields, and a flat bake skips the
  footprint check — that check keeps a picture inside ground a tilemap handed
  out, and no tilemap is handing out any. There is no separate animation mode
  and none was needed: `variants` already packs several models onto one sheet as
  named frames, which is exactly what an animation clip reads. Orbital Last
  Stand's mine blast is the first thing baked this way — five top-down frames of
  one sheet, played once by a script that despawns it when the clip ends.
- **A script can drive an animation.** Sprite clips have existed, played and been
  authorable in the editor for some time, and no script could select one — so an
  animation could be cut, previewed and shipped without ever responding to
  gameplay. `Animation.play`, `stop` and `restart` name a clip the scene already
  holds; `is_finished`, `frame` and `clip` say where playback has got to;
  `set_speed` scales it. The surface is shaped around where the two halves of an
  animation live: which clip plays is authored state and is written to the
  world, and the cursor is derived and only read, so a script driving an
  animation still does not rewrite the scene it came from. `play` is idempotent
  — naming the clip already playing does nothing — because a script says what
  state it is in on every frame, and a `play` that restarted would hold a walk
  cycle on its first frame for ever; `restart` is the separate way back to the
  start of a one-shot. Gather's player is the proof: its walk cycle had run
  whenever the scene did, including while standing still, and now runs only
  while it walks.
- **A field that decides what else its object holds is now editable, anywhere.**
  A collider piece's `shape` was a readout: choosing `circle` has to replace a
  box's half extents with a radius, and writing the word alone leaves a payload
  the schema refuses. A camera's `projection` was the single exception, switched
  by a hand-written rule in the editor that knew the camera's two field names —
  which is exactly why nothing else could have one. A component now says what
  each spelling makes it hold, the registry checks the claim by building the
  component that spelling produces and decoding it, and one path performs the
  switch for every tagged field at any depth: the fields of the variant being
  left are dropped, the arriving variant's are filled in, and everything else is
  kept, because a field both variants have is a field the author set. A variant
  that could not be chosen safely now fails the build instead of the scene, and
  the pair of lists cannot drift — a spelling nothing describes and a variant no
  spelling names are both registration errors. `docs/generated/` carries the
  variants, so a tool that is not the editor knows them too.
- **A list of objects is something you can edit.** A compound collider was
  authorable in a scene file and not in the inspector: an array of objects fell
  to `ValueKind::Opaque` and was shown as stored, which was the honest answer
  while nothing described what a list held. The field template's exemplar item
  is what changed that — it says what a piece consists of, what each of its
  fields means, and what a fresh one is, so a new piece is the piece the
  registration already describes rather than one the editor invents. Pieces can
  be added, removed and reordered, and each piece's fields are drawn through
  their own meanings. Two refusals are deliberate: the last item cannot be
  removed, because the engine refuses a collider with no pieces, and a variant
  tag below the top level — a piece's `shape`, where `circle` must replace a
  box's half extents with a radius — is a readout that says why rather than a
  control that would write a payload the schema rejects. Whether something is a
  list is decided by the template, not the stored value, so an empty list can
  still be added to while a tilemap's thousand tiles and a footprint's pairs of
  numbers stay readouts.
- **Meanings are read at every depth.** They were consulted only for a
  component's top-level keys, so a meaning recorded for a nested field was never
  used: `sindri.ui.text`'s `outline.color` and `shadow.color` had swatches
  declared and drew four number boxes. Rows now carry the dotted path they sit
  at — `outline.color`, `pieces.2.friction` — which is what a meaning is keyed
  by, and what made list items describable at all.

- **A component says what its fields are for.** A field template said a sprite
  had a `texture` and that it held a string; it could not say the string named a
  texture in the project. So the editor guessed from the field's name, through
  three lookup tables that were wrong in both directions: `(_, "clip")` offered
  the project's audio to any component with a `clip` field, including a game's
  own, while a field the tables had never heard of was a text box in silence —
  which is what happened to every component added after they were written. A
  registration now declares meaning — which asset kind a field names, which
  spellings it accepts, that it is a colour, an angle, a bounded number, a
  collision mask, or another entity — and the registry checks every declared
  path against that component's field template, so a renamed field is a startup
  error rather than a picker that quietly stopped appearing. Paths are dotted
  and `[]` descends into a list, so one `pieces[].rotation` describes every
  piece of a compound collider. The editor's tables are deleted and it asks the
  registry instead; `docs/generated/sindri-capabilities.json` now carries the
  meanings too, so the knowledge reaches every tool rather than living where
  only the inspector could see it. Nothing the old tables got right was lost —
  a test pins each of the eight pickers and eight dropdowns they drew — and
  shapes, layouts, and collider pieces gained dropdowns they never had.

- **One parity document replaces the two status matrices.** `docs/parity.md`
  is written from the outside in: it records what a game engine is expected to
  do, what Sindri actually does across engine, editor, Decay and game proof, and
  the distance between them. `docs/function-matrix.md` and
  `docs/feature-integration-matrix.md` are deleted, because both graded Sindri
  against Sindri — a feature nobody had thought of had no row and so could not
  show as missing. The integration matrix called 2D physics **Ready** while a
  character could not be given a capsule with a circle at each side. The new file
  carries rows for capabilities we have never built, a protected list of what
  Sindri already does better, anti-goals taken from mistakes the baseline is
  still paying for, the Unity Asset Store read as a market-validated gap list,
  and a ranked queue of what blocks somebody shipping a game. The audit turned up
  two capabilities that were built and unreachable — the bloom chain and the
  input action layer — and a root cause for hand-typed asset fields: the
  editor guesses a field's meaning from its name through three lookup tables,
  which are global where they should be scoped — `(_, "clip")` offers the audio
  list to any component with a `clip` field — and silent for anything unlisted,
  which is why every component added since lands as raw fields.

- **A 2D collider may be authored in several pieces.** One shape is often a poor
  description of a thing: a character is a capsule with a circle at each side, a
  ship a box and two pods. `sindri.physics2d.collider` now takes a `pieces` list,
  and the pieces belong to the one entity and move as one object — a compound is
  one collider made of parts, not several colliders, and it needs no child
  entities, because a piece already carries its own offset and rotation. The
  single form still means what it always did and is kept rather than migrated,
  since a scene is a file someone wrote. Mass properties come from every piece
  and sum, which is the change most likely to move behaviour quietly, so
  `PhysicsWorld2d::mass` exposes the total and a test pins it. Validation is per
  piece and names the failing index, and nothing is inserted unless every piece
  passes.

- **A sheet says where its sprites meet the ground.** A world sprite is drawn on
  a quad centred on its entity, so the middle of the picture was what landed on
  the tile. Right for something that floats, wrong for anything that stands:
  Gather's player was drawn a third of a ball low, standing in the tile in front
  of its own, and nothing failed. `SpriteAnchor` — `"center"`, `"bottom"`, or an
  exact `[x, y]` in fractions of the frame — is declared on the sheet, because
  it is a fact about the picture rather than about whoever draws it. `"center"`
  is the default and is what a quad already did, so every sheet written before
  the field keeps its picture exactly; baked sheets now declare it anyway, since
  a sheet that says nothing cannot be told apart from one whose author never
  considered the question. Gather requires every texture it draws in the world
  to answer, which is the part that stops the next hand-drawn sprite repeating
  the mistake.

- **The baker refuses art that overhangs its own footprint.** A footprint is what
  the game reserves — collision, placement and draw order all read it and all
  assume the picture stays inside it. Gather's shrine stood 1.12 tiles across on
  a footprint of one, so it covered ground the game still handed out, and a
  player standing legally on the next tile was drawn sliced by a plinth it was
  not touching. Height stays free; only the ground is measured. Refused rather
  than widened automatically, because whether the fix is a smaller model or a
  bigger footprint depends on what the thing is.

- **A tilemap's tiles may hang below their cell.** `sindri.tilemap` gained
  `tile_overhang`: how far below its cell a tile's art reaches, in world units,
  zero unless a map says otherwise. A flat floor needs none; a floor of slabs
  does, because the top face still covers exactly one cell while the sides that
  make it read as a slab hang into the cells in front. The cell is unchanged and
  is still what the grid, picking, occupancy and gameplay measure in — only the
  drawn quad grows, and only downward. Gather's floor is now baked slabs, so its
  island has thickness at the rim instead of being a paper-thin diamond, and the
  baker gained a `plate` primitive for the flat case.

- **Gather draws in the order things stand in, and its scenery is solid.** Every
  world entity's sprite layer now comes from the isometric row it occupies, and
  the player and Wisp keep theirs current as they move — so the player walks
  behind what is north of it and in front of what is south. Hand-picked layers
  could not do this: the orbs sat on layer 10 and the player on 20, so both drew
  over the shrine from anywhere on the island. Solid scenery also claims its
  grid cell, so the Wisp's pathfinding routes around a tree and the player is
  stopped by one instead of walking through it.

- Expand Weave from an isolated presentation proof into composed responsive
  project UI: `@use` stylesheet graphs, percentages, constraints, padding,
  gaps, wrapping, alignment, viewport units, and text wrapping resolve into
  ordinary Sindri UI state. The focused browser showcase documents the surface,
  and Orbital Last Stand now uses four composed stylesheets across its HUD,
  title, pause, result, upgrade, route, event, and campaign-overlay screens.

- Give Orbital Last Stand an authored combat-spectacle pass: pooled neon
  projectile and companion trails, spinning/pulsing weapon geometry, reusable
  additive impact flares, expanded nova and death bursts, synergy celebrations,
  and fast-decaying camera trauma. A deterministic showcase capture assembles
  every weapon and four companions for desktop and phone visual inspection.

- Move Orbital Last Stand's 19 reference synergy recipes into a reusable
  profile asset, evaluate them generically in Decay, and connect their core
  projectile and companion interactions.

- Fix Orbital Last Stand portrait movement bounds, preserve the ship's last
  heading, restore its animated shield/core/engine presentation, and keep
  asteroid orientation stable through gameplay overlays.

- Add editor wall/footprint authoring, typed Decay pathfinding, and a Gather Wisp that proves authored A* navigation end to end.

### Fixed

- **Enemies no longer appear in the middle of the arena.** Regular enemies,
  elite chargers and bosses were placed on a fixed circle of radius 7.5, which
  the camera is required to frame whole on every supported screen — so every
  spawn was visible as it happened. Placement now follows the viewport: a
  random direction walked out past the edge of what the camera shows, with the
  authored arena radius as a floor, and a boss entering from just above the
  top. Drops still appear where the thing that dropped them died.

- **Shoving an asteroid no longer kills its script.** `HazardAsteroid` wrote
  its authored drift back when a rock was knocked off the player, but declared
  it `let`, so the shove failed with `Immutable("this.vx")` and the asteroid
  stopped running. The two fields are `@export var`, which is what a value the
  host authors and the script then changes has to be.

### Added

- **Reusable profile assets across runtime, editor, Decay, and export.** A
  versioned `.profile.json` is Sindri's ScriptableObject-like home for
  game-owned data outside the scene. The editor creates and structurally edits
  nested profiles and offers them in typed `Profile` field pickers; Decay reads
  them through typed `Profiles.*` calls; native, browser, and static export hosts
  load the same assets. Orbital Last Stand proves the path by moving all 160
  module definitions, weighted pools, requirements, labels, and generic effects
  into one reusable profile, so adding ordinary modules no longer grows either
  gameplay script.

- **The engine now writes down what it can do, for tools and agents.**
  `cargo run -p sindri-capabilities -- --write` generates
  `docs/generated/decay-api.json`, its Markdown rendering, and
  `docs/generated/sindri-capabilities.json` from the two descriptions the
  repository already treats as authoritative: the Decay host surface the
  analyzer and runtime share, and the built-in component registry. A workspace
  test fails when the files disagree with the code, so widening the scripting
  surface or registering a component cannot leave the reference behind.
  `docs/cli-conventions.md` records the grammar the future `sindri` CLI follows
  and why its operation names are a contract of their own.

- **Orbital Last Stand now has its complete eleven-boss roster.**
  Harrower, Prism and Singularity join Warden, followed by Crown, Brood, Mirror,
  Architect and Spine at 4:00 and Leviathan and Last Light at 8:00. The director
  avoids its two most recent selections. Each encounter restores its reference
  attack identity and obeys the viewport combat rule, while clear phase
  transitions and safe-gap volleys keep the denser patterns readable.

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
  its own.

  None of them go through the undo history, and that is not an oversight: the
  history describes a world and these describe a directory, so undoing a delete
  would mean holding the bytes of every removed file for as long as the session
  lasts. What stands in for it is that each operation is checked before it runs
  and refuses rather than overwrites; that nothing can name a path outside the
  project, because a row hands over whatever was typed into it and `../secrets`
  is a perfectly good string but not a file name; and that deleting — the one
  with nothing behind it, and which takes a whole folder — asks first.

  Renaming the open scene follows it, because the editor holds the path it saves
  to and a rename it was not told about would write the scene back under its old
  name and leave two of them on disk. A copy keeps the whole suffix that says
  what kind of asset it is, since `file_stem` stops at the last dot and a
  duplicated scene would otherwise become `level.scene copy.json`.
- **An entity's stable ID is visible and editable**, and the scene has a panel
  of its own. `source_id` is what the file keys an entity by, what a parent link
  names, what sibling order is derived from and what `sindri.grid.occupant`
  points at — and nothing showed it, so the editor could produce
  `game-object-1` and nothing else while Gather's entities are `player`,
  `floor`, `orb-1`. It is a field under the name now, and renaming one takes
  every occupant that points at it along in the same transaction: a stable ID is
  a reference, not a label, and renaming a grid without rewriting its occupants
  leaves a scene that still opens with nothing on the board. An ID that is blank
  or already taken is refused at the field rather than written and rejected.
  With nothing selected, the inspector now shows the scene rather than a shrug:
  its name — a real field that round-trips through a save and was shown nowhere
  — its file, and how many entities it holds. Both are written once the edit is
  finished rather than on every keystroke, and both go through the command
  history, so the editor still knows when the document is unsaved.
- **New Scene and Save As.** A scene file had to exist before the editor could
  do anything with it, so the tool could only continue a project someone else
  had started — and started somewhere the default scene is not, it opened
  detached with Save disabled and no way to make a file to save into. New scene
  (Ctrl+N) asks where the scene goes, writes it, and opens it through the
  ordinary path, so a new scene proves it loads before anyone works in it. It
  holds one world camera: a scene with none is legal and renders a black Game
  view, and "why is the game view empty" is not the first question a new project
  should raise. Save scene as… (Ctrl+Shift+S) is offered whether or not the
  scene has a file, adopts the path only after the write succeeds, and takes the
  project beside the scene, the remembered scene, the textures and the scripts
  with it. A save box takes a name rather than an extension, so the suffix is
  the editor's business: `level` is written as `level.scene.json`, which is what
  the project browser lists as a scene and what reopening it finds.

### Fixed

- **A script failure named a handle nobody could look up.** It printed
  `entity EntityId { index: 4, generation: 0 }`, which is what the runtime has
  and not something anyone can find in a hierarchy. `ScriptFailure` says which
  entity and what went wrong separately now, so the editor — which holds the
  world — writes "Wisp: names script 'NoSuchContainer', which
  'scripts/wisp.decay' does not declare", and the console row ends in that
  entity's name as the way to it. The entity is carried on the entry rather than
  read back out of the message: searching the text for something that looks like
  a name would select the wrong entity the first time a message mentioned a word
  that happened to be one.
- **Add Component could break the scene in one click.** It offered Camera
  whether or not the scene already had one, and a second authored world camera
  is a hard extract error — both viewports go dark with "the scene contains more
  than one authored world camera", and nothing says which two entities are now
  the cameras. Camera is listed and disabled on a scene that already has one.
- **A `.txt` file drew with the glyph the editor uses for an entity**, claiming
  it is an object in the scene, and a font shared the image glyph with a
  texture. Both have their own now.
- **Nothing in the Scene view could be selected by clicking it.** The viewport
  allocated its region with `Sense::drag()`, and egui sets a response's clicked
  flag only for a widget whose sense includes clicks — so the panel's
  `clicked_by` was always false, whatever the picking underneath decided. The
  tile brush was half-dead the same way: it painted on a drag and ignored a
  single click. The coupling between what the response senses and what the panel
  asks it is stated in a test now, because it is invisible from both ends.
- **A fully transparent element swallowed every click over it.** Found by
  clicking Gather's player and selecting its win banner instead: the banner is
  `tint` alpha zero, a third of the viewport wide, sitting in the middle of the
  scene until the game says otherwise. A thing drawn as nothing is not a thing
  to click, in the overlay or in the world.
- **A UI element's gizmo appeared where it is not.** A UI element's position is
  an offset from its anchor in overlay space, and the handle was drawn at the
  entity's transform in world space — so selecting Gather's title and choosing
  Move put one red arm in the bottom-left corner of the Scene view, mostly off
  screen, while the text was at the top. The gizmo is told where its handles
  belong now, separately from the transform it edits, so the pointer maths
  happens against the drawn origin and the answer lands on the authored value. A
  UI element is offered two arms rather than three: its Z orders it within the
  overlay rather than placing it.

### Changed

- **The snap increments can be set, and are remembered.** The snap button's
  tooltip named 0.5 units, 15° and 0.1 scale, and all three were constants
  nothing could change — so a board laid out on quarter units was a board laid
  out by hand. Right-clicking the button sets them. They live there rather than
  in a preferences dialog because that is the control they belong to, and a
  toolbar has no room for three number fields nobody usually touches. Zero is
  allowed and means that one does not round, which is what the gizmo already did
  with a zero step.
- **The console filters by level.** All, Problems, or Errors, remembered across
  launches because it is a reading preference rather than a state: someone
  watching for a failure wants it filtered to failures for as long as they are
  watching. A filter that hides everything says so, since an empty panel
  otherwise reads as a console that stopped working.
- **Delete and F2 act on whichever selection was made last.** The editor holds
  two — an entity and an asset — and these keys always meant the entity, so the
  project browser's menu could not honestly print them beside its own entries.
  Choosing something is what says which the keys mean, which is what a selection
  already communicates.
- **Add Component says what it is not offering, and why.** Sprite Animation
  needs a sliced sheet, Grid Occupant needs a grid, UI Text needs a font in the
  project — and failing any of those, the entry was simply absent, leaving the
  menu quietly shorter than the documentation. Every type the entity's space
  accepts is listed now, disabled with the reason. The rule it replaces stands
  where it belongs: a button that adds a component the engine then rejects is
  worse than no button, and an entry that says why is neither of those.
- **UI elements can be picked in the view that draws them.** Twelve of Gather's
  twenty-two entities are UI, and none of them could be clicked: an anchor picks
  a point on the viewport and the transform is an offset from it, so a world ray
  through a world camera passes nowhere near them. UI images are picked in a
  pass of their own, against the same overlay matrix the frame draws them
  through, and — being drawn over the world — take precedence over it. UI text is
  deliberately left to the hierarchy: what a string covers is decided by glyph
  layout inside the text renderer.
- **Duplicate, rename in place, and Delete, from the row they act on.** The
  three verbs whose absence is felt on every entity after the first. Gather has
  five Orbs, five Pips, and five Pip Sockets, each of which had to be built from
  scratch; renaming meant selecting a row and finding the name field in another
  panel; and Delete was one icon in a header. All three are on the hierarchy
  row's own right-click menu and on a key: Ctrl+D, F2, and Delete or Backspace.
  A rename happens in the row, focused as it appears, Enter to commit and Escape
  to abandon. A duplicate takes the whole subtree, lands beside the original as
  a sibling, earns a stable ID nothing else is using, and undoes in one step —
  which needs the copy rehearsed against a clone of the world first, because
  `WorldCommand::Spawn` names the handle it spawns at and `World::next_handle`
  answers the same thing however many times it is asked.
- **The editor answers a right-click.** There was not one `context_menu` call in
  it, which is half of why the verbs above did not exist: an action that belongs
  to *a specific thing* had nowhere to live. A hierarchy row and a project row
  have menus now, both drawn through one primitive so a menu does not change
  width with the name of whatever is selected and a destructive entry reads as
  destructive. The project row's menu offers what the browser can already do —
  open a scene, look inside a folder, slice an image — and the asset path a
  component field wants, which until now had to be read off the row and typed
  back in by hand.
- **The unmodified shortcut keys belong to whatever is being typed into.** With
  a text field now in the hierarchy, F while renaming would have framed the
  camera and Backspace would have deleted the entity being named rather than a
  letter of its name.
- **The Project dock is a browser rather than a listing.** Its folders fold, so
  a project with four asset directories is no longer a wall of every file in all
  of them. Its folder pane navigates: choosing one lists that folder and nothing
  else, and it used to be labels with no sense that selected nothing and
  filtered nothing. And it has a selection of its own, marked with the band a
  selected row wears — it used to mark only the open scene, so the scene file
  was permanently lit and clicking anything else changed nothing visible. The
  open scene keeps a quieter rule in the margin, because which scene is open and
  which asset is selected are different facts. Every row answers a click now,
  including the ones the editor can do nothing else with: what a row can do is
  said on hover instead of by refusing to respond.


- **A component added in the editor is the component the game uses.** The
  registry recorded one payload per type and asked it two questions — what does
  this component have, and what is a fresh one — so a type with no honest blank
  had no answer to the first either. `sindri.ui.text` inspected as two rows for
  a seven-field component, and a tilemap made in the editor had no `projection`
  and could never be isometric. Those are now separate registrations: a field
  template says what a component consists of, a default payload says what a
  fresh one is, and a type may have the first without the second. Both are
  checked against the field list serde will ask the type for, so a template that
  drifts from its struct is a startup error rather than a missing row noticed a
  release later.
- **A script can be put on an entity.** `sindri.script` and
  `sindri.audio.source` had no default payload and so were never offered by Add
  Component — in an engine whose headline capability is scripting, and a
  companion game that is thirteen script components. Both are offered now,
  completed from the project beside the scene: the first `.decay` source that
  declares a container, the first audio clip. A script arrives with its source,
  its container, its typed `@export` fields, and `enabled`, which had never been
  visible. Behind it, a component that says what an entity *does* no longer
  decides where it *is*: a script on a fresh entity used to mark it a world
  object and stop the menu offering UI Text, which made Gather's script-driven
  HUD unbuildable.
- **Play mode is read-only.** Saving while a scene was playing wrote the running
  world to the file — the authored scene replaced by wherever the scripts had
  pushed everything, marked as saved, with Stop then restoring a world the file
  no longer held. Every other edit made while playing was discarded by Stop
  without being mentioned, leaving undo describing changes the world no longer
  contained. A running scene is not the document: the inspector, the hierarchy's
  create and delete, the gizmo, the tile brush, undo, redo and the File menu all
  stand down until it stops, each saying why. The viewport still orbits and
  selects, and the inspector still shows the values changing.


- **The editor draws itself from one design system instead of eleven opinions.**
  Every panel used to pick its own greys, gaps, and font sizes by copying the
  panel beside it, so the same idea was spelled differently in each one and
  property values were right-aligned — no two rows in a component started at the
  same place. `editor/src/ui/` now holds the tokens, the icon vocabulary, and
  the controls built from both, and the panels are rewired onto it: a scene row
  is a banded row with a selection rule and a guide per level of nesting, a
  component is a heading that folds and carries its own actions, a transform
  component is a well with its axis letter on a spine tinted the colour that
  axis has in the viewport, and a panel with nothing in it says what it is for
  rather than being blank. Scene and Game read as workspaces rather than as two
  more tabs, the scene tools are grouped by what they do and scroll rather than
  clip when the viewport is narrow, and destructive actions — Discard changes,
  Remove component, Delete entity — are drawn as destructive. Every interaction
  is the one it was: selection, drag-and-drop reparenting, the fold
  preferences, the slicer, the tile brush, and the checked-command path behind
  every inspector edit.
- **A procedural texture stops being reported as a texture the project lacks.**
  The inspector's asset picker was built from the scene's directory alone, and
  `procedural:checkerboard` is deliberately not a file, so the fixture's own
  cube was marked as naming something that does not exist. The picker offers
  what the engine can draw, which is the project's images and the ones it
  generates.


- **A scene now has two kinds of entity, and they are spelled apart.** A sprite
  is a thing in the world; a thing on the viewport is `sindri.ui.image`, and
  `sindri.text` becomes `sindri.ui.text`. `sindri.sprite` loses the `space`
  field that used to mean one component was really two — an anchor mattered on a
  screen sprite and decided nothing on a world one — and `sindri.tilemap` loses
  it too, because a map is in the world. Scene format 8 migrates every scene:
  a screen sprite becomes a UI image with everything it drew with, a world
  sprite keeps its name and loses the two fields that decided nothing, and a
  screen-space tilemap stops the migration with a message rather than being
  quietly relocated. Scripts follow: a HUD element writes
  `this.ui_image.tint.a` where it used to write `this.sprite.tint.a`.
- **The Scene view's authored-camera gizmo stops lying about the camera.** Its
  frustum is drawn at the aspect the camera actually renders at — the Game
  viewport's — so resizing the Scene view no longer reshapes it, and every line
  is clipped against the near plane in clip space, so orbiting past a camera no
  longer smears its frustum across the viewport. The frustum is drawn for the
  selected camera only; an unselected one keeps its marker and a short forward
  stub, which is also all a click selects.
- **Play, pause, and stop are two controls and a word instead of four
  controls.** There were a stop icon, a pause icon, a play icon, and an accent
  button — and the accent button said "Stop" while running but paused when
  pressed, as did the play icon beside it. Now one button enters and leaves play
  mode and is labelled with what pressing it does, one icon holds and releases a
  running scene, and the editor says whether it is Editing, Playing, or Paused.
  Ctrl+P plays and stops; Ctrl+Shift+P pauses and resumes.
- **The inspector shows what a component has, and edits it with controls that
  know what it means.** Every field of a component is drawn whether or not this
  instance wrote it down, so two of one component no longer show two different
  sets of rows; a field left alone is still not written to the file. A field
  whose value is one of a few names — a camera's projection, a UI anchor, a
  tilemap's projection, a body's kind — is a menu rather than a text box, and
  switching a camera's projection writes the fields that projection has instead
  of producing a payload the schema refuses. A field naming a project file gets
  a picker beside it, a tint gets a colour swatch, fields are ordered by what
  they say about the component rather than alphabetically, and a row that is
  only a readout says on hover why.
- Rust source files are now capped at 600 lines, with 400 as the target, and
  `scripts/check-file-size.py` enforces it in CI. Twenty-one files were over the
  cap — the largest was the 4,714-line native editor — and each is now a
  directory module named for what its parts do. No public API changed; every
  `use sindri_core::…`, `sindri_scene::…`, and `decay_*::…` path is what it was.
  `docs/module-layout.md` states the rule and how to satisfy it.
- Decay's `&&` and `||` now short-circuit: the right operand is evaluated only
  when the left does not already decide the answer. A guard such as
  `held != null && World.exists(held)` protects the call to its right, which it
  did not before. Both operators still require and produce `bool`.

### Added

- Browser startup failures now reach the player: the engine announces them on
  `window` and the page shows them, instead of a blank canvas and a console line
  nobody opens.
- The browser page asks for a real WebGPU adapter before starting, rather than
  trusting that `navigator.gpu` existing means WebGPU works.

- End-to-end audio across assets, scene authoring, native/browser platform backends, typed Decay calls, editor discovery/component authoring, and Gather background/pickup/victory playback, with a silent backend for device-free tests.
- Command-backed Scene-view translate, rotate, and scale gizmos with local/world
  orientation, optional movement/angle/scale snapping, Z-lock-safe movement,
  Q/W/E/R tool shortcuts, and one undo step per drag; transform rotation is now
  editable as Euler degrees in the inspector while remaining quaternion-backed.
- Authored `sindri.grid_navigation` walls and `sindri.grid_occupant`
  footprints, with a world adapter that resolves stable grid references,
  derives occupancy from transforms, validates complete placement, and runs
  wall-aware whole-footprint paths.
- Normalized renderer-independent wall edges in `sindri-grid`, with bounded
  symmetric block/unblock queries and A* integration for cardinal, diagonal,
  occupancy, and multi-cell footprint paths.
- Deterministic renderer-independent A* pathfinding in `sindri-grid`, with
  cardinal/eight-way movement, explicit corner-cutting policy, integer costs,
  memoized passability, and whole-footprint occupancy paths.
- Renderer-independent multi-cell footprints and bounded occupancy in
  `sindri-grid`, with deterministic cells, atomic moves, and explicit placement
  errors for conflicts, bounds, and coordinate overflow.
- Typed Decay grid positioning through `Grid.position_x`, `position_y`, and
  `place`, with Gather gameplay migrated from top-down world coordinates to the
  exact logical space of its transformed isometric tilemap.
- Shared tilemap/grid coordinates: `sindri.tilemap` now adapts `sindri-grid`
  into upward world Y for both rendering and editor picking, while full map
  transforms rotate and scale the grid itself instead of only its tile quads.
- A dependency-free `sindri-grid` foundation with typed logical coordinates,
  finite bounds, stable neighbour queries, validated orthogonal/isometric
  projection, and exhaustive round-trip coverage across negative and positive
  space.
- A feature integration matrix tracking runtime, editor, Decay, and Gather
  counterparts together so a feature added on one surface leaves its remaining
  integrations visible.
- Drag-and-drop hierarchy reparenting onto another GameObject or the World root, with legal-target feedback, cycle prevention through the world's existing checks, and one-step undo.
- Unity-style editor hierarchy authoring: every GameObject can contain children, child-bearing rows collapse with restart-persistent state, filtered results retain their ancestor paths, and the create menu can add an empty root or child with a stable scene ID.
- Scene-viewport entity selection for world sprites, filled tilemap cells, and meshes, using the rendered camera and geometry with layer-, depth-, and occlusion-aware overlap resolution.
- Sprite-animation clip authoring in the native editor: add a valid animation from a sprite sheet, create, rename, and remove clips, arrange named frames, edit timing and looping, choose the runtime clip, and preview playback against the project texture without changing scene state.
- Text authoring in the native editor: add `sindri.text` when a project font exists, edit multiline content, and choose among project-owned font assets without hand-editing scene JSON.
- Screen-space `sindri.text` rendering through Glyphon, with anchored/layered frame extraction, validated project font assets shared by native and browser hosts, editor loading and hot reload, and a real Inter-rendered title in Gather.
- GitHub Pages delivery for the WebAssembly/WebGPU Gather build from `main`.
- World-space tilemap authoring in the native editor: a visual palette from the texture's sprite-sheet sidecar, overlap-preserving grid resizing, Scene-view paint and erase with projected cell feedback, editable render layers, and undoable drag strokes.
- Initial Rust workspace with `sindri-core` and the public `sindri` facade.
- Engine lifecycle and renderer-independent runtime host.
- Capped fixed-step clock with pause and spiral-of-death protection.
- Generation-checked entities, safe hierarchy operations, and recursive destruction.
- Versioned scene documents with stable logical IDs and hierarchy validation.
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
