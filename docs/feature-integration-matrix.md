# Feature integration matrix

Sindri features cross four product surfaces. A runtime type alone is not an
authorable engine feature, an editor control is not useful when a game cannot
run what it writes, and a Decay binding is not proven until gameplay uses it.
This table keeps those counterparts visible in one place.

Update this file in the same change that moves any cell. `docs/capabilities.md`
remains the detailed evidence for what works; this is the cross-surface index
that makes missing integrations difficult to overlook.

Status terms are deliberately plain:

- **Ready** — implemented and exercised on that surface.
- **Partial** — a useful slice works, but the named gap still matters.
- **Missing** — planned or required, but unavailable.
- **N/A** — the surface does not need a counterpart for the feature.

| Feature | Engine/runtime | Editor authoring | Decay | Gather proof | Next integration gap |
| --- | --- | --- | --- | --- | --- |
| Entities and hierarchy | **Ready** — safe handles, parents, recursive destruction, commands | **Ready** — create/delete, folding, drag reparent | **Partial** — find, inspect, reference, despawn; no spawn | **Ready** — scripts find and despawn orb entities | Define what scripts spawn; currently blocked on the prefab decision |
| Transform | **Ready** — shared 2D/3D transform and checked Z lock | **Partial** — position/scale/Z lock; no rotation editor or gizmo | **Partial** — typed position/scale paths; no structured vector value | **Ready** — movement is scripted | Rotation authoring, then structured vector access |
| Camera | **Ready** — perspective and orthographic, pixel snapping | **Partial** — view controls and authored Game view; no gizmo | **Missing** — scripts cannot reach cameras | **Ready** — authored camera renders the game | Decide the gameplay camera host surface |
| Sprites and sheets | **Ready** — world/screen sprites, sheets, UVs, layers, blending | **Ready** — components, slicer, texture selection, picking | **Partial** — typed sprite asset and visibility paths only | **Ready** — floor props, player, orbs, lamps, banner | Generate the wider component surface from schemas |
| Sprite animation | **Ready** — clips, timing, loop state | **Ready** — clip/frame authoring and preview | **Missing** — cannot select or control clips | **Ready** — player animation advances in play | Expose typed play/stop/clip paths to Decay |
| Tilemaps | **Ready** — orthogonal/isometric rendering and cell lookup | **Ready** — resize, palette, paint/erase | **Missing** — no tile queries or writes | **Ready** — floor is one tilemap | Define typed read/write access without exposing storage details |
| Text | **Partial** — screen text and project fonts; no world text or rich layout | **Ready** for current component | **Missing** — scripts cannot change content | **Partial** — static title; score and win state remain sprites | Add dynamic text host access |
| Assets | **Ready** — IDs, manifests, async loading, texture/font/script reload | **Partial** — browse/select/slice; cannot create, delete, or import watched files | **Partial** — string-like asset fields, no safe asset value | **Ready** — project-owned assets on native and web | Safe typed asset handles and project import workflow |
| Input | **Ready** — keyboard and pointer state behind platform boundary | **Partial** — play owns keyboard; no authoring-input contract for Game view | **Partial** — keyboard axes/edges; no pointer | **Ready** — arrow-key movement | Pointer host surface and Game-view routing |
| Decay scripts and exports | **Ready** — typed host, compile/run/reload, safe entity references | **Partial** — component and `@export` fields; source cannot be created or edited | **Partial** — useful gameplay subset; no loops, collections, LSP, formatter, or debugger | **Ready** — all gameplay rules are Decay | Host manifest, typed schema access, and source tooling |
| Play mode | **Partial** — scripts and animation step; full game-system scheduling does not | **Partial** — play/pause/stop and snapshot restore; no single-step | **Ready** for current script lifecycle | **Ready** | Complete system stepping and define deterministic ordering |
| Grid and isometric gameplay | **Partial** — typed geometry, footprints, bounded occupancy, placement validation, and world/screen Y orientation; tilemaps share projection for rendering and picking | **Partial** — tile painting uses the shared map adapter; no footprint, occupancy, wall, or height tools | **Ready** — typed continuous position/read/place API through an explicit tilemap entity | **Ready** — movement, bounds, and collection use the floor's logical isometric coordinates | Add wall edges, then adapt footprints and occupancy into engine authoring |
| Pathfinding and occupancy | **Partial** — renderer-free footprints and atomic bounded occupancy exist; no walls or A* | **Missing** | **Missing** | **Missing** | Add wall edges, engine adapters, then renderer-free A* and script access |
| 3D content | **Partial** — cube primitive, depth, cameras, textured mesh foundation | **Partial** — preview/picking; no import, material authoring, light, or gizmo | **Missing** | **Missing** | glTF/material/light path, then one 3D prop in Gather |
| Audio | **Missing** | **Missing** | **Missing** | **Missing** | Platform boundary, asset type, one-shot playback, browser backend |
| Physics and collision | **Missing** | **Missing** | **Missing** | **Missing** | Optional Rapier adapters with collision layers kept separate from render layers |
| Projects and export | **Partial** — browser build exists, but no product export pipeline | **Missing** — single-scene workflow, no project settings/build controls | **N/A** | **Partial** — hand-built Pages host | Project model, CLI, native/web export pipeline |

## How a new feature moves through the table

A feature may land in stages, but each stage must say what remains. The usual
order is runtime model and tests, editor authoring, Decay access where gameplay
needs it, and a Gather use that applies real pressure. That is an order of
dependencies, not permission to forget later columns: the row remains partial
until the relevant counterparts exist.

Some features legitimately have no Decay surface. Serialization internals and
editor layout are examples. Mark those **N/A** rather than **Ready**, because a
green cell must mean something was implemented and exercised.
