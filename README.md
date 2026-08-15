# Sindri Next

Sindri Next is the new foundation for a coherent Rust-powered 2D and 3D game engine targeting native applications and modern browsers, with a first-class TypeScript API and an integrated visual editor.

The repository is at its foundation stage. The renderer-independent core currently provides:

- strict engine lifecycle semantics
- capped frame time and fixed-step simulation with drift-free rational time scale
- a platform boundary with keyboard and pointer input, clock traits, and fallible gameplay hooks
- a shared windowed host owning the window, event loop, device request, timing, and input on native desktops and in browsers
- a host loop that runs a game headlessly, in a window, or in a browser
- component-driven extraction that draws any scene without hand-written render code
- scene texture references resolved to renderer textures, with a missing-texture fallback
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
- one presentation surface policy, so hosts recover from a resized, hidden, or lost surface identically
- a reusable triangle renderer proven by one native/browser example

Read the [project overview](PROJECT_OVERVIEW.md), the [feasibility review](docs/FEASIBILITY.md), the [asset foundation](docs/asset-foundation.md), the [component schema registry](docs/component-schema-registry.md), the [scene serialization contract](docs/scene-serialization.md), the [scene extraction seam](docs/scene-extraction.md), the [transparent rendering policy](docs/rendering-transparency.md), the [colour handling contract](docs/rendering-color.md), the [presentation surface policy](docs/rendering-surface.md), the [windowed host](docs/platform-host.md), and the checkable [roadmap](ROADMAP.md).

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

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for the checks, conventions, and commit style, and the [code of conduct](CODE_OF_CONDUCT.md) for what participation looks like. The [dependency policy](docs/dependency-policy.md) covers what the tree is allowed to contain and how it is enforced.

## Status

Sindri Next is pre-alpha. Public APIs and serialized formats may change while the foundation milestones are being proven, but format changes will be explicit and versioned. What "versioned" means for crates and scene files — and what is deliberately still undecided for the editor protocol and the npm SDK — is written down in [`docs/versioning.md`](docs/versioning.md).

## License

Dual-licensed under either [Apache License 2.0](LICENSE-APACHE) or the [MIT license](LICENSE-MIT), at your option.

Unless you state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache 2.0 license, shall be dual-licensed as above, with no additional terms or conditions.
