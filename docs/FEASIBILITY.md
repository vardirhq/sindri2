# Sindri Next feasibility review

Reviewed: 2026-08-14

## Verdict

The direction in `PROJECT_OVERVIEW.md` is technically feasible. It is a multi-year engine product, not a single release-sized project, but its first useful release can be scoped realistically. The strongest choice is the shared Rust runtime with platform adapters and a deliberate TypeScript layer. The largest risk is not `wgpu`; it is keeping the runtime, serialized model, editor, and TypeScript API aligned without coupling them together.

The legacy `vardirhq/sindri-engine` repository contains substantial reusable 2D concepts and product work, including a `wgpu` backend, world/entity code, scenes, input, physics, pathfinding, scripting, examples, and a Tauri editor. It is useful migration material. It should not be copied wholesale: several large modules currently combine responsibilities that Sindri Next explicitly needs to separate.

## Decisions required for realism

- **Runtime handles are not scene IDs.** Runtime entities use compact generation-checked handles. Scene files use stable logical string IDs. This prevents allocator details from becoming permanent file-format API.
- **Scene compatibility is versioned from day one.** Every scene has a format version. Loaders reject unsupported versions until explicit migrations exist.
- **WASM calls are coarse-grained.** TypeScript sends command batches and receives event/query batches. The public SDK must not turn ordinary updates into thousands of calls per frame.
- **The web loop and native loop share semantics, not identical plumbing.** Browser animation frames, async initialization, canvas lifecycle, and native `winit` event loops live behind platform adapters.
- **WebGPU is the first browser target, not the only eventual backend.** Lack of WebGPU support must produce a clear capability error. A WebGL fallback is deliberately outside the first release.
- **The editor embeds or talks to the real runtime.** React never reimplements the viewport renderer. A small, versioned editor protocol must precede editor migration.
- **Custom components need schemas before editor editing.** JSON payloads allow early forward-compatible storage, but typed registration/metadata is required before the editor or TypeScript SDK can safely manipulate arbitrary components.
- **2D and 3D share GPU infrastructure, not every high-level abstraction.** Separate transforms and render components are reasonable. Render passes establish ordering when 3D world content and 2D overlays coexist.
- **The first release is a foundation release.** It excludes advanced PBR, skeletal animation, networking, a plugin market, native mobile, and WebGL fallback.

## Risk register

| Risk | Impact | Mitigation / gate |
|---|---:|---|
| Rust/JS ownership and callback churn | High | Command/event batching benchmark before SDK expansion |
| Scene format churn | High | Version field, fixtures, migrations, golden tests |
| Editor/runtime divergence | High | Protocol contract tests and runtime-rendered viewport |
| Over-fragmented crate graph | Medium | Create crates only at proven platform or dependency boundaries |
| Legacy code copied with desktop coupling | High | Port behavior subsystem-by-subsystem with web compile checks |
| 2D/3D render ordering ambiguity | Medium | Explicit camera/pass/layer model before combined-scene milestone |
| WebGPU/browser differences | Medium | Automated browser smoke tests plus capability diagnostics |
| Engine scope overwhelming a small team | High | Vertical milestones with explicit non-goals and exit criteria |

## Definition of the first major release

The realistic first major release is reached when a small scene containing a textured mesh and an animated sprite can be loaded from the same versioned scene file in a native Rust example, a browser TypeScript example, and the editor viewport. Input, resize, fixed simulation, asset loading, transform editing, and static web export must work. Everything beyond that is subsequent product development.

