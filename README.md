# Sindri Next

Sindri Next is the new foundation for a coherent Rust-powered 2D and 3D game engine targeting native applications and modern browsers, with a first-class TypeScript API and an integrated visual editor.

The repository is at its foundation stage. The renderer-independent core currently provides:

- strict engine lifecycle semantics
- capped frame time and fixed-step simulation
- generation-checked runtime entities
- safe entity hierarchies
- versioned, editor-friendly scene documents
- stable serialized IDs kept separate from runtime handles
- shared `wgpu` 30 device/surface negotiation
- a reusable triangle renderer proven by one native/browser example

Read the [project overview](PROJECT_OVERVIEW.md), the [feasibility review](docs/FEASIBILITY.md), and the checkable [roadmap](ROADMAP.md).

The rendering proofs now include the [shared triangle example](examples/triangle/README.md) and a [depth-tested textured cube](examples/cube/README.md). Both use the same GPU and renderer crates on native desktops and in WebGPU browsers.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

The workspace declares Rust 1.87 as its minimum supported Rust version, matching `wgpu` 30.

## Status

Sindri Next is pre-alpha. Public APIs and serialized formats may change while the foundation milestones are being proven, but format changes will be explicit and versioned.
