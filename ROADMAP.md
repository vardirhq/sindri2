# Sindri Next roadmap

This is the checkable engineering plan. Check an item only when its acceptance criteria and relevant tests are complete. The roadmap is ordered by dependency, not excitement.

## Milestone 0 — Architecture and repository baseline

- [x] Review the vision against the legacy `sindri-engine` implementation
- [x] Record feasibility constraints and major risks
- [x] Create a Rust 2024 workspace with an explicit MSRV (Rust 1.95 for `wgpu` 30 and `egui` 0.36)
- [x] Add formatting, Clippy, test CI, and dependency caching
- [x] Establish `sindri-core` and the public `sindri` facade
- [x] Add `CONTRIBUTING.md`, code of conduct, and dual-license files
- [x] Add dependency policy and automated dependency/security review
- [ ] Decide versioning policy for Rust crates, scene files, editor protocol,
  and any future browser embedding SDK (crates and scene files decided; the
  protocol and optional SDK are deferred until they exist rather than guessed at)
- [ ] Add release/changelog validation (deferred until there is a release process to validate)

Exit gate: a clean clone passes format, lint, and test checks on the declared MSRV.

## Milestone 1 — Renderer-independent engine core

### Lifecycle and time

- [x] Define and test legal engine lifecycle transitions
- [x] Implement capped frame delta and fixed-step accumulation
- [x] Prevent a fixed-update spiral of death
- [x] Define pause semantics
- [x] Add time scale without accumulating floating-point drift
- [x] Define lifecycle hooks and error propagation used by native and web hosts
- [x] Add a platform-independent `EngineCore` advanced by runtime hosts
- [x] Add deterministic clock/test host

### Entities and world

- [x] Add generation-checked runtime entity handles
- [x] Add safe spawn, access, recursive destruction, and slot reuse
- [x] Add hierarchy parenting with cycle prevention
- [x] Keep serialized logical IDs separate from runtime handles
- [x] Define typed component registration and metadata
- [x] Add query API for built-in and registered component types
- [x] Add deferred world command buffer
- [ ] Specify deterministic system ordering
- [x] Benchmark 1k, 10k, and 100k entity workloads before considering an archetype ECS — see `docs/entity-scaling.md`; it found quadratic scene validation rather than an entity-storage problem, and an archetype ECS is not warranted

### Scenes and serialization

- [x] Add versioned scene document and stable logical entity IDs
- [x] Validate duplicate IDs, missing parents, and hierarchy cycles
- [x] Load a validated scene into a runtime world in two passes
- [x] Preserve forward-compatible JSON component payloads
- [x] Define component schema registry and unknown-component behavior
- [x] Save a runtime world back to a scene without losing stable IDs
- [x] Add canonical pretty serialization for reviewable diffs
- [x] Add fixtures and golden round-trip tests
- [x] Define scene migration API before format version 2 exists
- [x] Split editor-only metadata into a namespaced, ignorable section

Exit gate: core runs without GPU/window/browser dependencies and scene fixtures round-trip deterministically.

## Milestone 2 — Platform and GPU bootstrap

### Boundaries

- [x] Add `sindri-platform` traits for lifecycle, input source, clock, and asset I/O (asset I/O stays in `sindri-assets`)
- [x] Add desktop adapter using `winit` — window, event loop, frame timing, and input all live in `sindri-desktop`
- [x] Add web adapter using `wasm-bindgen`, `web-sys`, and async initialization
  — the `winit` host serves the browser through the same event loop, canvas
  attachment, and asynchronous device request; applications currently expose
  their own narrow `wasm_bindgen` entry point
- [x] Keep target-specific conditionals inside platform hosts — window, canvas, future spawning, and the clock live in `sindri-desktop`; each application still picks its own logger, which is an application choice rather than a host detail
- [ ] Produce explicit capability errors for unavailable WebGPU/surface features

### GPU foundation

- [x] Add `sindri-gpu` adapter/device/queue ownership
- [x] Negotiate conservative cross-target limits and features
- [x] Add shared surface configuration and non-zero resize policy
- [x] Centralize surface loss, outdated, occlusion, and timeout recovery after the proof example
- [x] Add resource labels and actionable adapter/device/surface errors
- [ ] Define typed buffer, texture, sampler, shader, and pipeline wrappers
- [x] Add render-target and depth-target management — `ViewportTarget` owns a colour target, both of its views, and the depth buffer sized with it
- [x] Prove headless adapter initialization on the Linux CI runner with Mesa software Vulkan

### Proofs

- [x] Render a triangle natively through a shared renderer
- [x] Render the same triangle in a browser through WASM/WebGPU
- [x] Resize correctly on desktop and browser
- [x] Recover or fail clearly on surface loss

Exit gate: the same render module displays the triangle on native and web hosts.

## Milestone 3 — Minimal shared renderer

- [x] Define frame extraction/preparation/render stages
- [x] Define cameras, viewports, clear operations, layers, and explicit render-pass ordering
- [x] Add camera matrices and projection math tests
- [x] Add depth buffer and colored cube
- [x] Add texture upload, sampler, and textured cube
- [x] Add mesh buffers, indices, vertex layouts, and reusable mesh resources
- [x] Add orthographic camera and textured sprite
- [x] Add configurable sprite blending and deterministic transparent ordering rules
- [x] Add instanced sprite batching with measured five-to-one draw-call reduction
- [x] Render a 3D cube and 2D sprite/overlay in one native/WebGPU runtime
- [x] Add minimal offscreen rendering test harness with a deterministic 512×512 PNG artifact

Exit gate: a serialized scene drives a combined native 2D/3D example without renderer-specific scene data.

## Milestone 4 — Asset system

- [x] Define logical `AssetId`, typed handles, states, errors, and reference lifetimes
- [x] Define asset source abstraction for filesystem and HTTP/fetch
- [x] Add async load queue that does not fake synchronous browser I/O
- [x] Load textures and scene JSON, driven end to end by `AssetLoader` and used by the editor
- [x] Add fallback/error assets and actionable diagnostics (the missing-texture checker draws in place of an unresolved reference, and every load failure names the asset, the source, and what went wrong)
- [x] Add asset root and URL resolution rules
- [x] Add hot reload for native development (assets; editing the open scene file on disk is not watched)
- [x] Define package/export manifest with content hashes
- [x] Prevent duplicate loads and release unused GPU assets
- [x] Add image decoding compatibility tests on native and WASM

