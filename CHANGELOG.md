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
- Shared-device editor viewport that renders the real prepared cube-and-sprite runtime frame into an egui texture, with drag rotation, zoom, resize handling, and a full-window CI screenshot artifact.

### Changed

- Increased the MSRV from Rust 1.85 to 1.87 to use the current `wgpu` 30 release.
- Replaced the planned Tauri/React editor architecture with native `egui`, `egui-winit`, and `egui-wgpu` integration.
- Increased the MSRV from Rust 1.87 to 1.95 for the first `egui` release aligned with `wgpu` 30.
- Replaced the editor's painted viewport composition with the actual Sindri render pipeline while retaining the native UI overlays and controls.
- Switched editor screenshot automation to eframe's bounded native capture lifecycle for reliable Xvfb CI runs.
