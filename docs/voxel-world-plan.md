# Engine-owned voxel world plan

Status: Phase 1 complete; Phase 2 in progress

Causeway proved that Sindri can generate, stream, pick, edit, and render a large voxel-like world. It also proved the current ownership boundary is wrong: generation and residency live in the game, terrain is materialized as a `sindri.tile_volume`, renderer revisions can rebake too much work, and the experimental smooth patch has no persistent chunk mesh cache.

The next milestone is an engine-owned voxel subsystem. Minecraft is the architectural reference for the boring, battle-tested parts: chunked world data, vertical sections, palette-friendly storage, neighbour-aware meshing, dirty rebuilds, bounded residency, separate simulation and render concerns, and cached compiled geometry. Sindri deliberately differs by making the visual mesher pluggable rather than making block faces the only presentation.

## Invariants

1. Voxel data describes the world. Render meshes are derived caches and may be discarded.
2. A game supplies generation policy; the engine owns coordinates, chunks, sections, residency, dirty tracking, neighbour sampling, meshing contracts, and cache lifetime.
3. Camera movement must not rebuild unchanged resident geometry.
4. Editing one voxel dirties only its section and any section sharing the edited boundary.
5. A mesher can inspect a one-cell neighbour halo without requiring that neighbour to be rendered.
6. Old compiled geometry remains usable until replacement geometry is ready.
7. Render distance and simulation distance are separate concepts even if Causeway initially configures them to the same value.
8. Block, smooth, hybrid, and future custom meshers consume the same voxel-world interface.
9. Empty/repetitive sections must be cheap. Palette compression is part of the storage design, not an afterthought.
10. Native and wasm builds share deterministic generation and meshing semantics.

## Target architecture

```text
Game generator / edits
        |
        v
sindri-voxel
  World -> Chunk column -> 16x16x16 Section
    |          |                |
    |          |                +-- palette + voxel indices
    |          +-- residency / revisions
    +-- deterministic neighbour sampling
        |
        +--> BlockMesher  -> compiled section mesh
        +--> SmoothMesher -> compiled terrain mesh
        +--> HybridMesher -> game-selected combination
        |
        v
scene/render bridge -> persistent GPU mesh cache
```

`sindri-voxel` must not depend on Causeway, scene JSON, wgpu, editor UI, or Decay. It owns world data and geometry policy. Rendering consumes compiled output through a narrower bridge.

## Phases

### 1. Foundation
- Add `crates/sindri-voxel`.
- Define signed 3D voxel coordinates, 16-wide chunk columns, 16³ section coordinates, local coordinates, and stable conversion across negative coordinates.
- Add `VoxelId`, air, palette-backed `VoxelSection`, revision tracking, and empty/uniform fast paths.
- Define deterministic random-access source/generator contracts.
- Test boundaries, negative coordinates, palette growth, deterministic access, and revisions.

Exit: an engine crate represents and generates sparse voxel sections without a Causeway type.

### 2. Engine residency
- Add a bounded resident section/chunk store.
- Separate render radius from simulation radius.
- Add entering/staying/leaving diffs instead of replacing an entire volume.
- Preserve edited sections independently of residency.
- Add generation queue contracts. Begin synchronously; the API must permit workers later.

Exit: moving a focus point changes only the residency delta.

### 3. Minecraft-style block mesher
- Generate only exposed faces.
- Sample a one-voxel halo across section/chunk boundaries.
- Split opaque, cutout, and transparent output.
- Carry material/face identity and UVs without depending on one atlas.
- Dirty boundary neighbours when an edit changes visibility.
- Keep meshing independent of GPU upload.

Exit: a solid 16³ section emits only its exterior; adjacent solid sections emit no internal boundary faces.

### 4. Persistent render cache
- Bridge compiled voxel meshes into scene/render.
- Cache GPU buffers by section coordinate + revision + mesher profile.
- Reuse unchanged buffers across frames and camera movement.
- Keep old buffers until a replacement is uploaded.
- Frustum-cull cached section bounds.
- Instrument resident sections, queued generation/mesh work, triangles, uploads, and rebuilds/frame.

Exit: panning without edits produces zero terrain remeshes after residency settles.

### 5. Move Causeway onto the engine
- Move generic chunk/streaming concepts out of `game/src/streaming.rs`.
- Adapt Causeway's biome/noise policy to engine generator traits.
- Preserve the seed and recognizable opening landscape.
- Remove `MAX_SKIRT` as a rendering workaround.
- Remove the one-centre-chunk `sindri.mesh` prototype.
- Keep construction, picking, navigation, and player edits working.
- Add a phone-sized regression/capture that pans across chunk boundaries.

Exit: Causeway uses `sindri-voxel` for generation/residency and has no holes caused by skirts or centre-chunk mesh replacement.

### 6. Persistence and digging
- Store edits as sparse overrides/deltas from deterministic generation.
- Generate a real vertical volume rather than a surface skirt.
- Support caves, tunnels, overhangs, ores, and multiple surfaces at one X/Z.
- Save/load edited chunks without serializing untouched procedural terrain.

Exit: dig through a mountain, unload it, reload it, and the tunnel remains.

### 7. Smooth and hybrid terrain
- Introduce a scalar/density sampling contract instead of smoothing block-top heights.
- Evaluate Surface Nets and Dual Contouring against editing and seam requirements.
- Hybrid mode smooths natural terrain while constructions/selected materials remain block-meshed.
- Collision initially remains voxel-derived.

Exit: one authoritative world can switch Block/Hybrid presentation without changing voxel data.

### 8. Minecraft-grade follow-ons
Only after the foundation is measured:
- skylight and emissive/block-light propagation
- biome/material registries and richer per-face material rules
- water/fluid surfaces
- asynchronous worker scheduling and prioritization
- distant terrain LOD
- region-like persistence if real saves justify it
- networking only when a game requires it

## Causeway acceptance scenario

Start in Causeway, cross many chunk boundaries, dig through a mountain, build a tower, leave render distance, return, and find the world unchanged. Ordinary camera motion does not remesh unchanged sections. On mobile, residency and GPU memory remain bounded.

## Migration rule

Do not rewrite Causeway in one jump. Each phase leaves main usable. New engine APIs are proven with engine tests first, then Causeway adopts them. Temporary compatibility code is acceptable only when its deletion phase is named here.

## Implementation progress

- **Phase 1:** complete on main. `sindri-voxel` owns coordinates, 16³ palette-backed sections, revisions, and deterministic source sampling.
- **Phase 2:** in progress. The engine now has bounded 3D section residency, separate render/simulation radii, entering/staying/leaving diffs, sparse edit retention across unload/reload, and boundary-aware dirty tracking.
- **Next:** finish the Phase 2 queue/scheduling contract, then implement the neighbour-aware block mesher before Causeway migration.

Causeway intentionally remains on the old path until the engine can both own residency and compile correct section geometry. Moving the game sooner would merely relocate the current rendering problems.
