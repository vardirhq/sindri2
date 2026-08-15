# Sindri Next

Sindri Next is the new foundation for a coherent Rust-powered 2D and 3D game engine targeting native applications and modern browsers, with a first-class TypeScript API and an integrated visual editor.

The repository is at its foundation stage. The renderer-independent core currently provides:

- strict engine lifecycle semantics
- capped frame time and fixed-step simulation with drift-free rational time scale
- a platform boundary with keyboard and pointer input, clock traits, and fallible gameplay hooks
- a host loop that runs a game headlessly, in a window, or in a browser
- component-driven extraction that draws any scene without hand-written render code
- generation-checked runtime entities
- safe entity hierarchies
- versioned, editor-friendly scene documents
- stable serialized IDs kept separate from runtime handles
- lossless world-to-scene saving with canonical, review-friendly serialization
- a scene migration API defined before the format needs it
- portable logical asset IDs with typed, lifetime-aware runtime handles
- a shared asynchronous source contract with native filesystem and browser fetch implementations
- a bounded cross-platform load queue with native I/O workers and non-blocking browser polling
- typed texture and validated scene decoding with generation-safe store completion
- shared `wgpu` 30 device/surface negotiation
- a reusable triangle renderer proven by one native/browser example

Read the [project overview](PROJECT_OVERVIEW.md), the [feasibility review](docs/FEASIBILITY.md), the [asset foundation](docs/asset-foundation.md), the [component schema registry](docs/component-schema-registry.md), the [scene serialization contract](docs/scene-serialization.md), the [scene extraction seam](docs/scene-extraction.md), the [transparent rendering policy](docs/rendering-transparency.md), the [colour handling contract](docs/rendering-color.md), and the checkable [roadmap](ROADMAP.md).

The rendering proofs, including a headless PNG capture used by CI, now include the [shared triangle example](examples/triangle/README.md) and a [depth-tested textured cube with a 2D sprite overlay](examples/cube/README.md). Both use the same GPU and renderer crates on native desktops and in WebGPU browsers.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

The workspace declares Rust 1.95 as its minimum supported Rust version, matching the native `egui` 0.36 editor and `wgpu` 30.

Run the native editor shell with:

```bash
cargo run --package sindri-editor
```

Capture a deterministic editor screenshot on Linux with `imagemagick`, `xdotool`, and `xvfb-run` installed:

```bash
xvfb-run --auto-servernum ./scripts/capture-editor.sh target/editor.png
```

## Status

Sindri Next is pre-alpha. Public APIs and serialized formats may change while the foundation milestones are being proven, but format changes will be explicit and versioned.
