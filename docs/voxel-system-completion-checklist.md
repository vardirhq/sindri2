# Voxel system completion checklist

This is the short execution checklist for turning the current voxel foundation
into a production subsystem. The architecture and phase history remain in
[`voxel-world-plan.md`](voxel-world-plan.md); this document records what is
still missing and the order in which it should be closed.

Causeway is the acceptance game. Its current `sindri.tile_volume` blocks already
have deterministic visual variants, buried-block appearances, directional face
shading, neighbour-aware corner darkening, picking, building, and navigation.
Moving Causeway to `sindri-voxel` must preserve or improve those qualities. A
faster world that looks visibly worse is not a successful migration.

## Current foundation

- [x] Signed voxel coordinates and negative-safe 16³ sections.
- [x] Palette-backed section storage, revisions, and deterministic generation.
- [x] Bounded 3D residency with separate render and simulation radii.
- [x] Sparse edits that survive unload/reload in memory.
- [x] Boundary-aware dirty tracking and deduplicated work queues.
- [x] Neighbour-aware block meshing with opaque, cutout, and transparent CPU
  output.
- [x] Persistent compiled/GPU section identities and stale-result rejection.
- [x] Conservative frustum culling of transformed cached section bounds.
- [x] An authored `sindri.voxel_world` proof in Voxel Lab and the editor.

## 1. Finish block presentation

This comes before Causeway migration. The block mesher is the correctness and
performance baseline for every later presentation profile.

- [ ] Make the editor scene and Pages Voxel Lab use the same material bindings
  and rendering configuration. The browser proof must not maintain a parallel
  hand-written Causeway-atlas mapping.
- [ ] Complete cutout and transparent section rendering, including stable cache
  identities, depth/blend policy, and correct release on eviction.
- [ ] Expand voxel materials beyond one fixed top/side/bottom lookup:
  deterministic variants, covered/buried appearances, render class, occlusion
  policy, and optional face-specific rules.
- [ ] Carry the data required for renderer-owned directional face lighting and
  neighbour-aware corner ambient occlusion through compiled geometry. Do not
  bake a fixed camera or world-light direction into generic voxel textures.
- [ ] Define atlas-safe sampling: padded regions, filtering/mipmap policy, and
  close-range tests that catch seams or neighbouring-tile bleed.
- [ ] Use seamless, evenly lit base textures where terrain should read as one
  continuous material; keep deliberate per-block borders only as a selectable
  art direction.
- [ ] Add visual regressions from several camera rotations and a very close
  zoom. They must catch flipped faces, missing triangles, repeated lighting
  bands, atlas bleed, and accidental transparency.

Exit: Voxel Lab remains solid and coherent from every camera angle, and its
materials can reproduce the useful parts of Causeway's current block look.

## 2. Complete the persistent runtime path

- [ ] Drive scene residency from the active world camera or an explicit runtime
  focus provider rather than a permanently authored section coordinate.
- [ ] Drain generation and meshing queues through bounded asynchronous workers
  with priorities, cancellation, and per-frame upload budgets.
- [ ] Keep old geometry visible until a replacement has reached the GPU.
- [ ] Bound CPU mesh memory, GPU cache memory, pending work, and resident section
  counts; expose all four in Voxel Lab diagnostics.
- [ ] Prove that settled camera motion produces zero remeshes, entering sections
  alone generate/upload, dirty sections alone replace, and leaving sections
  release their cache.
- [ ] Render section coordinates relative to a moving origin before large world
  coordinates lose useful `f32` precision.

Exit: crossing section boundaries has bounded work and no visible holes or
frame-time cliff on a phone-sized browser viewport.

## 3. Editing and authoring

- [ ] Add editor picking, paint, erase, fill, and material selection for
  `sindri.voxel_world`, routed through undoable editor commands.
- [ ] Add runtime/Decay APIs for voxel queries and edits without exposing
  renderer or storage internals.
- [ ] Define reusable voxel assets/prefabs for structures such as trees,
  buildings, props, and block-built items, with simple placement in a world.
- [ ] Give voxel material and generation profiles inspectable project assets
  rather than burying a full world definition inside one scene component.
- [ ] Derive collision and multi-level navigation only for entering or dirty
  sections.

Exit: an author can create, place, inspect, edit, undo, and script voxel content
without hand-editing scene JSON.

## 4. Migrate Causeway without losing its strengths

- [ ] Adapt Causeway's seed, biome, strata, decoration, and structure policy to
  engine generator interfaces while preserving a recognizable opening world.
  The engine's `natural_terrain` generator now covers Causeway's landform,
  climate, frayed-threshold and tree policy and goes further (warped
  coastlines, rivers, blended biome relief and terraces, steep-slope rock,
  caves, overhangs, ice); see [`voxel-terrain.md`](voxel-terrain.md). Causeway
  itself has not moved onto it.
- [ ] Move generic streaming/residency code out of `game/`.
- [ ] Translate Causeway tile semantics—variants, buried looks, shapes,
  occlusion, face treatment, and material identity—rather than merely pointing
  engine voxels at the old atlas.
- [ ] Preserve block placement/removal, picking, Build/Play cameras, Wanderer
  movement, navigation, and edits across unload/reload.
- [ ] Remove `MAX_SKIRT`, the thin surface-shell workaround, and the
  one-centre-chunk smooth overlay.
- [ ] Add desktop and phone captures that cross section boundaries, rotate the
  camera, dig, build, unload, and return.

Exit: Causeway owns no generic voxel infrastructure and looks at least as good
as its current tile-volume path while doing less rebuild work.

## 5. World depth and persistence

- [ ] Generate true vertical terrain with strata, caves, tunnels, overhangs,
  ores/resources, structures, and multiple surfaces at one X/Z coordinate.
  Caves, tunnels, overhangs and multiple surfaces exist in `natural_terrain`;
  strata, ores and structures remain.
- [ ] Save sparse edits independently from deterministic base generation, with
  versioning and migration for material/generator changes.
- [ ] Stream saved edits and generated sections without serializing untouched
  terrain.
- [ ] Add biome and material registries plus water/fluid surface rules.
  Biomes are authored per world in `natural_terrain` and water fills to sea
  level, freezing where cold; shared registries and flowing water remain.
- [ ] Test digging through a mountain, unloading it, returning, and finding the
  tunnel and construction unchanged.

## 6. Alternative meshers

- [ ] Define the scalar/density sampling contract needed by editable smooth
  terrain; do not infer smooth terrain from averaged column heights.
- [ ] Evaluate Surface Nets and Dual Contouring for caves, overhangs, edits,
  normals, material boundaries, and seam-free section borders.
- [ ] Add `Smooth` and `Hybrid` profiles behind the same work queue and cache
  lifecycle as `Block`.
- [ ] Keep authored construction block-exact in hybrid mode while natural
  density terrain can be smooth.
- [ ] Add distant-terrain LOD only after correctness and cache behaviour are
  measured.

## Final acceptance scenario

Dig into a mountain, tunnel underneath it, emerge on the other side, build a
tower, travel far enough for the area to unload, then return and find both the
tunnel and tower unchanged. During the trip, unchanged sections never remesh,
memory remains bounded, mobile frame time remains usable, and the world can be
shown through block, smooth, or hybrid presentation without changing its
authoritative gameplay data.