Exit gate: the same logical texture and scene references load from disk and static web hosting.

## Milestone 5 — Browser runtime and delivery

The original milestone specified a first-class TypeScript gameplay SDK. Decay
is now the common gameplay layer for native and browser builds, so the browser
work here is about running, loading, diagnosing, and shipping the same project.
A TypeScript embedding API remains possible if a concrete web-application use
case justifies a second public authoring surface; it is not a first-release
requirement.

### Runtime

- [x] Run the shared `winit` host, WGPU renderer, and engine lifecycle on a page canvas
- [x] Run Decay gameplay, fixed-step simulation, keyboard input, entity references, and the shared game board in Chromium
- [x] Make asynchronous host failures visible in the browser console rather than returning success before startup fails
- [ ] Exercise `AssetLoader` and `UrlRoot` with real browser fetches rather than embedded bytes
- [ ] Define page lifecycle semantics for resize, visibility, teardown, and device loss
- [ ] Add an explicit capability error and user-facing message when WebGPU is unavailable

### Browser quality and packaging

- [x] Add a Playwright smoke test that proves the engine configured the canvas
- [ ] Test missing WebGPU messaging
- [ ] Test canvas selector/element validation
- [ ] Test resize, device-pixel ratio, visibility pause, and teardown
- [ ] Test static hosting under a non-root base path

Exit gate: a Sindri project can be exported to static files, load its assets
through the real pipeline, and run the same Decay gameplay and scene in a
supported browser as it does natively.

## Milestone 6 — Focused 2D migration

An item here is finished when the editor understands it, not when the runtime
does. The current product direction makes that a project rule and Milestone 9
lists its editor tools accordingly; this milestone listed only runtime halves,
so the authoring surfaces are named explicitly below.

They follow their runtime half rather than lead it. A sheet editor edits a
format, so the format, its serialization, and its renderer come first —
designing a data model through the tool that paints it is how the model ends up
shaped like the tool.

- [x] Collapse `Transform2D` into `Transform3D` as scene format version 2, so sprites and meshes share one world — the model is in `docs/2d-model.md`; the render is pixel-identical across the change, and a version 1 fixture migrates to exactly the version 2 one stored beside it
- [x] Give sprites a world-space option, keeping screen-anchored as the default so overlays and both proofs are unchanged — a sprite carried a `space`, and the two never share a batch because they differ in camera and pipeline; the offscreen capture is byte for byte what it was. Format 8 split that field into two components: `sindri.sprite` is the world one and `sindri.ui.image` is the screen one, because the anchor a screen sprite is positioned by decided nothing on a world one and a component that hides half of itself is two components
- [x] Sort transparent sprites by camera distance rather than an authored depth field, with layer as the explicit override — scene format version 3, whose migration turns a screen sprite's depth into the Z that now orders it; a world sprite reorders when the camera moves, a screen sprite's Z orders it without moving it
- [x] Add 2D-shaped transform accessors that take and return X and Y only, so the common way a layered scene gets flattened is not expressible — position, translation, scale, and the turn about Z, on `Transform3D` itself
- [x] Add a Z lock a transform can declare, respected by checked write paths and visible in the inspector — `Transform3D::z_locked`, refused by `WorldCommand::SetTransform3D`, and shown in the inspector as a toggle that takes the Z drag away
- [x] Inventory each legacy 2D subsystem as port, refactor, replace, or defer — `docs/2d-inventory.md`, read from the legacy code at `77e0489` rather than from memory of it
- [x] Give a sprite a UV rect, so it can address part of a texture — the inventory found that sprite animation and tilemaps both block on this; `UvRect` is checked at construction and normalized so a grid slice does not depend on the sheet's resolution, it rides on the instance so a sheet stays one draw call, and a GPU test reads the pixels back to prove the shader honours it
- [x] Port sprite animation and sprite sheets — `sindri.animation.sprite` carries clips of named sprites and which one plays, while `SpriteAnimations` holds the cursor beside the world, so a scene saved mid-run is the scene that was opened; the legacy engine's texture-per-frame becomes a rect into one sheet
- [x] Give a sliced image somewhere to say how it is cut — a sheet document beside the texture at a derived ID, naming its parts as a grid or as explicit rects, with scenes referring to them as `textures/tiles.png#floor`. Three components each carried their own copy of a sheet's layout and could disagree; none carries one now. Scene format 4, with a migration that recovers a rect's cell without being told the grid
- [x] Add a sprite sheet authoring surface: selecting a texture opens a slicer showing the image with its grid drawn over it; columns and rows are drags, every cell can be named, and Save writes the sidecar so the browser and component editors share the same sprite names. The Sprite Animation inspector creates, renames, and removes clips, arranges their named frames, edits timing and looping, chooses the runtime clip, and previews playback against the real project texture without advancing scene state
- [ ] Port camera 2D behavior and pixel snapping — partly done: orthographic
  pixel snapping works; follow/dead-zone/smoothing behaviour remains
- [x] Port tilemap data model and renderer — `sindri.tilemap` holds a palette of sheet sprite names, the map grid, and a flat array of cells with `null` for empty, and extracts into the same sprite batches loose sprites use, so a prop sorts among the floor rather than behind it; it gained a projection the legacy type did not have, because the first thing to use it was an isometric floor, and placing a tile and finding the tile under a point are inverses on every cell of both
- [x] Add tilemap authoring: selecting a map shows its sliced texture as a visual palette, primary drag paints or erases through the exact Scene-view camera, grid resizing preserves the overlap, and the stored render layer remains editable; a stroke is one undo step. A map is now always in the world, so the screen-space case that stayed data-only is gone rather than waiting
- [x] Port text rendering with a web-safe font asset strategy — `sindri.ui.text`
  extracts anchored overlay strings into ordered frame commands, `glyphon`
  shapes and rasterises them through wgpu, and fonts are validated project
  assets bound from bytes rather than operating-system families. Gather embeds
  Inter and draws real text through this path natively, in browser builds, in
  the editor, and in its offscreen capture
