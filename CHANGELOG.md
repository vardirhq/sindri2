# Changelog

All notable user-facing changes to Sindri Engine are documented here.

This project has not made its first release yet. Until then, changes are collected
under `Unreleased` and kept intentionally concise. Detailed implementation
history, design rationale, debugging notes, and test archaeology belong in pull
requests, commit history, and subsystem documentation rather than this file.

## [Unreleased]

### Fixed

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
