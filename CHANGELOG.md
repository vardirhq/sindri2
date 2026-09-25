# Changelog

- Gamepads, on desktop and in the browser. Players join by pressing a face button or Start on their own pad and leave when it is unplugged; scripts read each player's pad through `Gamepad` by player slot (`Gamepad.joined()`, `Gamepad.just_pressed(1.0, "south")`, `Gamepad.axis(2.0, "left_x")`), with slot 0 meaning any pad. Action bindings accept `gamepad.<button>` and `gamepad.axis.<axis>`. The editor's Play reads pads too, and starts the players over each time. Building on Linux now needs `libudev-dev`, as audio needs `libasound2-dev`; turn off `sindri-platform`'s `gamepad` feature to do without.
- Inspector edits can wait. A component says, per field or as a whole, when an edit applies: as it is made (the default), once you stop typing or dragging, or only when you press Apply. A voxel world now waits for Apply, with Revert beside it in the component's header, so typing a number no longer regenerates the terrain at every keystroke; the editor shows "Regenerating…" while it rebuilds. `ComponentSchemaRegistry::apply_when` declares the mode, and the generated component list reports it.
- Weave devtools can edit. Clicking a value in the inspector's Styles section edits it in place, and the new value is written into the stylesheet on the rule's line (only the value changes; comments and spacing stay), then the styles reload from the file. The Game view has a Pick toggle: while the game runs, the next click selects the frontmost element under it instead of reaching the game, so its styles show in the inspector. `weave::set_declaration` makes the same edit for other tools.
- Weave devtools in the editor. Selecting a UI element shows a Styles section in the inspector: its box model as drawn, every rule that matched, strongest first, with the file and line it was written on and the declarations a stronger rule overrode struck through, and the computed values and variables, at the Game view's size. Rules now remember where they were written, through `@use` imports and `@media` blocks, and `weave::matched` and `sindri_weave::inspect` answer the same questions for other tools.
- Weave selectors and lengths come closer to CSS. Structural pseudo-classes (`:first-child`, `:last-child`, `:only-child`, `:nth-child()` with `an+b`, `odd` and `even`, `:nth-last-child()` and `:root`), `:not()`, `:is()` and `:where()` with CSS's specificity, and the `+` and `~` sibling combinators, all in scene order. `calc()` mixes units wherever a length goes, `var()` included, and shorthands keep a `calc()` whole. The Weave showcase styles its feature cards by position instead of a class each.
- UI grid layout. A UI Grid component (`sindri.ui.grid`), or Weave's `display: grid`, places children in columns and rows: tracks are fixed lengths, `auto` or `fr` shares (with `repeat()`), items can start where they say and span tracks while the rest flow into free cells, extra rows or columns are added as needed, and items fill or align in their cells. In Weave: `grid-template-columns`, `grid-template-rows`, `gap`, `row-gap`, `column-gap`, `justify-items`, `align-items`, `grid-column` and `grid-row`. The Weave showcase's row of feature cards is now a grid, drawn identically on desktop and phone.
- Text sizes what holds it. A text element can size itself to its words, measured by its font (Weave's `width: auto` and `height: auto`, or `fit_content` on the UI Box component), plus its padding; a layout that fits its content then grows with its label, and a label is the least a shrinking flex item keeps. The game, the browser export, the editor's views and the Weave showcase's capture measure each draw, and clicks land at the measured size. The showcase's "Live layout" badge now fits its label instead of a hand-set width.
- UI layouts are CSS flexbox. Children can grow into spare room, shrink when there is too little (weighted by size, as in CSS), start from a basis, change order, wrap onto new lines and align themselves, within minimum and maximum sizes, and a layout can size itself to its children. The layout now decides sizes as well as positions, and what is drawn and what is clicked follow it. In Weave: `flex`, `flex-grow`, `flex-shrink`, `flex-basis`, `flex-wrap`, `flex-direction`, `order`, `align-self`, `align-items: stretch`, `justify-content: space-around` and `space-evenly`, and `width`/`height: auto`. As in CSS, a row whose children are wider than it now shrinks them to fit instead of letting them overflow; `flex-shrink: 0` keeps a child's size.
- UI elements have a box model. A new UI Box component (`sindri.ui.box`) gives any element padding and margin per side, and layouts honour them: children flow inside their parent's padding and keep their own margins clear, as in CSS flexbox. Weave's `padding` and `margin` take one to four values as in CSS, with `padding-top` and the other per-side longhands overriding one side, and percentages of the containing element's width. A layout that had Weave padding now starts its children inside that padding, so a phone layout whose content touched its card's border now sits inside it.
- Weave has `box-shadow`: an offset, blur, spread and colour, drawn as a soft copy of the shape behind it with matching corners. The Weave showcase's cards and buttons now cast them.
- Weave `:hover` and `:active` now work in the running game: the editor's Play, native games and browser exports lay the pointer's states over the styled UI each frame, and clicks land where things are drawn, so a button a media query moved is clicked where it is drawn. A game's styles are applied when it starts and when the screen changes shape, and a value a script writes afterwards is not styled back, as an inline style beats a stylesheet. Stylesheets can animate between states with CSS `transition` (durations, delays, `all`, and `ease`/`ease-in`/`ease-out`/`ease-in-out`/`linear`/`cubic-bezier` easings) on colours, lengths and numbers.
- Weave selectors now work as in CSS: element names (`text`, `button`), compound selectors (`button.primary:hover`), descendant and child combinators (`.menu text`, `.menu > button`), selector lists, and CSS specificity. Text properties inherit from their container, custom properties (`--accent`) and `var()` with fallbacks give themes one place to live, and media queries combine with `and` and commas. `:disabled` and `:checked` follow the entity's own data; `:hover` and `:active` match when a host passes pointer state in. Existing stylesheets keep working, `sindri.ui.text` included. `docs/ui-direction.md` sets out the plan to make Sindri's UI better than Unity's and Godot's.
- A platformer showcase: `games/platformer`, a small side-view game with a painted, solid level, a hero who runs and jumps, coins, a flag and a camera that follows. It is the first of a set of genre showcases, and is playable in the editor and on the site.
- Moving a physics body's transform from a script now moves the body, as it does in Unity, instead of the next physics step putting it back. A respawn or an edge clamp written as a position now works; a position-kinematic body treats the move as its next target, so a platform moved this way carries what stands on it.
- A frictionless collider is now frictionless against everything: friction combines as the smaller of the two rather than their average, so a platformer's hero pressed into a wall slides down it instead of clinging. Two equal frictions combine as before.
- Camera follow, confinement and shake now run in the editor's Play and in exported browser games, not only in the camera example.
- Tilemaps can be solid. Add a Tilemap Collider 2D beside a Tilemap and every painted tile collides, merged into as few boxes as cover them so characters don't catch on tile seams; list sprites as passable for decoration. A scene can set its own gravity with a Physics 2D World component, so a platformer falls in the editor's Play as it will in its build.
- The Scene view shows collision. Every collider is outlined in green, a tilemap's generated boxes included, and the selected collider has handles: drag a box's edge, a circle's radius or a capsule's height to size it, as one undoable step.
- The Scene view has a 2D mode beside Perspective and Ortho: straight onto the level, with no perspective, and dragging pans instead of orbiting. A scene whose camera is a flat 2D camera opens in it, framed on what that camera sees.
- The Scene view's camera, light and collider markers no longer draw on top of the floating panels.
- Mountains no longer have holes through them. Natural terrain's overhangs were carved into thin ridges from both sides at once, and tunnels broke through ridges too narrow to hold them, leaving windows to the sky with rock hanging above. Undercuts and cave mouths now only cut into walls thick enough to keep rock behind them, so ranges keep their ledges and overhangs but peaks and ridges are solid, and snow caps stay on the peaks. Worlds from the same seed change shape slightly in their ranges.
- The Scene view has a scene-lighting toggle, as Unity's does: the bulb in its toolbar. On (the default), the view is lit exactly as the game is, so a sun at zero and no ambient is black. Off, the Scene view is lit evenly by the editor, from over the camera's shoulder with no shadows, so a dark scene can still be worked on; the Game view always shows the scene's own lighting.
- Sunlight now lands on the side of the world its arrow points at. The textured shader worked out which way a surface faces with the sign backwards, so every face turned away from the sun was the one lit, and shadows (which were right) fell on the lit side. Scenes look lit from where their Sun is aimed, and slopes facing the sun are bright instead of nearly black.
- Blocks can move and glow. The built-in water now ripples and lava churns and glows, drawn by shifting where each face reads rather than by rebuilding the world, and a block set can give any face an animation (frames on one texture, and a speed) and any block a glow. Leaves and other blocks that don't hide their neighbours are now cut out, holes and all, and a lake no longer has walls inside it. The block editor has a Glow field.
- Blocks are premade and chosen by name. The engine ships a block set, `builtin:blocks` (grass, dirt, stone, sand, snow, ice, mud, moss, gravel, clay, planks, log, leaves, water, lava and more), and a voxel world can name a block set and have its generator say "grass" and "water" instead of material numbers. A new Voxel World is built from the built-in blocks, and Voxel Lab now is too. The inspector offers blocks as a menu of cubes.
- Block sets are edited in the inspector: select a `.tileset.json` to see its blocks as cubes and edit each block's faces (with pictures), whether it hides its neighbours, supports, is walkable, its height and its tags. "New block set here" starts one as a copy of the built-in set.
- Blocks can carry tags (`hot`, `liquid`, …) that scripts ask about with `Grid.tagged(volume, column, row, level, tag)`.
- Exports no longer look for engine-provided `procedural:` and `builtin:` assets on disk.
- Texture fields in the inspector show the picture they name beside the reference, and the reference picker shows each texture and sprite as a picture. A long reference is cut off before its sprite name, so fields naming different parts of one sheet no longer look identical.
- The inspector draws voxel materials as the blocks they are: every material picker (a biome's surface, the water, the trunk…) shows each option as a small cube with its own top and side textures, and each material in a Voxel World's list shows its cube beside its ID.
- The sun is now an object in the scene, like a camera: a Directional Light entity aimed by its rotation. The Scene view draws it as a sun with an arrow showing which way the light goes (and, when selected, a trail across the world), it can be clicked and rotated, and Create GameObject offers Directional Light. New scenes start with one. Scenes that stored a sun direction in the Environment are migrated to a Sun entity aimed the same way (scene format 10), so a light shining upwards, which put shadows on hilltops, is now visible as an arrow pointing up.
- Voxel Lab's Scene view now starts on open ground by the coast instead of inside a mountain.
- Voxel worlds can generate natural terrain: continents and sea, mountain ranges with overhangs and cave mouths, rivers, beaches, snow lines, frozen seas, tunnels and caverns, trees, and biomes you define by climate, surface materials, tree density, roughness and terraces. Biome edges fray into each other and their heights blend. The inspector edits every field with material pickers (with *None* for optional features) and switches between generators cleanly; Voxel Lab now opens on a natural world.
- Canvas panels can be moved anywhere over the scene: drag the empty part of a panel's tab strip to float it, drag its bottom-right corner to resize it, and double-click the strip or drop it back in its corner to anchor it again. Panels snap to the canvas edges and to each other, and cannot be dropped off the canvas.
- A voxel world is no longer rebuilt when an unrelated texture loads or hot-reloads; it compiles its sections again only when a texture its own faces draw with changes.
- Editor layout fixes: the search box no longer sits on top of the Play controls in the Docked and Wide arrangements; docks share the window so the scene always keeps room, and a dock squeezed by a small window grows back with it; Wide keeps the Assistant as a tab beside the Inspector; the Project browser in a bottom dock lists every file; the Scene view's status plate and axes no longer hide under the title bar and transport; the Canvas inspector uses the height of the window; and long read-only values end in an ellipsis.
- The console separates what is wrong now from what the log remembers: the status bar names the current problem and counts current problems rather than past errors, the count clears as soon as the cause is fixed, and clicking it opens the console's new Now view. The console tab carries the count on its icon instead of a chip that covered neighbouring tabs, console lines can be copied from a right-click, and the entity a line is about is linked beneath it rather than cut off beside it.
- The inspector now edits Environment and Voxel World with the right controls: colour pickers for their colours, values held inside the ranges the engine accepts, a menu for shadow map size and tone mapping, texture pickers for voxel material faces, and whole-number section coordinates.
- Texture pickers offer the sprites cut from the project's sprite sheets, so references like `blocks.png#stone-0` are no longer marked as missing.
- The editor no longer freezes the Scene view when a Voxel World or Environment value is invalid: the last valid terrain and lighting keep drawing, and the problem is reported against its entity, naming the exact field and the values it accepts.
- Adding a voxel material in the inspector now gives it an unused ID, a material the generator still uses cannot be removed, and the generator's surface, subsurface, and deep layers are chosen from the defined materials.
- The viewport's error banner no longer sits under the status bar or the bottom-left panel, and wraps long messages.
- Added authored world fog/atmosphere with distance, exponential-density, and height contributions, plus Voxel Lab horizon integration and consistent browser materials.
- Camera gameplay can now trigger engine-owned shake through Decay with `Camera.add_trauma`, and the camera acceptance demo moves its target and triggers impacts from Decay instead of bespoke Rust gameplay.

All notable user-facing changes to Sindri Engine are documented here.

This project has not made its first release yet. Until then, changes are collected
under `Unreleased` and kept intentionally concise. Detailed implementation
history, design rationale, debugging notes, and test archaeology belong in pull
requests, commit history, and subsystem documentation rather than this file.

## [Unreleased]

### Changed

- Voxel Lab's authored scene now uses the engine-owned `sindri.voxel_world`
  component in the editor, with deterministic layered terrain, bounded 3D
  section residency, neighbour-aware block meshing, and persistent cached GPU
  geometry. Its previous editor-only tile-volume island is no longer the scene
  being inspected.
- Voxel Lab now uses the editor Scene view's orbit, pan, and zoom interaction
  model. Touch screens use one finger to orbit and two fingers to pan or pinch,
  while an unmoved tap still digs at the camera focus.
- Causeway now materializes deterministic 16×16 terrain chunks around its
  camera instead of generating a complete 160×160 island up front. Its sparse
  navigation work follows loaded ground rather than the declared world bounds,
  so Build panning can reveal new biomed terrain inside a 65,536-cell envelope
  without scanning that envelope.
- A tile volume is resolved into the faces it draws once and kept, rather than
  rebuilt every frame. Extracting Gather's farm cost 6.7 ms a frame and now
  costs 1.2 ms; the volume's own share of that fell from 5.7 ms to about 0.2 ms.
  Only the depth each cell sorts at is measured again, because that is the only
  part a moving camera changes. An entity now carries a revision, and texture
  and tile-set bindings a generation, so an edit to a volume — its cells, its
  grid, its transform, the art it draws from, or whether it takes part in the
  scene at all — is picked up on the next frame.

### Added

- Added an authored world post-processing stack with exposure, tone mapping,
  contrast, saturation, bloom, and vignette. World effects resolve before
  overlay/UI rendering so interface content remains crisp; Voxel Lab now uses
  the complete stack as its presentation acceptance surface.
- Added mesh-time ambient occlusion for voxel corners and contacts, with authored environment strength and Voxel Lab proof.

- Added authored directional shadows for textured world and voxel geometry, with environment controls for coverage distance, map resolution, and bias.

- Added authored ambient and directional world lighting to `sindri.environment`; textured 3D and voxel geometry now share the same renderer lighting in editor and browser Voxel Lab, while scenes without it retain the previous unlit appearance.

- `sindri.environment` now authors ambient and directional world lighting. Textured 3D geometry and Voxel Lab terrain respond to the same sun direction, colour, and intensity in editor and browser rendering.

- Cached voxel sections outside the current camera frustum are no longer
  submitted for drawing or uploaded merely because they remain resident. Their
  compiled CPU geometry stays cached and becomes drawable when the camera can
  see it again.
- `sindri-scene` now bridges semantic block-mesh output into deterministic
  texture/atlas batches and revisioned persistent renderer commands. Stable GPU
  identities survive remeshing, superseded worker results are discarded before
  upload, and sections leaving residency explicitly release their buffers. The
  first bridge deliberately accepts opaque block batches only; cutout and
  transparent pipelines remain follow-up work.
- `sindri-render` now owns persistent textured-mesh GPU buffers keyed by an
  opaque cache identity and monotonic revision. Unchanged meshes reuse their
  buffers, pending replacements keep the last uploaded geometry drawable, and
  explicit release drops meshes that leave residency. Cache counters expose
  installs, uploads, reuse, stale draws, misses, and releases for diagnostics.
- Persistent textured meshes use 32-bit indices, so a highly fragmented 16³
  voxel section is not limited by the 65,535-vertex ceiling of transient
  authored meshes.
- Voxel mesh work now carries a monotonic section revision and meshing profile,
  and a renderer-independent persistent cache keeps old compiled geometry
  available while a replacement is built. Superseded worker results cannot
  replace newer terrain, and one removal releases every cached profile of a
  leaving section.
- `sindri-voxel` now compiles neighbour-aware block sections into indexed
  CPU geometry split into opaque, cutout, and transparent passes. Material and
  face identity remain semantic so renderers can choose their own atlas or
  shader path, while configurable occlusion avoids internal transparent faces.
- Causeway can derive a smoothed textured triangle surface from the same
  deterministic voxel terrain that still owns picking, building and navigation.
  The first slice intentionally meshes only one 16×16 camera-centre chunk so
  the visual approach can be judged before voxel generation and chunk meshing
  are promoted into a general engine subsystem.
- `sindri.mesh` can carry explicit textured surface triangles, including a
  sprite-sheet region, through the opaque 3D render path.


- Tile volumes expose one shared engine chunk coordinate and sparse runtime
  chunk store. Scene serialization remains the readable sparse cell list; the
  runtime store is the unit generators, renderers, and future persistence use.
- `Grid.walkable(floor, x, y)` tells a script whether a walker can stand where a
  point falls, read from the same walkable surface the pathfinder uses. A script
  could previously ask only about a route between two entities, so a game moving
  its own player had no way to ask about terrain at all.

### Fixed

- Causeway's Wanderer now crosses grid steps smoothly, and the Play camera
  holds a small dead zone before easing after it. The generated scene also keeps
  its intended central start instead of a stale override placing it beside the
  world edge, so BUILD panning has terrain around it in every direction.
- Causeway Play taps now move their runtime Target instead of having its
  authored cell snap it back every frame, and use the voxel's grid row rather
  than its height, so the Wanderer walks toward the place the player tapped.
- The editor no longer draws a tile volume as a comb of vertical stripes after
  the first edit. A volume names a tile set rather than a texture, so nothing
  about the world asked for the sheet that cuts its blocks: the tile set's
  arrival requested it, and the next pass over what the scene references —
  which runs on every edit — found nothing claiming it and released it. An
  unbound sheet does not blank the texture, it unbinds the slicing, so every
  face resolved the whole strip at once.

- Gather's player no longer walks across its moat or into its outcrop. It
  compared positions against tagged props, which are entities; water and the
  hill are not, so neither stopped it while the Wisp routed around both.
- Gather's player no longer starts inside the hill. Its scene authored no
  starting position, so it began at the world origin — the middle of a 25x25
  island, which is the top of the outcrop. It now starts on the path just north
  of the ridge.
- A walker standing over water is no longer lifted onto it. An authored occupant
  rests on whatever holds it up, but a walker stands only on what it can stand
  on.

- The editor's grid chooser offers entities carrying `sindri.tile_grid` as well
  as `sindri.tilemap`. It asked for the flat map alone, and no scene has carried
  one since Gather moved to a volume, so a grid could not be named at all.

- Flat ground no longer draws over what stands on it. Something placed on a
  grid sorts past ground no higher than its feet, since no face of such a cell
  can cover it; a block raised a step ahead is a wall and still covers it.
- A tile volume's depth is taken from a cell's column rather than from each
  drawn face, so raising a block no longer moves it toward the viewer and a
  block's own faces are no longer sorted against each other.

### Changed

- A grid placement names its grid with the same stable-ID type a grid occupant
  uses, and carries the cells it covers. Standing something over a hole is now
  an error naming that cell instead of placing it at height zero.

- A tile says whether its top holds anything up (`supports`) and whether a
  walker can stand on it (`walkable`) instead of one `solid` flag that meant
  both. Water supports without being walkable, so a pond is no longer
  indistinguishable from a hole. `solid` still reads as `walkable`.

- Resolving a tile volume indexes its cells once instead of scanning them for
  every neighbour it asks about, and no longer revalidates a bound tile set on
  every frame.

### Added

- A tile can declare several looks with weights, chosen per cell from a stable
  hash of where the cell is, the tile's name and the volume's `variant_seed`.
  One tile ID replaces a family of near-identical ones, and the same field
  looks the same on every machine and after every reload.

- Choosing a `.prefab.json` in the project browser arms it, and clicking a cell
  in the Scene view puts it there: one undoable step, standing on that cell of
  that grid, keeping whatever footprint the prefab declares.

- The editor can read a `.prefab.json` and put it into the open scene as one
  undoable step, with every entity given a stable identity nothing else is
  using. Distinct from the runtime's spawn, which deliberately gives none.

- `sweep_occlusion` walks a virtual actor over every standable surface of a
  scene and reports where something that cannot cover it is drawn over it
  anyway, with the reason attached. Gather's farm is swept in its own test.
- The editor draws that report on the grid it describes, under Build →
  Ordering, so a run of faults along one edge reads as one cause.
- Entities can be placed by naming a grid cell rather than a world position.
  `sindri.grid.placement` derives the transform from the cell, including the
  height of the ground in that column and the Z that orders it, so raising the
  ground raises what stands on it and nothing authors a draw order.

### Changed

- Draw order in a 2D scene is derived from where a thing stands rather than
  from an authored render layer. Gather carries no layer on any world sprite,
  its player and wisp scripts no longer compute one, and `layer_step` on tile
  volumes is replaced by the grid's `depth_step`.
- Gather's floor is a stacked tile volume rather than a flat tilemap, and no
  scene in the project carries `sindri.tilemap` any more. Water is no longer
  walkable, so the moat and pond now bound the island.

### Added

- Tile volumes can spread their cells across render layers with `layer_step`,
  which is what lets blocks interleave with sprites in a 2D scene.
- The isometric baker can give a material grain: a texel grid that shifts each
  texel a step along the ramp it already has, so a baked block reads as a
  surface rather than as three flat faces. Gather's blocks are rebaked with it,
  and the set grows from three tiles to twelve, half-height slabs included.
- Decay reaches a stacked volume: `Grid.block` and `Grid.set_block` read and
  write the tile in one cell at a column, row and level, and a script's
  pathfinding now sees the holes and walls a volume floor makes.
- Grid navigation derives what a stacked volume allows: a column with nothing
  solid in it is a hole, and `sindri.grid.navigation` gains `max_step` deciding
  how big a step between columns a walker may take, with a slab counting as
  half. A volume only becomes the floor once the flat map is gone.
- Tile volumes work in orthogonal projection as well as isometric. The
  projection now decides which faces a view can see and how cells are ordered
  back to front, so one tile set serves both.
- Tile set tiles can declare how much of their cell they fill, so half-height
  slabs and other partial blocks are logical cells rather than art tricks.
  Face culling now hides a face only when a neighbour covers it completely.
- Grid position, placement, pathfinding and navigation now accept a floor
  carrying `sindri.tile_grid` instead of `sindri.tilemap`, so a scene can move
  onto a stackable tile volume without its scripts, walls or occupants moving
  with it. `Grid.tile` and `Grid.set_tile` remain flat-map calls and say so.
- Added an optional advanced sprite colour transform with independent RGBA
  multiply and offset, alongside the existing simple tint, reachable from
  scenes, the editor and Decay.
- Added a typed batch preflight and agent-facing runtime-contract guidance for
  Decay gameplay scripts.
- Added multi-scene projects and runtime scene switching, including persistent
  scene state, exported secondary scenes, and Decay scene navigation.
- Added reusable profile assets with runtime, editor, Decay, and export support.
- Added Tile System 2 foundations for stackable isometric volume cells and
  globally correct transparent painter ordering.
- Added a substantially expanded Gather showcase with a larger farm, multiple
  playable places, camera following, richer baked environment art, and
  script-editable grid tiles.
- Added broader Decay gameplay APIs, including runtime signals and improved
  environment interaction support.
- Added extensive Orbital Last Stand and Orbital Baked gameplay, boss, module,
  UI, hazard, combat-lab, and visual-parity work.
- Added a managed local-AI runtime path using llama.cpp, model manifests,
  hardware-aware compatibility grading, and continued Ollama support.
- Added major editor authoring improvements across project browsing, component
  editing, asset pickers, snapping, play controls, scene handling, diagnostics,
  viewport behavior, and native rendering.
- Added export/browser hardening, asset manifests, verification, hot reload,
  sprite sheets and animation, GPU-backed rendering tests, and additional
  project/scene validation.
- Added isometric baker improvements and reproducible baked-art workflows used
  by Gather and Orbital.

### Changed

- Reworked Sindri's editor around native `egui`/`wgpu` rather than the earlier
  Tauri/React direction.
- Evolved the scene format through multiple migrations as transforms, sprite
  sheets, component namespaces, cameras, and UI ownership were clarified.
- Consolidated 2D and 3D transform/rendering behavior so world sprites, UI,
  cameras, animation, physics, grids, and scripts share clearer runtime
  contracts.
- Expanded Gather's role from a passive capability showcase into a playable
  farming-game proving ground that may drive reusable engine features.
- Reworked Orbital's baked assets and bosses so authored rotations, silhouettes,
  attacks, reactions, and environment interactions survive the baking pipeline.
- Raised the Rust MSRV as required by current `wgpu` and `egui` releases.

### Fixed

- Fixed exports dropping a sprite sheet whose texture is named from the project
  root rather than from `assets/`, which shipped the texture with no slices and
  drew an animated sprite as its whole sheet in one quad.

- Fixed native and browser hosts discarding the underlying cause of application
  failures at their reporting boundary.
- Fixed exported browser projects failing before asset fetch when one asset kind
  exceeded the loader's default queue capacity.
- Fixed numerous editor issues involving selection, unsaved changes, undo/redo,
  viewport color handling, hierarchy layout, project browsing, asset loading,
  console reporting, and controls that previously did nothing.
- Fixed browser/export failures involving missing scripts, incorrect asset kinds,
  manifest handling, startup errors, and project-relative asset paths.
- Fixed rendering bugs involving multi-batch cameras, transparent ordering,
  sprite-sheet frames, world/screen sprite behavior, viewport color spaces, and
  multi-mesh clears.
- Fixed gameplay/runtime bugs involving input edges, inactive scene lookups,
  scene identity handling, projectile hit ordering, camera activation, enemy
  placement, hazard persistence, and physics interactions.
- Fixed Spine sections failing at runtime when a destroyed middle section
  severed and promoted the surviving rear chain.
- Reworked Spine's body to follow one exact cardinal route instead of being
  dragged around corners like a physics rope, including after a severed rear
  chain becomes independent; Spine now hunts through the arena rather than
  circling its boundary.
- Fixed the isometric baker's authored rotations, which had been supplied in
  radians while the baker interpreted them as degrees.

## Changelog policy

Keep entries release-oriented and readable:

- Record changes that matter to users, game authors, plugin/tool authors, or
  downstream integrators.
- Group entries under `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`,
  or `Security` where useful.
- Prefer one concise entry for a feature or behavior change. Do not append a
  debugging diary, implementation essay, test narrative, or commit-by-commit
  history.
- Put detailed rationale and technical history in the pull request, relevant
  documentation, or architecture decision record.
- When the first release is cut, rename `Unreleased` to that version and date,
  then add a fresh empty `Unreleased` section above it.
- Added the authored `sindri.environment` presentation component and connected the existing bloom renderer to editor viewports and Voxel Lab, making bloom scene-controlled instead of a stranded renderer-only capability. Voxel Lab now serves as the acceptance lab for the world-presentation roadmap.