- [x] Add font and text authoring: the inspector edits multiline content and
  chooses from the project browser's OpenType font assets; Add Component offers
  UI Text only when a project font can complete its otherwise dishonest blank
- [ ] Port particles after the render lifecycle is stable
- [ ] Port layers, anchors, and sprite bounds — layers work, and the nine
  anchors are a field of the UI family rather than of every sprite; editor picking uses the renderer's unit-quad geometry directly,
  while reusable component bounds remain. Parallax is not
  ported, because under one world with real depth it is what a perspective
  camera already does; see `docs/2d-model.md`
- [x] Port A* pathfinding into a renderer-free grid crate, with the general grid beside it — `sindri-grid` now has deterministic cardinal/eight-way A*, explicit corner policy and costs, memoized passability, and whole-footprint occupancy paths; the legacy platform-jump graph remains deferred
- [x] Add optional Rapier2D adapter without core dependency, with collision layers from the start — physics ignores Z, so depth cannot keep a parallax background out of the player's way, and collision layers are not render layers; `sindri-physics` masks Rapier entirely, and `sindri.physics2d.rigid_body` and `sindri.physics2d.collider` author bodies into a scene
- [ ] Finish 2D physics across the surfaces it is meant to reach — collider outlines and shape handles in the editor rather than only the generic component inspector, typed Decay access, and a Gather behaviour a body actually drives; the runtime is complete and the feature is not, which is what this item exists to say
- [ ] Add 2D pan/zoom viewport controls, which need a 2D scene rather than the screen-anchored overlay the demo uses
- [x] Run the engine in a browser at all, which it never had been — two things had been broken since the browser target was added: a failure was recorded and never reported, because `spawn_app` hands the loop to the page before `run` returns, and the surface refused every canvas because a canvas offers no sRGB format. See `docs/browser.md`; `scripts/browser/smoke.mjs` is the check
- [ ] Start the companion game and grow it through this milestone, verified natively and in browser — see "The companion game" below, which replaces the `hello-2d` and platformer slice this milestone used to schedule (started: `game/` plays natively and in a browser, with a scripted offscreen capture in CI; animation, its tilemap floor, and project-font title work, while authored parallax remains)

Exit gate: Sindri Next matches the useful core of legacy 2D without inheriting its desktop/server/Lua coupling, and each ported system can be authored in the editor rather than only by hand in JSON.

## Audio — unnumbered on purpose

Audio depends on the platform boundary (Milestone 2) and the asset system
(Milestone 4) and on nothing else, so it does not sit naturally before or after
the 2D migration. It can be taken whenever a game needs to make a sound.

It is unnumbered rather than inserted as a milestone because renumbering seven
milestones to record one omission costs more than the omission does. It belongs
beside the asset system, behind the platform boundary, and explicitly outside
the core crates; no numbered milestone had scheduled it, so the plan once
described an engine that could not make a sound.

- [x] Define an audio asset type and its decoding path alongside textures
- [x] Add a platform audio boundary with a silent implementation, so tests and CI need no sound device and a headless run can still assert what was asked to play
- [x] Play one-shot sounds from gameplay through that boundary
- [x] Add looping music with volume, and tie stop and resume to the engine lifecycle so pausing the game pauses the sound
- [x] Add a browser backend behind the same boundary, including the user-gesture requirement browsers impose before any audio may start
- [x] Let a scene reference an audio asset through a component, and surface audio in the editor's project browser (the browser lists audio; the editor cannot play a clip, and nothing gathers the audio a scene names the way `referenced_textures` does)

Exit gate: the companion game plays a sound when something happens, natively and
in a browser, and a headless test can assert it was asked to without a sound
device existing. Met: `scripts/browser/smoke.mjs` watches what each `play()`
promise settles to, so a clip a browser refuses is a failed check rather than a
console line nobody reads.

## Milestone 7 — Editor/runtime protocol and minimal editor

- [x] Choose and document a native Rust `egui` editor with in-process WGPU viewport integration
- [x] Establish a deliberately styled, resizable native editor shell
- [x] Load the real demo scene for a read-only hierarchy and transform-inspector proof
- [x] Capture the complete native editor window as a deterministic CI screenshot artifact
- [ ] Define versioned editor protocol and capability handshake
- [x] Load/save the real scene format
- [x] Render the actual Sindri runtime in the viewport through the editor's shared WGPU device
- [x] Display hierarchy from runtime scene state
- [x] Inspect/edit names, hierarchy, Transform2D, and Transform3D — reparent through the inspector's `Parent` menu or by dragging onto a GameObject or the World root; both offer only moves `World::check_set_parent` allows
- [x] Add selection and transform change commands
- [x] Add command-based undo/redo with transaction grouping
- [x] Add 3D orbit, pan, and zoom camera controls, with a reset to the authored camera
- [x] Add play, pause, stop, and reset-to-authored-state
- [x] Add one sprite, one cube, and one camera editor fixture — `editor/assets/fixture.scene.json`; it holds two cameras rather than one, because a mesh needs a world camera and a sprite resolves its anchor against an overlay camera
- [ ] Add protocol contract and save/reload integration tests (save, reload, and undo are covered end to end in `editor/tests/fixture_scene.rs`; the protocol contract waits on the protocol)

Exit gate: editing a transform, saving, and reopening produces the same visible native/web scene.

This milestone is about the editor understanding the runtime model. Whether the
editor is pleasant to use for an afternoon is tracked separately, below.

## Milestone 8 — Basic 3D product capability

- [ ] Finalize mesh and material public APIs
- [ ] Add glTF 2.0 mesh/material import using an established loader
- [ ] Add normals, UVs, and basic unlit/standard material paths
- [ ] Add directional light and ambient/environment term
- [ ] Add frustum culling and mesh instancing after profiling
- [ ] Add optional Rapier3D adapter
- [x] Add editor transform gizmos — translate, rotate, and scale use the exact
  Scene-view camera, local/world orientation, optional snapping, checked Z
  locks, and command-history drag merging
- [ ] Add camera gizmos and collider visualization
- [ ] Put a 3D prop into the companion game's sprite scene, which is what `hello-3d` and a navigable scene were for
- [ ] Validate representative integrated and discrete GPUs

