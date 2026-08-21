# The legacy 2D inventory

What `vardirhq/sindri-engine` contains, subsystem by subsystem, and what should happen to each: **port**, **refactor**, **replace**, or **defer**. This is the gating item for the rest of Milestone 6 — every port below it depends on a decision recorded here.

Read at `77e0489`, from the code rather than from memory of it. The legacy engine crate is 13,873 lines of Rust across 34 files, plus a 4,505-line Axum server, a 210-line Ollama client, a TypeScript/Tauri editor, and five examples.

## What the survey changed

Four findings that were not obvious from the outside, and that move the plan.

**The desktop coupling is concentrated, not spread.** The fear the milestone's exit gate names — inheriting "desktop/server/Lua coupling" — turns out to be three dependencies and two files rather than a property of the whole codebase. `std::fs` and `SystemTime` appear in `script.rs` and `screenshot.rs`; `Instant` in `render/particles.rs`; nothing else in the 2D simulation or render code reaches for the host. The subsystems this milestone wants are largely portable as written. What is not portable is the *scripting layer*, `rodio`, and `tokio`.

**Lua is the specific blocker, and it is not a matter of taste.** `Cargo.toml` pins `mlua = { version = "0.9", features = ["lua54", "vendored"] }`. Vendored means the C source is compiled, and `wasm32-unknown-unknown` has no libc. The legacy engine could not have reached the browser without dropping Lua, whatever else changed. This independently confirms the conclusion reached in `decay-direction.md` from a spike: whatever Sindri Next scripts with, it cannot be C Lua.

**`examples/scripted_asteroids/scripts/player.lua` is the best specification of the scripting host we have.** It is real gameplay rather than an imagined sample, and the surface it needs is small enough to write down in full:

```text
params.<name>          authored properties with defaults   (the note's @export)
on_start(self)         lifecycle
on_update(self, dt)    lifecycle
self:transform()       component handle
self:sprite()          component handle
self:input()           input handle
transform:position() / :set_position(vec2) / :set_rotation(f32)
sprite:set_tint([f32; 4])
input:is_key_down("A")
vec2(x, y)
print, math.*
```

That list is the acceptance criteria for Sindri Next's scripting host, and it should be treated as such rather than reinvented. It is language-neutral on purpose: per-instance state, lifecycle calls, component handles, and engine functions are what a host has to offer whatever evaluates the script. Decay is what will evaluate it — see `docs/decay-direction.md` — and it already has the first of those four.

**The legacy already converged on two designs Sindri Next has independently.** `ScriptCommandBuffer` routes script writes through a buffer rather than letting scripts touch the world directly — the same reasoning behind `WorldCommand` and `Transaction`. And script hot reload is `SystemTime` polling, which is exactly the approach `AssetWatch` now uses for textures. Neither needs re-deciding.

## Subsystem verdicts

