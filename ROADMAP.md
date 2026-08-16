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
- [ ] Decide versioning policy for Rust crates, scene files, editor protocol, and npm SDK (crates and scene files decided; the editor protocol and npm SDK are deferred until they exist rather than guessed at)
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
- [x] Add web adapter using `wasm-bindgen`, `web-sys`, and async initialization — the `winit` host serves the browser through the same event loop, canvas attachment, and asynchronous device request; the `sindri-web` binding crate for the TypeScript SDK is Milestone 5 rather than a second host
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
- [x] Load textures and scene JSON
- [ ] Add fallback/error assets and actionable diagnostics
- [x] Add asset root and URL resolution rules
- [ ] Add hot reload for native development
- [ ] Define package/export manifest with content hashes
- [ ] Prevent duplicate loads and release unused GPU assets
- [ ] Add image decoding compatibility tests on native and WASM

Exit gate: the same logical texture and scene references load from disk and static web hosting.

## Milestone 5 — First-class TypeScript/Web SDK

### WASM contract

- [ ] Add a narrow `sindri-web` binding crate
- [ ] Define versioned command, event, query, and error payloads
- [ ] Implement command batching and bulk component writes
- [ ] Implement event/query draining without per-entity frame calls
- [ ] Benchmark boundary overhead and publish budgets
- [ ] Make destruction/disposal explicit and leak-tested

### TypeScript package

- [ ] Create `packages/engine` with hand-designed public types
- [ ] Hide generated bindings as package internals
- [ ] Implement `Engine.create({ canvas })`
- [ ] Add start, pause, resume, stop, resize, and destroy semantics
- [ ] Add typed scene/entity handles with stale-handle errors
- [ ] Add keyboard and pointer input
- [ ] Add one safe gameplay update callback per frame
- [ ] Add batched mutation helpers for high-entity workloads
- [ ] Generate API docs and source maps
- [ ] Package ESM JS, TypeScript declarations, and WASM correctly

### Browser quality

- [ ] Add Playwright smoke tests in Chromium
- [ ] Test missing WebGPU messaging
- [ ] Test canvas selector/element validation
- [ ] Test resize, device-pixel ratio, visibility pause, and teardown
- [ ] Test static hosting under a non-root base path

Exit gate: `npm install @sindri/engine` can run a documented sprite-and-cube browser example without Rust code.

## Milestone 6 — Focused 2D migration

An item here is finished when the editor understands it, not when the runtime
does. `PROJECT_OVERVIEW.md` makes that a project rule and Milestone 9 lists its
editor tools accordingly; this milestone listed only runtime halves, so the
authoring surfaces are named explicitly below.

They follow their runtime half rather than lead it. A sheet editor edits a
format, so the format, its serialization, and its renderer come first —
designing a data model through the tool that paints it is how the model ends up
shaped like the tool.

- [ ] Inventory each legacy 2D subsystem as port, refactor, replace, or defer
- [ ] Port sprite animation and sprite sheets
- [ ] Add a sprite sheet authoring surface: slice a sheet into frames, name clips, set timing, preview playback
- [ ] Port camera 2D behavior and pixel snapping
- [ ] Port tilemap data model and renderer
- [ ] Add tilemap authoring: a tile palette, paint and erase, and layer selection
- [ ] Port text rendering with a web-safe font asset strategy
- [ ] Add font and text authoring: choose a font asset and edit text content in the inspector
- [ ] Port particles after the render lifecycle is stable
- [ ] Port layers, parallax, anchors, and sprite bounds
- [ ] Port A* pathfinding into a renderer-free grid crate
- [ ] Add optional Rapier2D adapter without core dependency
- [ ] Add 2D pan/zoom viewport controls, which need a 2D scene rather than the screen-anchored overlay the demo uses
- [ ] Build `hello-2d` and one small platformer vertical slice
- [ ] Verify both examples natively and in browser

Exit gate: Sindri Next matches the useful core of legacy 2D without inheriting its desktop/server/Lua coupling, and each ported system can be authored in the editor rather than only by hand in JSON.

## Milestone 7 — Editor/runtime protocol and minimal editor

- [x] Choose and document a native Rust `egui` editor with in-process WGPU viewport integration
- [x] Establish a deliberately styled, resizable native editor shell
- [x] Load the real demo scene for a read-only hierarchy and transform-inspector proof
- [x] Capture the complete native editor window as a deterministic CI screenshot artifact
- [ ] Define versioned editor protocol and capability handshake
- [x] Load/save the real scene format
- [x] Render the actual Sindri runtime in the viewport through the editor's shared WGPU device
- [x] Display hierarchy from runtime scene state
- [ ] Inspect/edit names, hierarchy, Transform2D, and Transform3D (names and both transforms done; reparenting has a command but no UI)
- [x] Add selection and transform change commands
- [x] Add command-based undo/redo with transaction grouping
- [x] Add 3D orbit, pan, and zoom camera controls, with a reset to the authored camera
- [x] Add play, pause, stop, and reset-to-authored-state
- [ ] Add one sprite, one cube, and one camera editor fixture
- [ ] Add protocol contract and save/reload integration tests

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
- [ ] Add camera/editor gizmos and collider visualization
- [ ] Build `hello-3d` and a small navigable scene
- [ ] Validate representative integrated and discrete GPUs

