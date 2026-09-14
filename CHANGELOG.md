# Changelog

All notable user-facing changes to Sindri Engine are documented here.

This project has not made its first release yet. Until then, changes are collected
under `Unreleased` and kept intentionally concise. Detailed implementation
history, design rationale, debugging notes, and test archaeology belong in pull
requests, commit history, and subsystem documentation rather than this file.

## [Unreleased]

### Added

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