Exit gate: imported glTF content, camera, lighting, and collision work in native, browser, and editor preview.

## Milestone 9 — Isometric and grid module

- [x] Extract coordinate/projection lessons from IsoGame without app-specific behavior — `docs/grid.md` keeps the reversible diamond projection and separates it from IsoGame's canvas centring, pan, zoom, room state, and actor-aware pathfinding
- [x] Add tested orthogonal/isometric grid coordinate types — `sindri-grid` is dependency-free and owns signed cells, continuous grid/plane points, finite bounds, stable neighbour order, and validated projection spaces
- [ ] Add world/grid/screen conversion with round-trip property tests — partly done: both projections round-trip logical coordinates and arbitrary plane points across negative and positive space, upward/downward plane Y adapts world and screen conventions, and tilemaps now expose the exact grid space used by rendering and picking; camera/viewport transforms remain
- [x] Add footprints, occupancy, placement validation, and wall edges —
  `sindri-grid` now owns normalized footprints, atomic bounded occupancy, and
  symmetric internal wall edges consumed by A*; authored engine components and
  a validated world adapter now derive them from scene state, while editor and
  Decay adapters remain tracked in the integration matrix
- [ ] Add pathfinding overlays and interaction points — partly done: renderer-free walls, A*, footprint-aware occupancy paths, and the engine world adapter exist; overlays, authored interaction points, editor tooling, and typed Decay access remain
- [ ] Define deterministic sprite-isometric depth keys
- [ ] Add integer zoom and generated-anchor metadata
- [ ] Add orthographic 3D isometric camera helpers
- [ ] Add editor grid, tile, footprint, and height-layer tools
- [x] Move the companion game onto this module, replacing the coordinate handling it grew of its own — Decay's typed `Grid` surface reads and places continuous coordinates through the floor tilemap, and Gather now moves, clamps, and gathers in that logical space without adding game rules to Rust

Exit gate: shared grid/gameplay logic supports both sprite isometric and orthographic 3D presentation.

## Milestone 10 — Tooling, distribution, and 1.0 hardening

- [ ] Implement minimal `sindri new/dev/build/test/editor` CLI
- [ ] Add native and web project templates to `sindri new`
- [ ] Add native and static-web export pipelines
- [ ] Add curated examples with CI build coverage
- [ ] Add Editor/Decay and CLI/Decay getting-started guides, plus Rust extension guidance
- [ ] Document supported browsers, GPUs, OSes, and MSRV
- [ ] Add CPU, GPU, memory, startup, and WASM-size benchmarks
- [ ] Add screenshot/render regression suite where stable
- [ ] Audit unsafe code policy and dependencies
- [ ] Stabilize public API and document deprecations
- [ ] Complete license, notices, release notes, and publishing dry run

Exit gate: a clean generated project can be authored through Editor/Decay or
CLI/Decay, tested, and exported for native and static web targets with documented
support and stable public contracts.

## Milestone 11 — The editor as a working tool

The first major release needs the editor of Milestone 7: open a project, inspect
entities, edit a transform, and preview the real scene. This milestone is the
distance between that and an editor someone would choose to spend a day in.

It comes last because it is where every other milestone's authoring surface
converges. Editing a sprite sheet needs sprite sheets; painting tiles needs
tilemaps; a gizmo needs something to transform. Those live with their features
in Milestones 6, 8, and 9 rather than here, and what is left below is the work
that belongs to no single engine feature.

Full-fledged means complete for what Sindri does, not feature-for-feature with
Unity — the README rules that out, and an editor that grows ahead of
the engine would be authoring things nothing can run.

### Authoring the world

- [x] Create and delete entities from the interface — deleting takes the subtree, and undo restores every entity at its own handle, so nothing holding one is left pointing at nothing
- [ ] Duplicate an entity, and duplicate a subtree
- [x] Add and remove components through the inspector, driven by the schema registry rather than a hardcoded list — Add Component offers what the entity lacks and the registry can create, and a type with no sensible blank is left out rather than offered and refused
- [x] Edit any component's fields in the inspector, including one the engine has never heard of — driven by the stored payload, checked against the component's own schema before it becomes a command
- [x] Edit a script's `@export` properties in the inspector, drawn from what the script declared — the capability that justified a statically typed language
- [ ] Author a Decay script's **source** from the editor: open one, edit it, create one
- [x] Reparent by dragging in the hierarchy, with legal/illegal target feedback, cycle prevention, root drops, automatic target expansion, selection of the moved entity, and one-step undo through the existing command
- [ ] Multi-select, and edit what a selection has in common
- [ ] Copy, paste, and duplicate across scenes
- [ ] Decide whether Sindri has prefabs, and if so what a prefab override is

### Working on a project

- [ ] Open a project rather than a single scene, and manage more than one scene at a time — partly
  done: a project is a directory holding `sindri.toml`, the welcome window lists, makes and opens
  one, a scene opened from anywhere adopts the project it is inside, and the browser roots at it;
  more than one scene at a time does not exist
- [ ] Import assets from a watched directory, decoding and registering them without a rebuild
- [ ] Surface project settings, whatever `sindri.toml` turns out to hold — partly done: the file
  exists and carries a format version, the project's name, and the scene it opens, which the project
  browser can now nominate; the name cannot be edited from the editor, and the runtime does not read
  the file at all
- [x] Show editor, script, render, and asset failures in the console; broader
  structured engine logging can grow behind the same surface
- [ ] Search and filter that reaches both the hierarchy and the project browser

### Running the game

- [ ] Play mode that runs the complete game loop against the edited world, with
  pause and single step — scripts and sprite animation already run; other game
  systems and single-step do not
- [ ] Native preview, and web preview through the actual WASM build
- [ ] Frame timing, draw calls, and entity counts where a developer can see them

### Shape of the tool

- [ ] Named layout presets, and panels that can be rearranged — `2 by 3` and
  `Wide` presets exist and persist; arbitrary rearrangement does not
- [ ] Script editing, or a clean handoff to the editor a developer already uses
- [ ] Expose editor actions as structured commands, which is what AI tooling operates through

Exit gate: someone can build one of the shipped examples from scratch inside the
editor — entities, components, assets, and all — without hand-editing JSON.