| Subsystem | Legacy | Verdict | Why |
| --- | --- | --- | --- |
| **Sprite animation and sheets** | `render/animation.rs` (160) | **Refactor** | The clip model — frames, durations, looping, `from_grid` — ports nearly as-is. But a legacy `AnimationFrame` holds a `TextureHandle` per frame, which under Sindri Next's registry would mean one GPU texture per frame of every animation. It must become a UV rect into one sheet, which the renderer could not express: sprites had no UV rect at all. **The renderer work came first**, and both are now done — `sindri.sprite_animation`, with the cursor held outside the world so playing a clip does not dirty the scene. |
| **Tilemap** | `render/tilemap.rs` (135) | **Ported** | Small and clean: `Tile`, `set_tile`, `fill_rect`, `tile_to_world`, `world_to_tile`, `tile_uv_rect`. Legacy models it as a render type; Sindri Next needs it as a schema'd component so the editor can author it and the scene can carry it. Same maths, different home. Depended on the same UV rect work as animation, which now exists. Done: `sindri.tilemap` carries the map grid, a palette of sprite names, and a flat array of cells indexing that palette, and extracts into the same sprite batches loose sprites use. It carried a sheet grid of its own for one release; the grid now lives beside the image, where the animation's copy of it also went. `tile_to_world`/`world_to_tile` became `tile_to_local`/`local_to_tile`, on the component, because the map's origin is its entity's transform. It gained a projection the legacy type did not have, because the first thing to use it was an isometric floor. |
| **Camera 2D behaviour** | `camera.rs` (179) | **Port** | `CameraFollow` with dead zone, smoothing, and max speed is a self-contained component with no coupling. |
| **Pixel snapping** | `math.rs:454` | **Port** | Implemented, not merely flagged — the camera's position is snapped to the pixel grid when building the view matrix, and the component's `pixel_perfect` defaults to on. Worth stating because the flag *looks* like the "drawn but does nothing" pattern until you follow it. Orthographic only, as `docs/2d-model.md` already argues. |
| **Text rendering** | `render/text.rs` (95), `fonts.rs` | **Replace** | Ninety-five lines wrapping `glyphon` and `ab_glyph`, with fonts loaded from bytes by hand. The dependency choice is sound and `glyphon` is wgpu-based, so it survives; the integration does not. A font is an asset, and the asset system that should carry it now exists. |
| **Particles** | `render/particles.rs` (451) | **Defer** | The milestone already defers it until the render lifecycle is stable, and the survey agrees: it is the largest 2D subsystem, the least load-bearing for a first game, and the only render file reaching for `Instant`. |
| **A\* pathfinding** | `pathfinding.rs` (685) | **Port** | `PathfindingGrid` and `AStarPathfinder` are renderer-free already, which is what the milestone asks for. `grid.rs` (209) is a general grid with coordinate conversion and neighbour queries and should travel with it. |
| **Platformer navigation** | `pathfinding.rs` (`PlatformGraph`, `PlatformPathfinder`) | **Defer** | Jump and fall edges between platform nodes are a different feature from grid A\*, and no milestone schedules them. Porting them alongside A\* because they share a file would be inheriting scope. |
| **Physics** | `physics.rs` (768), `scene_physics.rs` (392) | **Replace** | The milestone wants an optional Rapier2D adapter with no core dependency and collision layers from the start. Legacy embeds Rapier in the engine crate and has no layers. The behaviour is a reference; the structure is the thing being changed. |
| **Scripting** | `script.rs` (2,214) | **Replace** | Lua cannot reach the browser. Replace the runtime, port the *interface* — see the surface listed above. Decay replaces it; `decay-direction.md` records the decision. |
| **Audio** | `audio.rs` (234) | **Refactor** | `rodio` is a defensible native backend, but the unnumbered Audio track's own first move is a platform boundary with a silent implementation, and the browser needs a different backend behind it. The decoding path belongs with textures in `sindri-assets`, which is where the milestone that just closed put every other asset type. |
| **HUD** | `hud.rs` (382) | **Defer** | Screen-space text and sprites with alignment. Once text rendering and screen-anchored sprites exist, a HUD is composition rather than a subsystem, and it is not clear it needs engine support at all. Revisit when the companion game wants one. |
| **2D lighting** | `render/light.rs` (107) | **Defer** | Point lights are in neither Milestone 6 nor `PROJECT_OVERVIEW.md`'s 2D scope. Not scheduled, so not ported. |
| **Layers, anchors, sprite bounds** | `component.rs`, `render/sprite.rs` | **Refactor** | Sindri Next already has layers and the nine anchors. What is missing is bounds, which the milestone names and which viewport picking will want anyway. Parallax is not ported, for the reason `docs/2d-model.md` gives. |

## What should not travel

- **`pathfinding_v1_backup.rs`** — 685 lines, an exact-size sibling of the live file. Dead code kept in-tree. It should not be read as reference and must not be ported.
- **`screenshot.rs`** — superseded by `examples/cube/src/bin/capture.rs` and `scripts/capture-editor.sh`, both of which verify their output's colours rather than only writing a file.
- **`crates/sindri-server`** — 4,505 lines of Axum routes and an input-map config service. The editor was a Tauri/TypeScript application talking to a local server; Sindri Next's editor is in-process. This is the "server coupling" the exit gate names, and none of it is wanted.
- **`crates/sindri-ai`** — an Ollama client. Out of scope for the engine.
- **`editor/`** — a TypeScript and Tauri application, already replaced by decision in Milestone 7.

## Out of scope for this inventory

The examples (`hello_sindri`, `platformer`, `rendering`, `editor_scene`, `scripted_asteroids`) were read only for what they reveal about the scripting surface. They are useful as behaviour references when each subsystem is ported, and `CLAUDE.md`'s rule about curated examples means none of them should be copied across as-is; the companion game absorbs what they demonstrated.

## Ordering this implies

The two subsystems the milestone lists first — sprite animation and tilemaps — both blocked on the same missing renderer feature: **a sprite could not address part of a texture**. Neither could be ported until sprites carried a UV rect. That was not a roadmap item and became one, ahead of both; `UvRect` and then `sindri.sprite_animation` closed it, and the tilemap port is now unblocked too.

After that the ports are largely independent, and the honest order is by what the companion game needs first.
