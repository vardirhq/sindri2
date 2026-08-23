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
| Transform | **Ready** — shared 2D/3D transform and checked Z lock | **Ready** — position/rotation/scale inspector plus command-backed Scene gizmos, snapping, and Z-lock-safe movement | **Partial** — typed position/scale paths; no structured vector value | **Ready** — movement is scripted | Structured vector and rotation access in Decay |
| Camera | **Ready** — perspective and orthographic, pixel snapping | **Partial** — view controls and authored Game view; no gizmo | **Missing** — scripts cannot reach cameras | **Ready** — authored camera renders the game | Decide the gameplay camera host surface |
| Sprites and sheets | **Ready** — world/screen sprites, sheets, UVs, layers, blending | **Ready** — components, slicer, texture selection, picking | **Partial** — typed sprite asset and visibility paths only | **Ready** — floor props, player, orbs, lamps, banner | Generate the wider component surface from schemas |
| Sprite animation | **Ready** — clips, timing, loop state | **Ready** — clip/frame authoring and preview | **Missing** — cannot select or control clips | **Ready** — player animation advances in play | Expose typed play/stop/clip paths to Decay |
| Tilemaps | **Ready** — orthogonal/isometric rendering and cell lookup | **Ready** — resize, palette, paint/erase | **Missing** — no tile queries or writes | **Ready** — floor is one tilemap | Define typed read/write access without exposing storage details |
| Text | **Partial** — screen text and project fonts; no world text or rich layout | **Ready** for current component | **Missing** — scripts cannot change content | **Partial** — static title; score and win state remain sprites | Add dynamic text host access |
| Assets | **Ready** — IDs, manifests, async loading, texture/font/script reload | **Partial** — browse/select/slice; cannot create, delete, or import watched files | **Partial** — string-like asset fields, no safe asset value | **Ready** — project-owned assets on native and web | Safe typed asset handles and project import workflow |
| Input | **Ready** — keyboard and pointer state behind platform boundary | **Partial** — play owns keyboard; no authoring-input contract for Game view | **Partial** — keyboard axes/edges; no pointer | **Ready** — arrow-key movement | Pointer host surface and Game-view routing |
| Decay scripts and exports | **Ready** — typed host, compile/run/reload, safe entity references | **Partial** — component and `@export` fields; source cannot be created or edited | **Partial** — useful gameplay subset; no loops, collections, LSP, formatter, or debugger | **Ready** — all gameplay rules are Decay | Host manifest, typed schema access, and source tooling |
| Play mode | **Partial** — scripts and animation step; full game-system scheduling does not | **Partial** — play/pause/stop and snapshot restore; no single-step | **Ready** for current script lifecycle | **Ready** | Complete system stepping and define deterministic ordering |
| Grid and isometric gameplay | **Partial** — typed geometry, authored footprints/walls, derived world occupancy, pathfinding, and world/screen Y orientation; tilemaps remain the shared projection authority | **Partial** — tile painting plus dedicated wall and footprint/occupant inspector authoring; no viewport wall painting or height tools | **Ready** — typed continuous position/read/place plus pathfinding calls through an explicit tilemap entity | **Ready** — movement, bounds, collection, and Wisp pathfinding use the floor's logical isometric coordinates | Richer viewport navigation painting and height authoring |
| Pathfinding and occupancy | **Ready** — deterministic walls, A*, whole-footprint paths, authored components, and a validated world adapter | **Ready** — tilemap walls and occupant grid/footprint cells have dedicated inspector authoring | **Ready** — typed `can_reach` and `step_toward` calls delegate to the shared world adapter | **Ready** — the Wisp follows the player around authored walls through Decay | Per-path policies, costs, and richer viewport wall painting |
| 3D content | **Partial** — cube primitive, depth, cameras, textured mesh foundation | **Partial** — preview, picking, and transform gizmos; no import, material authoring, or light | **Missing** | **Missing** | glTF/material/light path, then one 3D prop in Gather |
| Audio | **Ready** — WAV/Ogg/MP3 assets, silent/native/browser backends, scene source and playback lifecycle | **Partial** — the project browser lists audio files and `sindri.audio` is editable through the generic component inspector; no clip preview, and nothing gathers the audio a scene names | **Ready** — typed play/loop/pause/resume/stop intent without device I/O | **Ready** — background loop plus pickup and victory sounds on the shared host path | A `referenced_audio` gather so hosts load what a scene names, editor clip preview, per-voice/script handles, and buses/spatial audio |
| 2D physics and collision | **Ready** — masked Rapier2D runtime, body kinds, fixed-step stepping, collision masks, sensors/events, validation, velocity and impulse operations | **Missing** | **Missing** | **Missing** | Register scene components and add editor authoring/gizmos, then typed Decay access and Gather use |
| 3D physics and collision | **Missing** — Sindri-owned 3D body/collider data model exists, but there is no 3D runtime or exercised behavior yet | **Missing** | **Missing** | **Missing** | Implement and exercise the parallel 3D runtime only after the 2D feature track is proven end to end |
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