## Editor usability — continuous, not a milestone

The milestones above ask whether the editor understands the engine. This asks
whether it is bearable to sit in front of, which is a different question and is
not answered by planning: items land here because using the editor turned one
up, so the list is deliberately unordered and never finished.

It is kept because work that has no home is work the plan cannot see. Everything
below happened, or was wanted, off the back of actually opening the editor.

Done:

- [x] Open, save, and reload a scene file, with the open file and its unsaved state visible
- [x] A working File menu and Ctrl+S, replacing labels that did nothing
- [x] Settings that survive a launch, alongside the window geometry and panel sizes egui persists
- [x] A project browser list view showing each asset's kind, defaulting to list until thumbnails exist
- [x] A game view rendering the authored camera beside the scene view being edited
- [x] Undo and redo on the toolbar and on the keyboard
- [x] Inspector component rows that read the selected entity's own payload, replacing fixed text that described the demo scene whatever was open
- [x] Open a scene from the interface rather than only from the command line — **File → Open scene…**
- [x] Audit the editor control by control, rather than describing it from memory — `docs/editor-audit.md`
- [x] Make a hierarchy row selectable, which nothing could do since the first editor commit; every editing feature in the tool was behind it
- [x] Open a scene carrying components the built-in schemas do not know, rather than refusing it and panicking on the way
- [x] Ask before discarding unsaved work, and stop the Stop button resetting the scene
- [x] Make the unsaved marker mean the file and the world differ, on the back of `CommandHistory::revision`
- [x] Turn the scene view's axis indicator with the camera, drawn from the same view the frame under it was drawn through
- [x] Read the project browser from the directory the open scene lives in, filter it with the search box that used to filter nothing, and open a scene by double-clicking its row
- [x] Remove or implement every control that was drawn and did nothing, including the four tool modes and the two that lied
- [x] Show what the engine reports in the console — failures, what each scene turned out to be, and the textures it names that nothing has bound
- [x] Remember the open scene between launches, and name it and its unsaved state in the window title
- [x] Widen the viewport's zoom and pitch limits, make the zoom proportional, stop the orbit reaching the pole, and add **Focus selection**
- [x] Stop reporting a script that is still loading as a compile error — asset loading is asynchronous, so every cold open logged one error per scripted entity for the moment between the scene landing and its scripts arriving, and the console keeps what it is told; opening the companion game showed twelve errors against a game that was working
- [x] Let `scripts/capture-editor.sh` photograph a named scene rather than only the fixture, which is what taking the editor to the game needed
- [x] A welcome window, in a window of its own: the projects you have, the ones that have gone missing, a New Project that makes a manifest and a scene rather than a save dialog, and the shipped Gather project — replacing a launch that opened the demo scene compiled into the repository whatever you were actually working on

Wanted, in no particular order. Several of these are small enough to do the
moment they annoy someone, and are also named in Milestone 11 as part of the
larger tool; doing one here early is the point rather than a conflict.

`docs/editor-audit.md` found most of what follows and puts it in the order it
would do the most good, which is roughly the order it is listed in here. What
it found that was drawn and idle is done and has moved above; what is left is
what the editor cannot yet do.

- [x] Edit a rotation, which the format stores, the renderer applies, and the inspector used to print the word "Quaternion" for — the transform section drags it as Euler degrees and writes the quaternion back, so a rotation authored here is the one the renderer applies and the one the file keeps
- [x] Select world sprites, filled tilemap cells, and meshes by clicking in the Scene viewport; picking inverts the exact camera used to draw the frame, respects the renderer's local geometry, transparent layers and depth, and opaque occlusion, while an empty click clears the selection
- [ ] Asset thumbnails, after which the grid view is worth defaulting to again
- [ ] Show which entity a hierarchy row is, when its name is empty or repeated
- [x] Keep a filtered hierarchy readable: matches retain their full ancestor path, temporarily opened without changing the user's stored folds
- [x] Group a hierarchy of repeated entities with Unity-style GameObjects: every entity can own children, child-bearing rows collapse, fold state survives a restart, and the create menu adds either an empty root or a child
- [x] Give the console a way to clear or filter what it holds, since a transient error stays in the count long after it stops being true — Clear empties it, and the All/Problems/Errors filter is remembered across launches because someone watching for a failure is watching for a while

## The companion game — continuous, not a milestone

An engine with no game is a set of answers to questions nobody asked. This is
the game that asks them.

It is one game, built once and grown, rather than a demo per milestone. The plan
used to schedule six throwaway artefacts — `hello-2d`, a platformer slice,
`hello-3d`, a navigable scene, `iso-room`, and a scripted slice — each proving
one milestone and then rotting. That is the accumulation `CLAUDE.md` warns
against, arriving by instalments. Folding them into one living game is less work
than building six, and it is the only one of the seven that is still running in
a year.

**It is a game first.** Not a feature showcase. The rule that keeps it honest:
only add to the game what makes the game better. When an engine feature does not
improve it, that is information about the feature, not a shortcoming of the
game. A showcase never has to be fun, never has to ship, and never meets the
awkward case — so it applies none of the pressure that makes an engine good.

**It is isometric in the end,** because Milestone 9 already is, so the game sits on the
path rather than beside it; because isometric is sprites with depth sorting,
layers, and transparent ordering, which is what the frame pipeline already does
well; and because its needs are broad enough — grid maths, pathfinding, text,
input, UI, eventually 3D props — that building it will not quietly reshape the
engine around one genre.

**It is not an example.** `examples/` holds curated proofs, each earning its
place by proving one thing, and the game is not that. It lives in its own
top-level directory, is allowed to be messy in ways a proof is not, and the
triangle and cube stay as the neutral baseline that does not move when the game
does.

**It stays playable.** Every milestone that touches it leaves it running, with a
tagged snapshot, so the engine's progress is something to look at rather than a
list of ticks. A milestone that would leave the game broken is a milestone that
is not finished.

**It started one item early.** The plan held it until sprite sheets, world-space
2D, and text all existed, on the reasoning that without text there is no way to
tell the player anything. The game began before text arrived because scripting
landed first and a game was the only honest way to find out whether authoring
gameplay in Decay actually works. Its score and win state still communicate in
sprites even though its title now uses real project-font text: a row of lamps
for the score and a banner that fades in when the game is won. That remains
useful evidence about how much text a 2D engine really owes its users.

