# Changelog

All notable changes to Sindri Next will be documented here.

## [Unreleased]

### Added

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

### Changed

- Increased the MSRV from Rust 1.85 to 1.87 to use the current `wgpu` 30 release.
- Replaced the planned Tauri/React editor architecture with native `egui`, `egui-winit`, and `egui-wgpu` integration.
- Increased the MSRV from Rust 1.87 to 1.95 for the first `egui` release aligned with `wgpu` 30.
- Replaced the editor's painted viewport composition with the actual Sindri render pipeline while retaining the native UI overlays and controls.
- Added a bounded X11 window-capture lifecycle for reliable full-editor WGPU screenshots in Xvfb CI runs.
- Rewrote the shared demo scene asset in canonical form; the extracted frame and draw order are unchanged.
