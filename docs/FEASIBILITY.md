# Sindri Next feasibility review

Reviewed: 2026-08-14
Reconciled with the implemented direction: 2026-08-21

## Verdict

The shared Rust runtime, native editor, Decay gameplay layer, and native/WebGPU
targets are technically feasible and now proven together by the companion game.
It remains a multi-year engine product rather than a single release-sized
project. The largest risk is not `wgpu`; it is keeping the runtime, serialized
model, editor, Decay host, and eventual export pipeline aligned without growing
parallel representations.

`PROJECT_OVERVIEW.md` is the historical TypeScript-first proposal. The accepted
product direction is Editor + Decay, recorded in `docs/decay-direction.md`.
TypeScript may still earn a deliberate embedding layer for web applications,
but it is not the primary browser gameplay API or a first-release gate.

The legacy `vardirhq/sindri-engine` repository contains substantial reusable 2D concepts and product work, including a `wgpu` backend, world/entity code, scenes, input, physics, pathfinding, scripting, examples, and a Tauri editor. It is useful migration material. It should not be copied wholesale: several large modules currently combine responsibilities that Sindri Next explicitly needs to separate.

## Decisions required for realism

- **Runtime handles are not scene IDs.** Runtime entities use compact generation-checked handles. Scene files use stable logical string IDs. This prevents allocator details from becoming permanent file-format API.
- **Scene compatibility is versioned from day one.** Every scene has a format version. Loaders reject unsupported versions until explicit migrations exist.
- **Any future WASM embedding API stays coarse-grained.** Commands, events, and
  queries cross in batches; ordinary gameplay runs inside the shared Rust +
  Decay runtime rather than making thousands of calls per frame.
- **The web loop and native loop share semantics, not identical plumbing.** Browser animation frames, async initialization, canvas lifecycle, and native `winit` event loops live behind platform adapters.
- **WebGPU is the first browser target, not the only eventual backend.** Lack of WebGPU support must produce a clear capability error. A WebGL fallback is deliberately outside the first release.
- **The native editor embeds the real runtime.** `egui` provides the tooling UI
  while the in-process Sindri runtime owns viewport rendering. A versioned
  protocol is required only before a separately running editor and runtime are
  allowed to communicate; in-process edits already use checked world commands.
- **Custom components need schemas before typed editing.** JSON payloads provide
  forward-compatible storage, and the implemented registry supplies validation,
  metadata, defaults, and generic inspector editing.
- **2D and 3D share one transform and GPU infrastructure, not every high-level
  abstraction.** Distinct render components and explicit passes establish
  ordering when 3D world content, world sprites, and overlays coexist.
- **The first release is a foundation release.** It excludes advanced PBR, skeletal animation, networking, a plugin market, native mobile, and WebGL fallback.

## Risk register

| Risk | Impact | Mitigation / gate |
|---|---:|---|
| Future Rust/JS ownership and callback churn | Medium | Add an embedding API only for a proven use case; batch commands/events at the boundary |
| Scene format churn | High | Version field, fixtures, migrations, golden tests |
| Editor/runtime divergence | High | One live world, checked commands, canonical scene round trips, and the runtime-rendered viewport |
| Over-fragmented crate graph | Medium | Create crates only at proven platform or dependency boundaries |
| Legacy code copied with desktop coupling | High | Port behavior subsystem-by-subsystem with web compile checks |
| 2D/3D render ordering ambiguity | Medium | Explicit camera/pass/layer model before combined-scene milestone |
| WebGPU/browser differences | Medium | Automated browser smoke tests plus capability diagnostics |
| Engine scope overwhelming a small team | High | Vertical milestones with explicit non-goals and exit criteria |

## Definition of the first major release

The realistic first major release is reached when one small authored game can be
built through the Editor/Decay project model, loaded from the same versioned
scene and assets in native and browser runtimes, edited and previewed in the
editor, and exported without hand-building a host. Input, resize, fixed
simulation, asset loading, transform/component/script-property editing, useful
diagnostics, and static web export must work. Everything beyond that is
subsequent product development.
