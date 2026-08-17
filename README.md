# Sindri Next

**A lightweight 2D + 3D game engine for Rust and the web.**

Build native games in Rust or browser games in TypeScript, using the same engine model, scene format, renderer, asset system, and visual editor.

Sindri is Rust-powered, WebGPU-native, and designed so the web is a first-class target rather than a separate edition of the engine.

> **Status:** Sindri Next is pre-alpha and under active development. Public APIs and serialized formats may change while the foundation milestones are being proven, but format changes are explicit and versioned.

## One engine, multiple ways to build

Sindri is being designed around two equally important authoring experiences.

### Rust

```rust
use sindri::prelude::*;

fn main() {
    Engine::new()
        .run(MyGame::new());
}
```

### TypeScript

```ts
import { Engine, Scene, Sprite } from "@sindri/engine";

const engine = await Engine.create({
  canvas: "#game",
});

engine.start();
```

The TypeScript API is intended to feel like a deliberately designed game engine API, not a thin layer over raw WebAssembly exports.

## Why Sindri?

### Native and web share the same foundation

Desktop and browser hosts use the same core engine concepts and the same `wgpu`-based GPU and rendering crates. Input, timing, scene extraction, assets, and presentation policies are designed once and carried across targets rather than reimplemented for each platform.

### 2D and 3D belong in the same engine

Sindri shares the infrastructure that should be shared — scenes, entities, assets, textures, cameras, timing, input, rendering passes, and resource handles — without forcing 2D into unnecessarily complicated 3D abstractions.

### The editor understands the runtime model

Scenes use stable serialized IDs that are deliberately separate from generation-checked runtime handles. The world can round-trip back to canonical, review-friendly scene files, and the editor works on those same scenes instead of maintaining a parallel representation.

### Complexity has to earn its place

Sindri prefers simple systems until measurements show they are the wrong systems. The current world model was profiled at 1k, 10k, and 100k entities before considering an archetype ECS. That benchmark found and removed a quadratic scene-validation bug; after the fix, entity storage was not the bottleneck, so an ECS was deliberately not introduced.

## What works today

The foundation already includes:

- strict engine lifecycle semantics
- capped frame time and fixed-step simulation with drift-free rational time scale
- keyboard and pointer input behind a shared platform boundary
- a shared windowed host for native desktops and modern browsers
- headless, windowed, and browser host loops
- component-driven scene extraction without hand-written per-scene render code
- scene texture references resolved to runtime textures with a missing-texture fallback
- generation-checked runtime entities and safe hierarchies
- versioned, editor-friendly scene documents
- stable authored IDs kept separate from runtime handles
- lossless world-to-scene saving with canonical serialization
- a scene migration API
- portable logical asset IDs and typed, lifetime-aware runtime handles
- native filesystem and browser-fetch asset sources behind one asynchronous contract
- a bounded cross-platform asset load queue
- typed texture and validated scene decoding
- shared `wgpu` 30 device and surface negotiation
- one presentation-surface policy across hosts
- a native visual editor shell that opens, edits, saves, reloads, orbits, pans, and zooms through real scene files

The rendering proofs include a [shared triangle example](examples/triangle/README.md) and a [depth-tested textured cube with a 2D sprite overlay](examples/cube/README.md). Both use the same GPU and renderer crates on native desktops and in WebGPU browsers. CI also exercises a headless PNG capture path.

## Try the editor

The workspace currently requires Rust 1.95.

```bash
cargo run --package sindri-editor
```

The editor is still early, but it already works with the same scene representation used by the runtime rather than a separate editor-only format.

## Architecture

Sindri is split into focused crates so platform, GPU, rendering, scene, asset, and engine concerns can evolve independently without leaking into the renderer-independent core.

```text
                    Sindri

        ┌─────────────┼─────────────┐
        │             │             │
       2D            3D         Isometric
        │             │             │
        └─────────────┼─────────────┘
                      │
                 Engine Core
                      │
                 GPU Renderer
                      │
                    wgpu
              ┌───────┴────────┐
              │                │
           Native            Web
                           WASM/WebGPU
```

The renderer-independent core deliberately has no dependency on a window, GPU, browser, editor, physics engine, scripting runtime, or async executor.

For the longer-term direction, read the [project overview](PROJECT_OVERVIEW.md) and the checkable [roadmap](ROADMAP.md).

The detailed contracts live alongside the code:

- [feasibility review](docs/FEASIBILITY.md)
- [asset foundation](docs/asset-foundation.md)
- [component schema registry](docs/component-schema-registry.md)
- [scene serialization](docs/scene-serialization.md)
- [scene extraction](docs/scene-extraction.md)
- [how Sindri does 2D](docs/2d-model.md)
- [transparent rendering](docs/rendering-transparency.md)
- [colour handling](docs/rendering-color.md)
- [presentation surfaces](docs/rendering-surface.md)
- [windowed host](docs/platform-host.md)
- [entity scaling](docs/entity-scaling.md)
- [what Sindri can do today](docs/capabilities.md)
- [versioning](docs/versioning.md)

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

The workspace declares Rust 1.95 as its minimum supported Rust version, matching the native `egui` 0.36 editor and `wgpu` 30.

To capture a deterministic editor screenshot on Linux with `imagemagick`, `xdotool`, and `xvfb-run` installed:

```bash
xvfb-run --auto-servernum ./scripts/capture-editor.sh target/editor.png
```

## Project status

Sindri Next is pre-alpha. The current goal is to prove a coherent foundation before expanding the feature surface.

Crate and scene-file versioning rules are documented today. The editor protocol and npm SDK versioning policy are deliberately still undecided; see [`docs/versioning.md`](docs/versioning.md).

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for checks, conventions, and commit style, and the [code of conduct](CODE_OF_CONDUCT.md) for participation guidelines. The [dependency policy](docs/dependency-policy.md) documents what the tree is allowed to contain and how that is enforced.

## License

Dual-licensed under either [Apache License 2.0](LICENSE-APACHE) or the [MIT license](LICENSE-MIT), at your option.

Unless you state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache 2.0 license, shall be dual-licensed as above, with no additional terms or conditions.