Exit gate: imported glTF content, camera, lighting, and collision work in native, browser, and editor preview.

## Milestone 9 — Isometric and grid module

- [ ] Extract coordinate/projection lessons from IsoGame without app-specific behavior
- [ ] Add tested orthogonal/isometric grid coordinate types
- [ ] Add world/grid/screen conversion with round-trip property tests
- [ ] Add footprints, occupancy, placement validation, and wall edges
- [ ] Add pathfinding overlays and interaction points
- [ ] Define deterministic sprite-isometric depth keys
- [ ] Add integer zoom and generated-anchor metadata
- [ ] Add orthographic 3D isometric camera helpers
- [ ] Add editor grid, tile, footprint, and height-layer tools
- [ ] Build sprite-based `iso-room` example

Exit gate: shared grid/gameplay logic supports both sprite isometric and orthographic 3D presentation.

## Milestone 10 — Tooling, distribution, and 1.0 hardening

- [ ] Implement minimal `sindri new/dev/build/test/editor` CLI
- [ ] Create `npm create sindri-game`
- [ ] Add native and static-web export pipelines
- [ ] Add curated examples with CI build coverage
- [ ] Add Rust and TypeScript getting-started guides side by side
- [ ] Document supported browsers, GPUs, OSes, and MSRV
- [ ] Add CPU, GPU, memory, startup, and WASM-size benchmarks
- [ ] Add screenshot/render regression suite where stable
- [ ] Audit unsafe code policy and dependencies
- [ ] Stabilize public API and document deprecations
- [ ] Complete license, notices, release notes, and publishing dry run

Exit gate: all first-major-release success criteria in `PROJECT_OVERVIEW.md` pass from clean generated projects.

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
Unity — `PROJECT_OVERVIEW.md` rules that out, and an editor that grows ahead of
the engine would be authoring things nothing can run.

### Authoring the world

- [ ] Create, duplicate, and delete entities from the interface
- [ ] Add and remove components through the inspector, driven by the schema registry rather than a hardcoded list
- [ ] Reparent by dragging in the hierarchy, using the command that already exists
- [ ] Multi-select, and edit what a selection has in common
- [ ] Copy, paste, and duplicate across scenes
- [ ] Decide whether Sindri has prefabs, and if so what a prefab override is

### Working on a project

- [ ] Open a project rather than a single scene, and manage more than one scene at a time
- [ ] Import assets from a watched directory, decoding and registering them without a rebuild
- [ ] Surface project settings, whatever `sindri.toml` turns out to hold
- [ ] Show real engine logs, errors, and asset failures in the console
- [ ] Search and filter that reaches both the hierarchy and the project browser

### Running the game

- [ ] Play mode that runs the real game loop against the edited world, with pause and single step
- [ ] Native preview, and web preview through the actual WASM build
- [ ] Frame timing, draw calls, and entity counts where a developer can see them

### Shape of the tool

- [ ] Named layout presets, and panels that can be rearranged
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

Wanted, in no particular order. Several of these are small enough to do the
moment they annoy someone, and are also named in Milestone 11 as part of the
larger tool; doing one here early is the point rather than a conflict.

- [ ] Read the project browser from a real asset directory; its contents are currently a fixed list
- [ ] Asset thumbnails, after which the grid view is worth defaulting to again
- [ ] Warn before discarding unsaved work on reload, reset, or exit
- [ ] Open a scene from the interface rather than only from the command line
- [ ] Show which entity a hierarchy row is, when its name is empty or repeated

## Future research — first-class Sindri gameplay language

This is a post-foundation research track, not part of the first major release. Do not begin language implementation until representative Rust and TypeScript vertical slices expose concrete gameplay-authoring problems. A Sindri language would complement, not replace, the first-class Rust and TypeScript workflows.

- [ ] Document recurring gameplay-authoring pain points from representative Rust and TypeScript games
- [ ] Define a language-neutral scripting host around versioned commands, events, queries, lifecycle hooks, and component schemas
- [ ] Evaluate a custom language against embedded-language alternatives using explicit product and maintenance criteria
- [ ] Prototype a Rust-implemented portable bytecode interpreter that behaves consistently on native and WebAssembly without a JIT
- [ ] Generate typed component access, diagnostics, and autocomplete from the component schema registry
- [ ] Specify safe entity and asset handles, coroutine cancellation, deterministic scheduling, and execution budgets
- [ ] Prove function/module hot reload with preservation or explicit migration of compatible typed state
- [ ] Define the editor, language-server, formatter, debugger, documentation, and testing commitments before making the language public
- [ ] Decide whether the prototype provides enough gameplay-specific value to justify a permanent language ecosystem

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
