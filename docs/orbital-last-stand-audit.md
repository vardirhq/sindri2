# Orbital Last Stand vertical-slice audit

This document asks a deliberately concrete question:

> Could [Orbital Last Stand](https://github.com/MadsenDev/tester-repo), as it exists today, be created in Sindri Next, as Sindri exists today?

It is a capability audit, not a commitment to port the game and not an argument
that Sindri should become a survivor-like-specific engine. Orbital Last Stand is
useful because it stresses a different part of a 2D engine than Gather: high
entity churn, procedural combat patterns, touch input, interactive screen UI,
persistent progression, and a complete static-web release.

This audit was written against Sindri `main` at commit
`b381cb1a3801511c4fa13e887526823cdf7a5bfa`, and its statuses are updated as the
gaps close rather than left as a permanent verdict. The work closing them is
planned in `docs/orbital-last-stand-plan.md`; `docs/capabilities.md` remains the
source of truth for what Sindri demonstrably does.

## Executive finding

**Not faithfully through the intended editor + Decay workflow today.**

A recognizable combat prototype could be written now by putting substantial
game-specific logic in Rust and using Sindri for its world, renderer, input,
audio, and browser host. That would prove those foundations, but it would not
prove that a user can make this game *in Sindri*. The complete current game
would require several engine and authoring capabilities to be implemented as
part of the port.

The decisive gaps were script-side spawning, reusable spawn definitions,
collection/query access, dynamic interactive UI, pointer/touch access from
Decay, gameplay collision access, persistence, particles or equivalent effect
rendering, and a product export pipeline.

**Spawning, reusable definitions, collections, queries, pointer/touch input and
gameplay collision are now done.** `docs/prefabs.md`, `docs/scripting.md`,
`docs/physics.md` and `decay/LANGUAGE.md` are the contracts; the rest of the
list stands.

## What the reference game exercises

Orbital Last Stand is a ten-minute browser action game with:

- continuous player movement from mouse, touch, or keyboard
- hundreds of simultaneously active enemies, projectiles, pickups, and effects
- thousands of spawn/despawn operations in one run
- procedural enemy waves, aimed volleys, radial patterns, rails, blast zones,
  boss phases, and difficulty-dependent behavior
- distance-based collision, damage, invulnerability, piercing, orbitals, and
  area effects
- random module offers and data-driven build combinations
- interactive title, settings, hangar, archive, upgrade, route, contract,
  pause, result, and debrief screens
- runtime-changing HUD text, bars, warnings, and boss information
- synthesized sound and lifecycle-aware audio
- persistent settings, statistics, unlocks, discoveries, currency, and run
  history stored on the device
- static hosting on GitHub Pages with a mobile-first layout

The game currently draws many objects procedurally on a canvas. A Sindri
version does not need to copy that implementation, but it must reproduce the
observable behavior and authoring workflow.

## Capability matrix

The statuses mean:

- **Ready** — the relevant capability is exercised end to end.
- **Partial** — useful foundation exists, but the reference workload cannot be
  authored through the intended surface.
- **Missing** — the required game-facing capability does not exist.
- **Rust escape hatch** — possible only by writing game-specific native engine
  code outside the desired editor + Decay workflow.

| Reference requirement | Sindri today | Assessment |
|---|---|---|
| Fixed-step update and lifecycle | Fixed-step simulation, capped frame time, pause/resume, native/browser hosts | **Ready** |
| 2D world rendering and layering | Textured/tinted/layered sprites, animation, cameras, screen images and text | **Ready** for sprite-based art |
| Procedural vector shapes and glow effects | No general shape/primitive or particle authoring system | **Partial**; bake sprites or add a render feature |
| High-volume entity lifetime | Core world supports safe spawn/despawn and has measured storage scaling; scripts now spawn within a bounded per-pass budget | **Partial**; the churn itself is not yet measured from Decay |
| Spawn enemies, bullets, pickups, and hazards from Decay | `World.spawn` takes a typed prefab reference and answers with a generation-checked entity | **Ready** |
| Reusable enemy/projectile definitions | Prefabs: versioned single-root scene fragments, validated and canonical; no editor authoring yet | **Partial**; the format and spawn path work, making one still means writing the file |
| Query groups of enemies/projectiles | `Array<T>` is a Decay value, and `World.with_tag` answers with a bounded, ordered, active-only group | **Ready** |
| Timed procedural behavior | Stateful update scripts, bounded loops, spawning, and collections exist | **Partial**; still blocked by randomness |
| Seeded random waves and module offers | Host-owned seeded randomness is planned, not implemented | **Missing** |
| Circle/sensor collision gameplay | `ScenePhysics2d` drives the runtime from authored components; Decay has velocity, impulse, and per-entity collision/sensor event queries | **Ready** |
| Mouse and touch movement | Unified `Pointer` plus raw `Touch` in Decay, routed through the editor Game view in its own pixels | **Ready** |
| Runtime HUD values | Screen text and images render | **Partial**; Decay cannot change text content |
| Interactive menus and modal flows | Anchored screen image/text components exist | **Missing** as a runtime button, focus, layout, and navigation system |
| Upgrade/build data | Exported scalar fields exist | **Missing** for collections, catalog queries, weighted choices, and loadout data |
| Sprite animation | Runtime/editor animation works | **Partial**; Decay cannot select or control clips |
| Audio | Native, browser, and silent backends plus Decay playback calls | **Ready**, with host-side clip gathering still manual |
| Persistent progression | Editor preferences persist, but there is no game save/storage API | **Missing** |
| Browser release | Gather proves a hand-built WASM/WebGPU Pages host | **Partial**; no general project export and browser asset fetching is not exercised |
| Full editor playtest | Scripts and animation run in editor Play | **Partial**; not the complete application/game loop |
| Mobile/browser fallback | WebGPU browser host exists, and touch reaches gameplay on every host | **Partial**; no WebGL fallback |

## What can be built now

### A Rust-authored prototype

A Rust game host could use Sindri today for:

- lifecycle and fixed-step timing
- the world and safe entity handles
- sprite, animation, text, camera, and layer extraction
- keyboard and platform-level pointer input
- audio requests
- native and WebGPU presentation

Game-specific Rust would still have to own spawning, pools or catalogs, queries,
collision integration or manual collision, random selection, UI state,
persistence, asset gathering, and the release host. This is technically
feasible, but it is not evidence that the editor and Decay product are ready for
the game.

### An editor + Decay implementation

Not yet, but the first two blockers are gone: a script can create an enemy from
an authored prefab, and it can ask the world for all of them at once. What
remains in the way is randomness, dynamic UI, touch input, collision from Decay,
and persistence. Pre-placing everything
would still be a workaround that teaches the engine the wrong lesson, and it is
still not what this audit is asking for.

## Gaps this game would expose

### 1. Spawn contract and reusable definitions — done

Delivered as **prefabs**: `docs/prefabs.md` for the format, `docs/scripting.md`
for the scripting surface. Every requirement below is met, except that making a
prefab still means writing the file — the editor cannot yet produce one from a
selection. What follows is the original statement of the requirement.

Decay needs a typed way to request an entity from an authored reusable
definition. The engine should decide whether that definition is called a prefab,
prototype, scene fragment, or something else before exposing `World.spawn`.
The contract must work identically in native, browser, editor Play, and tests.

Required behavior:

- spawn by validated project asset/reference
- return a generation-checked entity reference
- initialize exported properties before the first script callback
- support parent, transform, and initial component overrides without arbitrary
  JSON escaping into Decay
- report missing or incompatible definitions as typed errors
- make lifetime and script cancellation deterministic

### 2. Queries and collections — done

Delivered as `Array<T>` in the language and `World.with_tag` on the Sindri
surface. Every requirement below is met except querying by typed component,
which was deliberately not built: spelling `sindri.sprite` in a script puts
engine internals in gameplay code and makes every enemy that happens to have a
sprite an enemy. A tag says what an entity *is*, and that is the only way to
ask. What follows is the original statement of the requirement.

A bullet-heavy game cannot retain an individual authored reference to every
target. It needs bounded, typed access to groups.

A minimal useful slice would include:

- an `Array<Entity>` or bounded query result
- query by authored tag or typed component, not by name prefix
- deterministic ordering
- explicit allocation and operation limits
- safe behavior when an entity is despawned during iteration

This should build on the collection work already described in the Decay
roadmap, not introduce an Orbital-specific enemy list.

### 3. Gameplay collision surface — done

Delivered as `ScenePhysics2d` on the scene side and the `Physics` namespace in
Decay. Every requirement below is met except the instrumentation, which is the
measured-performance work the effects slice carries: circle and box colliders,
sensors and masks, velocity and kinematic movement, deterministic events, and
safe destruction from one. Piercing without duplicate hits falls out of
`collision_started` rather than needing a feature — an event that fires when
contact *begins* fires once per target.

Manual distance checks remain a valid gameplay tool: `World.with_tag` plus
transform reads is a loop a script can write, and nothing forces a game through
physics. What follows is the original statement of the requirement.

The Rapier2D foundation is useful, but the feature is not complete until scripts
can drive bodies and consume collision/sensor events.

For this workload, the first vertical slice must cover:

- circle and box colliders
- sensors and collision masks
- velocity or kinematic movement
- collision start/stop events delivered deterministically
- projectiles that can pierce several targets without duplicate hits
- safe destruction from an event
- enough instrumentation to verify that collision cost remains bounded

Manual distance checks may still be a valid gameplay tool, but they require a
typed query surface and must not force all games through physics.

### 4. Runtime UI

The current screen-image/text foundation is rendering, not yet a UI product.
This game needs:

- dynamic text and progress/fill values
- pointer/touch activation
- buttons and disabled/selected states
- anchors plus practical row, column, stack, scroll, and safe-area layout
- modal focus and game-input suppression
- screen/state navigation
- accessible labels on the web
- responsive behavior across portrait phones and desktop windows

A complete retained-mode UI system is not required before starting. A small,
coherent screen UI slice is.

### 5. Input parity — done

Delivered as the `Pointer` and `Touch` namespaces. Every requirement below is
met. The one thing deliberately not built is the normalized drag: the reference
game's deadzone and radius are tuning, and baking one game's numbers into an
engine is how an engine acquires a genre — a script records where the pointer
went down and subtracts, which is four lines. What follows is the original
statement of the requirement.

Decay should receive pointer position, button edges, and a normalized
touch/drag abstraction through the same input state the host already owns.
Editor Game view routing, browser pointer capture, focus loss, and viewport
coordinates must agree. A mobile game cannot rely on keyboard-only script input.

### 6. Persistence

The game needs a versioned, game-owned save API for settings and progression.

The first contract should support:

- named project-local save slots or key/value documents
- typed or JSON-like serializable data with explicit versioning
- browser storage and native filesystem implementations
- atomic replacement on native
- missing/corrupt/newer-version outcomes that gameplay can handle
- a silent in-memory test backend

Editor preferences are not game saves and should remain separate.

### 7. Effects and rendering

Orbital Last Stand uses many short-lived particles, rings, telegraphs, trails,
and color flashes. Creating a full world entity for every visual fleck may be
wasteful. The port should measure both approaches before choosing:

- a pooled lightweight particle/effect batch
- instanced sprite effects
- a small 2D primitive renderer
- custom material/shader support later

The acceptance target is readable high-volume combat, not reproducing Canvas 2D
calls one for one.

### 8. Export and browser delivery

The Gather host proves the runtime path, but a normal project still needs a
repeatable export.

A usable slice should:

- gather referenced scenes, textures, fonts, scripts, and audio
- produce a content-hashed static web directory
- generate the WASM/JavaScript host without hand-maintained project code
- work under a GitHub Pages project subpath
- expose a clear WebGPU-unavailable message
- document cache invalidation and deployment
- run a browser smoke test against the exported artifact

## Recommended implementation order

This is ordered by the earliest point at which a genuine playable slice becomes
possible, not by subsystem familiarity.

1. ~~**Reusable spawn definitions + typed Decay spawning**~~ — done; see
   `docs/prefabs.md`
2. ~~**Bounded entity queries and the first Decay collection**~~ — done; see
   `decay/LANGUAGE.md` on `Array<T>` and `docs/scripting.md` on `World.with_tag`
3. ~~**Pointer/touch input through Decay and Game view**~~ — done; see
   `docs/scripting.md` on `Pointer` and `Touch`
4. ~~**Typed 2D collision access and event delivery**~~ — done; see
   `docs/scripting.md` on `Physics` and `docs/physics.md` on the step order
5. **Dynamic text plus a minimal interactive screen UI**
6. **Host-owned seeded randomness**
7. **Game persistence boundary**
8. **Measured particle/effect path**
9. **Complete editor Play application loop**
10. **Static-web project export with gathered assets**

Audio clip gathering and animation control should join the vertical slice when
their consumers are reached; they should not block the first moving,
shooting, spawning prototype.

## Acceptance test: the ten-minute run

Sindri should not claim this vertical slice is supported until a project can
demonstrate all of the following without game-specific engine patches:

1. Open the project and its main scene in the editor.
2. Start a run from an interactive screen using mouse or touch.
3. Move the player with keyboard and pointer/touch controls.
4. Spawn, update, collide, and despawn enemies, bullets, pickups, and effects
   continuously for ten minutes.
5. Execute at least three enemy behaviors and one multi-phase boss.
6. Pause combat for a data-driven upgrade choice and apply the result.
7. Update HUD text and bars from gameplay state.
8. Play looping music and one-shot effects through the shared audio path.
9. Save settings and one persistent progression value, reload, and recover them.
10. Run equivalently in native preview and the actual browser build.
11. Export to a static directory and pass an automated browser smoke test.
12. Record frame-time, entity-count, query, collision, and allocation evidence
    from a late-run stress point.

The reference game has produced runs above 2,500 kills with dense projectile and
pickup populations. The Sindri test does not need the same balance or art, but
it should use a comparable workload so a quiet demo cannot satisfy a
high-churn acceptance criterion.

## Scope guardrails

This audit should not cause Sindri to:

- hard-code survivor-like concepts such as waves, XP gems, modules, or bosses
- expose Rapier handles or renderer internals to Decay
- add unbounded queries or collections
- couple save data to editor settings
- require WebGPU-specific gameplay code
- copy the current JavaScript architecture into the engine
- replace Gather as the companion game before the missing generic slices exist

The right result is a set of general capabilities that also make other action,
arcade, tower-defense, and simulation games easier to author.

## Recommended role for the reference game

Keep Gather as the continuity game for isometric/grid/editor development.
Treat Orbital Last Stand as a **second vertical-slice acceptance project** for
real-time 2D action once spawning, queries, and pointer input begin.

It should initially live outside the engine repository or in a clearly bounded
`games/` project, consume public Sindri surfaces, and refuse private engine
shortcuts. That separation is the test: if the game must reach into engine
internals, the public authoring surface is still incomplete.

## Related source-of-truth documents

- [Current capabilities](capabilities.md)
- [Feature integration matrix](feature-integration-matrix.md)
- [Decay host surface](scripting.md)
- [Decay language reference](../decay/LANGUAGE.md)
- [2D physics boundary](physics.md)
- [Entity scaling measurements](entity-scaling.md)
- [Roadmap](../ROADMAP.md)
