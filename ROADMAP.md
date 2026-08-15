# Sindri Next roadmap

This is the checkable engineering plan. Check an item only when its acceptance criteria and relevant tests are complete. The roadmap is ordered by dependency, not excitement.

## Milestone 0 — Architecture and repository baseline

- [x] Review the vision against the legacy `sindri-engine` implementation
- [x] Record feasibility constraints and major risks
- [x] Create a Rust 2024 workspace with an explicit MSRV (Rust 1.95 for `wgpu` 30 and `egui` 0.36)
- [x] Add formatting, Clippy, test CI, and dependency caching
- [x] Establish `sindri-core` and the public `sindri` facade
- [ ] Add `CONTRIBUTING.md`, code of conduct, and dual-license files
- [ ] Add dependency policy and automated dependency/security review
- [ ] Decide versioning policy for Rust crates, scene files, editor protocol, and npm SDK
- [ ] Add release/changelog validation

Exit gate: a clean clone passes format, lint, and test checks on the declared MSRV.

## Milestone 1 — Renderer-independent engine core

### Lifecycle and time

- [x] Define and test legal engine lifecycle transitions
- [x] Implement capped frame delta and fixed-step accumulation
- [x] Prevent a fixed-update spiral of death
- [x] Define pause semantics
- [ ] Add time scale without accumulating floating-point drift
- [ ] Define lifecycle hooks and error propagation used by native and web hosts
- [x] Add a platform-independent `EngineCore` advanced by runtime hosts
- [ ] Add deterministic clock/test host

### Entities and world

- [x] Add generation-checked runtime entity handles
- [x] Add safe spawn, access, recursive destruction, and slot reuse
- [x] Add hierarchy parenting with cycle prevention
- [x] Keep serialized logical IDs separate from runtime handles
- [x] Define typed component registration and metadata
- [x] Add query API for built-in and registered component types
- [ ] Add deferred world command buffer
- [ ] Specify deterministic system ordering
- [ ] Benchmark 1k, 10k, and 100k entity workloads before considering an archetype ECS

### Scenes and serialization

- [x] Add versioned scene document and stable logical entity IDs
- [x] Validate duplicate IDs, missing parents, and hierarchy cycles
- [x] Load a validated scene into a runtime world in two passes
- [x] Preserve forward-compatible JSON component payloads
- [x] Define component schema registry and unknown-component behavior
- [ ] Save a runtime world back to a scene without losing stable IDs
- [ ] Add canonical pretty serialization for reviewable diffs
- [ ] Add fixtures and golden round-trip tests
- [ ] Define scene migration API before format version 2 exists
- [ ] Split editor-only metadata into a namespaced, ignorable section

Exit gate: core runs without GPU/window/browser dependencies and scene fixtures round-trip deterministically.

## Milestone 2 — Platform and GPU bootstrap

### Boundaries

- [ ] Add `sindri-platform` traits for lifecycle, input source, clock, and asset I/O
- [ ] Add desktop adapter using `winit`
- [ ] Add web adapter using `wasm-bindgen`, `web-sys`, and async initialization
- [ ] Keep target-specific conditionals inside platform hosts
- [ ] Produce explicit capability errors for unavailable WebGPU/surface features

### GPU foundation

- [x] Add `sindri-gpu` adapter/device/queue ownership
- [x] Negotiate conservative cross-target limits and features
- [x] Add shared surface configuration and non-zero resize policy
- [ ] Centralize surface loss, outdated, occlusion, and timeout recovery after the proof example
- [x] Add resource labels and actionable adapter/device/surface errors
- [ ] Define typed buffer, texture, sampler, shader, and pipeline wrappers
- [ ] Add render-target and depth-target management
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
- [ ] Add asset root and URL resolution rules
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

- [ ] Inventory each legacy 2D subsystem as port, refactor, replace, or defer
- [ ] Port sprite animation and sprite sheets
- [ ] Port camera 2D behavior and pixel snapping
- [ ] Port tilemap data model and renderer
- [ ] Port text rendering with a web-safe font asset strategy
- [ ] Port particles after the render lifecycle is stable
- [ ] Port layers, parallax, anchors, and sprite bounds
- [ ] Port A* pathfinding into a renderer-free grid crate
- [ ] Add optional Rapier2D adapter without core dependency
- [ ] Build `hello-2d` and one small platformer vertical slice
- [ ] Verify both examples natively and in browser

Exit gate: Sindri Next matches the useful core of legacy 2D without inheriting its desktop/server/Lua coupling.

## Milestone 7 — Editor/runtime protocol and minimal editor

- [x] Choose and document a native Rust `egui` editor with in-process WGPU viewport integration
- [x] Establish a deliberately styled, resizable native editor shell
- [x] Load the real demo scene for a read-only hierarchy and transform-inspector proof
- [x] Capture the complete native editor window as a deterministic CI screenshot artifact
- [ ] Define versioned editor protocol and capability handshake
- [ ] Load/save the real scene format
- [x] Render the actual Sindri runtime in the viewport through the editor's shared WGPU device
- [ ] Display hierarchy from runtime scene state
- [ ] Inspect/edit names, hierarchy, Transform2D, and Transform3D
- [ ] Add selection and transform change commands
- [ ] Add command-based undo/redo with transaction grouping
- [ ] Add 2D pan/zoom and basic 3D orbit camera controls
- [ ] Add play, pause, stop, and reset-to-authored-state
- [ ] Add one sprite, one cube, and one camera editor fixture
- [ ] Add protocol contract and save/reload integration tests

Exit gate: editing a transform, saving, and reopening produces the same visible native/web scene.

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