It is `game/`, crate `sindri-gather`: five orbs on a diamond floor, a thing you
drive with the arrow keys, and a lamp per orb. Its tilemap-based scene is 20
entities and all
four of its rules — moving, gathering, counting, winning — are Decay scripts.
There is no gameplay in its Rust, which is the claim it exists to test.

It has already paid for itself. Mixing a world with an overlay is something no
proof in the workspace did, and it turned up a renderer bug that made every
frame with more than one sprite batch draw through the wrong camera — see
`docs/rendering-frame-pipeline.md`.

Its diamond is now gameplay rather than presentation alone: Decay reads and
places entities through the floor tilemap's logical isometric coordinates, and
the same shared projection drives rendering and editor picking.

- [x] Start the game: one room, one character, one tile floor (started before text existed, which the entry above records)
- [ ] Grow it with Milestone 6: animation, a tilemap floor, parallax, and a font
  rendering real text — animation, the tilemap floor, the browser build, and a
  project-owned font rendering the title work; authored parallax remains
- [x] Take the Milestone 7 editor to it, and record what authoring it in the
  editor is actually like — `docs/editor-meets-the-game.md`; it opens, renders
  in both viewports, and Play runs its scripts, which is the parity claim
  holding. That first session found 49 floor rows drowning the hierarchy; the
  tilemap removed those rows, animation clips now have dedicated inspector
  authoring, world renderables can be selected in the viewport, and the
  hierarchy now folds any child-bearing GameObject while preserving readable
  search paths
- [ ] Give it depth with Milestone 8: a 3D prop in the same scene as the sprites
- [x] Rebuild its coordinate handling on Milestone 9's grid module rather than its own — player movement, floor bounds, and orb distance checks use the typed Decay grid surface backed by the floor tilemap
- [ ] Ship it through Milestone 10's export pipeline, natively and to the web,
  as the pipeline's real test — it is playable in a hand-built browser host, but
  no export pipeline produces that host yet
- [ ] Rebuild it inside the editor for Milestone 11's exit gate

Exit gate: the game is what someone is shown when they ask what Sindri is for,
and it runs on both targets at every tagged snapshot.

## Decay — the gameplay language, started ahead of this plan

**This track was begun deliberately out of order, and Decay is now the decided direction.** The advice recorded in `docs/decay-direction.md` was to defer a bespoke language and put Rhai behind a scripting host; that recommendation was not taken, and the question is closed — Sindri Next scripts in Decay, and no embedded language is adopted. `decay/` is a separate workspace with a lexer, parser, semantic analyzer, symbolic IR, and interpreter. The boxes below are the original research framing, kept because the questions are still the right ones, with the state of each marked honestly.

What the language has and does not have is in `decay/README.md`, and the engine-facing summary is in `docs/capabilities.md`. The rule that keeps this affordable: **nothing under `decay/` may depend on a `sindri-*` crate, and no engine crate depends on Decay.** The language is replaceable for exactly as long as that holds.

The next item is not more language. It is `sindri-decay` — one script driving one transform in the editor. The foundation reached three thousand lines with no engine caller, and the first thing that ran a script found three faults reachable from the first line anyone would write, including one that aborted the process. This repository's specific failure mode is depth without a caller, and `docs/capabilities.md` exists because of it.

