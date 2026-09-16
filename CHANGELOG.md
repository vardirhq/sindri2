# Changelog

All notable user-facing changes to Sindri Engine are documented here.

This project has not made its first release yet. Until then, changes are collected
under `Unreleased` and kept intentionally concise. Detailed implementation
history, design rationale, debugging notes, and test archaeology belong in pull
requests, commit history, and subsystem documentation rather than this file.

## [Unreleased]

### Added

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