- [x] Evaluate a custom language against embedded-language alternatives using explicit product and maintenance criteria (`docs/decay-direction.md`, including a Rhai spike)
- [x] Prototype a Rust-implemented portable interpreter that behaves consistently on native and WebAssembly without a JIT (the IR is symbolic and portable, and the whole workspace compiles for `wasm32-unknown-unknown`, which Decay's CI now checks — it is not yet a bytecode VM, but scripts now **do** execute on the browser target: the companion game is playable in Chromium, entity references and all, see `docs/browser.md`)
- [x] Bind Decay to the engine: a `sindri-decay` crate driving one script on one transform in the editor (`sindri.script`, a `WorldHost` mapping symbolic paths to a transform, and the editor's fixture spinning its cube from a `.decay` file — see `docs/scripting.md`)
- [x] Write a language reference a person or a model can work from, with its claims enforced by a test (`decay/LANGUAGE.md`)
- [x] Define typed host members so `this.transform.position` is checked instead of remaining an unknown path (`HostType`, `Environment::add_type`/`add_this_value`, and member resolution on reads, writes, and method calls — a misspelled path is now a compile error with a line number). Describing a host is gradual and per type: an undescribed type stays permissive, so a host part-way through describing itself rejects nothing that worked
- [ ] **Emit a host manifest**, now unblocked: `Environment` carries types and can enumerate them, so the description can be written out and read back. This is the first item of the external-editor track below, and the thing a language server has to agree with
- [x] Define a language-neutral scripting host: a script reaches its transform, its sprite, the keyboard, the frame's time, and a log, all through the `Host` trait's three methods and all typed. The surface is the one `docs/2d-inventory.md` read off the legacy engine's `player.lua` rather than an invented one — see `docs/scripting.md`
- [x] Give Decay a value that can hold an entity — `Value::Reference` is opaque to the language: holdable, passable, comparable, with no literal, no arithmetic and no way inside. The engine packs a slot and a generation into it, and the `Host` trait's three methods each take a subject, so a path rooted at a value a script holds resolves against what it names
- [ ] Extend the host to other entities, spawning, and despawning, routed through `WorldCommand` (partly done: `World.find`, `World.exists`, `World.despawn` and reaching through a reference all work and are typed — **spawning** is blocked on the engine having a prefab to say what to create, and despawn is not yet routed through `WorldCommand`, because no script write is and play mode restores from a snapshot)
- [ ] Generate typed component access, diagnostics, and autocomplete from the component schema registry (Decay's `Environment` is where host globals enter, and `IrField` already carries `exported` and `type_name` — this is the capability Rhai structurally could not have offered, and the reason the language has a case)
- [ ] Specify safe entity and asset handles, coroutine cancellation, deterministic scheduling, and execution budgets (call depth is bounded; the operation budget is slice 1 of the language track below, where it lands with the loops that make it necessary)
- [ ] Prove function/module hot reload with preservation or explicit migration of compatible typed state
- [ ] Document recurring gameplay-authoring pain points from Gather and other
  representative games, and feed them back into the Decay host and tooling
- [ ] Define the editor, language-server, formatter, debugger, documentation, and testing commitments before making the language public
- [ ] Decide whether the prototype provides enough gameplay-specific value to justify a permanent language ecosystem

### The language basics, ordered by what a script cannot say

An external audit of the language produced a twenty-item list of missing
fundamentals. Its inventory was almost exactly the "What does not exist" section
of `decay/LANGUAGE.md` — which is what that section is for, and the first
evidence it is doing its job. What follows is the **ordering**, because that is
the part worth arguing about: a twenty-item list of language fundamentals is a
feature checklist, and this repository's rule is that a capability is not
complete until Gather uses it in a real gameplay context. So each slice below
names what a script cannot currently say, and the script that proves it can.

Two consequences of ordering it that way rather than by familiarity. Collections
are pulled forward by Milestone 9's outstanding "typed Decay access" for
pathfinding rather than by a general appeal to inventories and waypoints — the
argument is below and it is concrete. Strings and formatting go to the back,
because the engine cannot display text: Gather's score is a row of sprite lamps
for exactly that reason, and a script that formats a string today can `print` it
and nothing else.

#### Slice 1 — loops, and everything that shares their machinery

One slice, not five. `decay-ir` already emits patched `Jump` and `JumpIfFalse`,
so all of this is the same work in the same place, and splitting it would pay
the cost of opening `decay-syntax`, `decay-semantic`, `decay-ir`, and
`decay-runtime` four times.

- [x] `while`, with `break` and `continue` — lowered through the patched jumps
  the IR already had, with one addition that is easy to miss: a `break` or
  `continue` emits the `ScopeExit` instructions it jumps over. A `return` can
  skip them because the frame goes with it; a `break` leaves the frame running,
  so a binding declared in the loop would otherwise outlive the turn that
  declared it
- [x] An operation budget in `decay-runtime`, reported as a value like every
  other runtime error, shaped after the call-depth limit: 1,000,000 instructions
  per outermost call by default, adjustable by the host, and counted so that a
  script cannot buy itself more by recursing. This is what makes `while` safe to
  offer — an infinite loop would otherwise take the editor and any unsaved work
  with it, which is what `CallDepthExceeded` already prevented for recursion
- [x] **Short-circuit `&&` and `||`.** Both operands used to be evaluated,
  which stopped being a curiosity when entity references landed:
  `target != null && target.transform.position.x > 0.0` read as a null guard,
  type-checked, and then faulted, because `docs/scripting.md` makes reaching
  through `null` an error deliberately. `decay-ir` now lowers both operators to
  branches, so the right operand is skipped when the left decides; the answer is
  still pushed as a `bool`, so the operators' type has not changed. Done ahead of
  the rest of this slice because it is branch lowering with no dependency on
  loops
- [x] `else if`, as a desugar in the parser rather than a new statement form:
  the tree it builds is `else { if ... }`, so a chain is nested blocks and
  scoping, lowering, and the analyzer keep working on one kind of conditional
- [x] `%` and `%=`, as arithmetic. A remainder rather than a floored modulo, so
  the sign follows the left operand as in the language Decay is shaped after
- [x] Reject a field initializer that reads a field declared below it, or
  itself. It used to compile and fail at runtime with `UnknownPath`, which named
  a path rather than the field that could not have had a value yet. The
  diagnostic names both fields, and the initializer is still analyzed against
  the whole member map so that one mistake produces one diagnostic rather than
  this one plus an unknown name
- [x] Gather proof, partial and worth being exact about: `wisp.decay` now holds
  its floor and player references across frames instead of looking both up by
  name every step, guarded and refreshed through `World.exists`, and guards its
  move with `floor != null && player != null`. What that does **not** show is a
  behaviour difference, because Sindri's surface was shaped around the missing
  short-circuit: `World.exists(null)` answers false and `World.despawn(null)` is
  a no-op precisely so that a guard did not have to protect them. The guard that
  short-circuiting actually rescues — one reaching *through* a reference, as in
  `player != null && Grid.position_x(player, floor) > 0.0` — has no Gather caller
  yet, and inventing one to have a proof would be worse than saying so. The
  runtime tests hold that behaviour instead, by counting a host call that must
  not happen
- [x] Gather proof for `%`: `wisp.decay`'s step timer is now
  `elapsed %= step_seconds` rather than a reset to zero, which had rounded every
  step up to the end of the frame it landed in and tied the wisp's cadence to
  the frame rate. `%` rather than one subtraction, so a frame longer than a step
  leaves no backlog

`while` itself has no Gather caller, and that is deliberate rather than
overlooked: nothing in Gather repeats work today, because each pip, orb, and
wisp is its own entity minding itself — which is the right answer and not a
workaround. Its first real caller arrives with slice 3, where a script can hold
a path and walk it. The runtime tests carry the behaviour until then, including
the budget stopping a loop that never ends.

#### Slice 2 — stop spelling an `f64` as `f32`

No Gather change. This is an honesty fix, and it is cheap in exactly the way the
next paragraph is not.

- [ ] Decide it: either the only numeric type stops being called `f32`, or Decay
  values are actually stored narrowed and `WorldHost` stops being the one place
  the two meet. This is now the next slice — slice 1 is complete
- [ ] Whichever is chosen, `decay/LANGUAGE.md`, `decay/README.md`,
  `docs/scripting.md`'s known gaps, and `docs/decay-direction.md` agree
  afterwards
- [ ] `i32` is deliberately **not** in this slice. A second numeric type in a
  language with no casts, no inference past a binding's own initializer, and
  where `7` and `7.0` are the same value means deciding literal defaulting,
  mixed arithmetic, explicit conversion, and narrowing in both directions at the
  host boundary — and it rewrites every signature in `surface.rs`. Its forcing
  function is `items[index]`, so the decision belongs to slice 3

#### Slice 3 — one collection, because a path already is one

The argument is not that gameplay uses collections. It is that
`Grid.step_toward` computes a whole footprint-aware A\* route in
`crates/sindri-decay/src/host.rs` and then keeps `nodes[1]` and throws the rest
away, once per mover per tick, for the single reason that Decay has no value
that can hold it. The engine computes the path; the language cannot name it.

- [ ] `Array<T>` as a value the host returns. No array literals in this slice
- [ ] Indexing and `len`
- [ ] `for … in`
- [ ] Decide `i32` here — or decide that indices stay the one numeric type, and
  record why
- [ ] `Grid.path(mover, grid, target)` on the Sindri surface, returning the
  route `sindri-grid` already computes, closing Milestone 9's outstanding
  "typed Decay access" for pathfinding
- [ ] Gather proof: the wisp holds a path and follows it, instead of paying for
  a full route every tick to take one step of it

#### Slice 4 — a value for a position

- [ ] `Vec2` and `Vec3` as built-in, non-extensible value types with fixed
  operator support. Not user-defined structs, and explicitly **not** operator
  overloading: the arithmetic belongs to the language and nothing else receives
  it
- [ ] `Grid.position(entity, grid)` replaces the `position_x`/`position_y` pair,
  which `docs/scripting.md` already records as a consequence of the missing
  value rather than as a design choice
- [ ] Sprite tint stays four typed numbers. `docs/scripting.md` argues that
  shape suits a property panel better than a packed value, and a vector type
  existing does not change that argument
- [ ] Gather proof: `orb.decay`'s four `resting_*` fields become two, and
  `player.decay` clamps a position rather than two coordinates

#### Slice 5 — held until something hurts

Each item names the trigger that promotes it. None starts before its trigger
fires, and an item without a trigger is not ready to be planned.

- [ ] `fixed_update` — host-side rather than language: `scripts.rs` names only
  `start` and `update`, and the engine already runs fixed-step simulation.
  Trigger: the first Gather behaviour that drives a physics body
- [ ] Enums and simple structs — trigger: a Gather script encoding a state
  machine in numbers or strings
- [ ] `match` — after enums, and not before
- [ ] Maps — after arrays have proven the collection model, as
  `decay/README.md` already says
- [ ] Modules and multiple files — trigger: the second script that wants a
  function the first one has. "Reusable types across files" is the same item,
  not a second one
- [ ] More maths — `floor`, `ceil`, `round`, `clamp`, `lerp`, `sign`, `tan`,
  `atan2`. Cheap individually, but Decay has no modules, so each is a bare
  global name a script can no longer use for its own. Added on demand, never as
  a batch
- [ ] Randomness as a seeded, host-owned stream, never a language builtin.
  `docs/decay-direction.md` recorded `getrandom` refusing to build for
  `wasm32-unknown-unknown` without an opt-in; the browser target and
  deterministic tests both want the host holding the seed
- [ ] `assert(condition, message)` — trigger: a script where a wrong number is
  harder to find than a wrong path
- [ ] Strings — concatenation, conversion, formatting, interpolation. Trigger:
  **the engine can display text.** Until then a formatted string can only be
  printed, and `print` already takes any type for that reason

#### Explicitly not on this list

Closures, lambdas, function values, generics beyond what a collection needs
internally, traits, classes, inheritance, `async`, coroutines, operator
overloading, exceptions, macros, reflection, and ownership. They are restated
here rather than left to `LANGUAGE.md` because they are the items most likely to
be proposed again, and the answer does not change: they are language-project
work, and Decay's case rests on staying small, typed, and gameplay-shaped. A
sophisticated closure-capture model in an engine that still cannot spawn a
prefab is the trade this section exists to refuse.

Exit gate: Gather is authored in a Decay that has loops, a bounded runtime, one
collection, iteration, and a position value — and `decay/LANGUAGE.md` still
describes the language exactly, with its "What does not exist" section shorter
by precisely what shipped and its test still passing.

### Editing a script somewhere other than the editor

The thing Unity gets right: opening a script in an external editor and finding
it already knows the project — what components exist, what a field is, where a
name is used. That is not magic and it is not an IDE feature. It is two pieces,
and Decay is unusually well placed for both because `decay-semantic` already
computes scopes, types, and spanned diagnostics, and the engine already has a
component schema registry.

- [ ] **Emit a host manifest.** The editor writes a generated, versioned file describing the host surface: every registered component and its schema, every `@export`-able field type, every host function and path. This is Decay's `.csproj` — the thing Unity regenerates when assets change, and the only reason OmniSharp knows anything. `ComponentSchemaRegistry` and `Environment` are the two halves of it, and `Environment` can now enumerate its types and globals for exactly this
- [ ] **Serialize `Environment` to and from that manifest**, so the compiler a tool runs is configured identically to the one the engine runs. A language server that disagrees with the runtime about what exists is worse than none
- [ ] **`decay-lsp`**: diagnostics, hover, go-to-definition, completion, find-references, over `decay-semantic`. Diagnostics come nearly free — they already carry line, column, and span — and completion after a `.` is now possible, because `HostType` knows what is behind one
- [ ] Decide whether references cross files. One file is one compilation unit today, so find-references is per-file; project-wide anything needs multi-file compilation first
- [ ] A syntax definition (TextMate or tree-sitter) — cheap, independent of everything above, and the first thing anyone notices

Typed host members landed first deliberately: a language server whose completion cannot see past a dot is a demo, and building it first would have meant building it twice.

Exit gate: a representative scripted vertical slice runs with equivalent behavior on native and web, receives schema-derived tooling, and hot-reloads behavior while preserving compatible state.

## Explicitly deferred beyond the first major release

- [ ] PBR and advanced shadows
- [ ] Skeletal animation and animation editor
- [ ] WebGL2 fallback
- [ ] Networking/multiplayer
- [ ] Native mobile and consoles
- [ ] Render graphs and general compute API
- [ ] Visual scripting or shader graphs
- [ ] Terrain engine
- [ ] Plugin marketplace
- [ ] Cloud services
- [ ] AI-assisted actions beyond a structured editor command proof
- [ ] Faster component storage: typed queries clone and deserialize each JSON payload, which `docs/entity-scaling.md` measured as the slowest part of reading a world and named as the thing to fix if any of it ever matters
